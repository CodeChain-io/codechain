// Copyright 2018 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

pub mod kind;

use std::cmp;
use std::collections::{VecDeque, HashSet, HashMap};
use std::sync::{Condvar as SCondvar, Mutex as SMutex, Arc};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::thread::{self, JoinHandle};

use cio::IoChannel;
use ctypes::{H256, U256};
use num_cpus;
use parking_lot::{Mutex, RwLock};

use super::super::consensus::CodeChainEngine;
use super::super::error::{Error, BlockError, ImportError};
use super::super::service::ClientIoMessage;
use self::kind::{BlockLike, Kind};

// maximum possible number of verification threads.
const MAX_VERIFIERS: usize = 8;

/// Type alias for block queue convenience.
pub type BlockQueue = VerificationQueue<kind::Blocks>;

pub struct VerificationQueue<K: Kind> {
    engine: Arc<CodeChainEngine>,
    verification: Arc<Verification<K>>,
    processing: RwLock<HashMap<H256, U256>>, // hash to score
    deleting: Arc<AtomicBool>,
    ready_signal: Arc<QueueSignal>,
    total_score: RwLock<U256>,
    empty: Arc<SCondvar>,
    more_to_verify: Arc<SCondvar>,
    verifier_handles: Vec<JoinHandle<()>>,
}

struct QueueSignal {
    deleting: Arc<AtomicBool>,
    signalled: AtomicBool,
    message_channel: Mutex<IoChannel<ClientIoMessage>>,
}

impl QueueSignal {
    fn set_sync(&self) {
        // Do not signal when we are about to close
        if self.deleting.load(AtomicOrdering::Relaxed) {
            return;
        }

        if self.signalled.compare_and_swap(false, true, AtomicOrdering::Relaxed) == false {
            let channel = self.message_channel.lock().clone();
            if let Err(e) = channel.send_sync(ClientIoMessage::BlockVerified) {
                debug!("Error sending BlockVerified message: {:?}", e);
            }
        }
    }

    fn set_async(&self) {
        // Do not signal when we are about to close
        if self.deleting.load(AtomicOrdering::Relaxed) {
            return;
        }

        if self.signalled.compare_and_swap(false, true, AtomicOrdering::Relaxed) == false {
            let channel = self.message_channel.lock().clone();
            if let Err(e) = channel.send(ClientIoMessage::BlockVerified) {
                debug!("Error sending BlockVerified message: {:?}", e);
            }
        }
    }

    fn reset(&self) {
        self.signalled.store(false, AtomicOrdering::Relaxed);
    }
}

impl<K: Kind> VerificationQueue<K> {
    pub fn new(engine: Arc<CodeChainEngine>, message_channel: IoChannel<ClientIoMessage>, check_seal: bool) -> Self {
        let verification = Arc::new(Verification {
            unverified: Mutex::new(VecDeque::new()),
            verifying: Mutex::new(VecDeque::new()),
            verified: Mutex::new(VecDeque::new()),
            bad: Mutex::new(HashSet::new()),
            check_seal,
            empty_mutex: SMutex::new(()),
            more_to_verify_mutex: SMutex::new(()),
        });
        let deleting = Arc::new(AtomicBool::new(false));
        let ready_signal = Arc::new(QueueSignal {
            deleting: deleting.clone(),
            signalled: AtomicBool::new(false),
            message_channel: Mutex::new(message_channel),
        });
        let empty = Arc::new(SCondvar::new());
        let more_to_verify = Arc::new(SCondvar::new());

        let num_verifiers = cmp::min(num_cpus::get(), MAX_VERIFIERS);
        let mut verifier_handles = Vec::with_capacity(num_verifiers);

        for i in 0..num_verifiers {
            let engine = engine.clone();
            let verification = verification.clone();
            let more_to_verify = more_to_verify.clone();
            let ready_signal = ready_signal.clone();
            let empty = empty.clone();

            let handle = thread::Builder::new()
                .name(format!("Verifier #{}", i))
                .spawn(move || {
                    VerificationQueue::verify(
                        verification,
                        engine,
                        ready_signal,
                        empty,
                        more_to_verify,
                        i,
                    )
                })
                .expect("Failed to create verifier thread.");
            verifier_handles.push(handle);
        }

        Self {
            engine,
            verification,
            processing: RwLock::new(HashMap::new()),
            deleting,
            ready_signal,
            total_score: RwLock::new(0.into()),
            empty,
            more_to_verify,
            verifier_handles
        }
    }

    fn verify(
        verification: Arc<Verification<K>>,
        engine: Arc<CodeChainEngine>,
        ready_signal: Arc<QueueSignal>,
        empty: Arc<SCondvar>,
        more_to_verify: Arc<SCondvar>,
        _id: usize,
    ) {
        loop {
            // wait for work if empty.
            {
                let mut more_to_verify_mutex = verification.more_to_verify_mutex.lock().unwrap();

                if verification.unverified.lock().is_empty() && verification.verifying.lock().is_empty() {
                    empty.notify_all();
                }

                while verification.unverified.lock().is_empty() {
                    more_to_verify_mutex = more_to_verify.wait(more_to_verify_mutex).unwrap();
                }
            }

            // do work.
            let item = {
                // acquire these locks before getting the item to verify.
                let mut unverified = verification.unverified.lock();
                let mut verifying = verification.verifying.lock();

                let item = match unverified.pop_front() {
                    Some(item) => item,
                    None => continue,
                };

                verifying.push_back(Verifying { hash: item.hash(), output: None });
                item
            };

            let hash = item.hash();
            let is_ready = match K::verify(item, &*engine, verification.check_seal) {
                Ok(verified) => {
                    let mut verifying = verification.verifying.lock();
                    let mut idx = None;
                    for (i, e) in verifying.iter_mut().enumerate() {
                        if e.hash == hash {
                            idx = Some(i);

                            e.output = Some(verified);
                            break;
                        }
                    }

                    if idx == Some(0) {
                        // we're next!
                        let mut verified = verification.verified.lock();
                        let mut bad = verification.bad.lock();
                        VerificationQueue::drain_verifying(&mut verifying, &mut verified, &mut bad);
                        true
                    } else {
                        false
                    }
                },
                Err(_) => {
                    let mut verifying = verification.verifying.lock();
                    let mut verified = verification.verified.lock();
                    let mut bad = verification.bad.lock();

                    bad.insert(hash.clone());
                    verifying.retain(|e| e.hash != hash);

                    if verifying.front().map_or(false, |x| x.output.is_some()) {
                        VerificationQueue::drain_verifying(&mut verifying, &mut verified, &mut bad);
                        true
                    } else {
                        false
                    }
                }
            };
            if is_ready {
                // Import the block immediately
                ready_signal.set_sync();
            }
        }
    }

    fn drain_verifying(
        verifying: &mut VecDeque<Verifying<K>>,
        verified: &mut VecDeque<K::Verified>,
        bad: &mut HashSet<H256>,
    ) {
        while let Some(output) = verifying.front_mut().and_then(|x| x.output.take()) {
            assert!(verifying.pop_front().is_some());
            if bad.contains(&output.parent_hash()) {
                bad.insert(output.hash());
            } else {
                verified.push_back(output);
            }
        }
    }

    /// Add a block to the queue.
    pub fn import(&self, input: K::Input) -> Result<H256, Error> {
        let h = input.hash();
        {
            if self.processing.read().contains_key(&h) {
                return Err(ImportError::AlreadyQueued.into());
            }

            let mut bad = self.verification.bad.lock();
            if bad.contains(&h) {
                return Err(ImportError::KnownBad.into());
            }

            if bad.contains(&input.parent_hash()) {
                bad.insert(h.clone());
                return Err(ImportError::KnownBad.into());
            }
        }
        match K::create(input, &*self.engine) {
            Ok(item) => {
                self.processing.write().insert(h.clone(), item.score());
                {
                    let mut ts = self.total_score.write();
                    *ts = *ts + item.score();
                }

                self.verification.unverified.lock().push_back(item);
                self.more_to_verify.notify_all();
                Ok(h)
            },
            Err(err) => {
                match err {
                    // Don't mark future blocks as bad.
                    Error::Block(BlockError::TemporarilyInvalid(_)) => {},
                    _ => {
                        self.verification.bad.lock().insert(h.clone());
                    }
                }
                Err(err)
            }
        }
    }

    /// Removes up to `max` verified items from the queue
    pub fn drain(&self, max: usize) -> Vec<K::Verified> {
        let mut verified = self.verification.verified.lock();
        let count = cmp::min(max, verified.len());
        let result = verified.drain(..count).collect::<Vec<_>>();

        self.ready_signal.reset();
        if !verified.is_empty() {
            self.ready_signal.set_async();
        }
        result
    }

    /// Mark given item as processed.
    /// Returns true if the queue becomes empty.
    pub fn mark_as_good(&self, hashes: &[H256]) -> bool {
        if hashes.is_empty() {
            return self.processing.read().is_empty();
        }
        let mut processing = self.processing.write();
        for hash in hashes {
            if let Some(score) = processing.remove(hash) {
                let mut td = self.total_score.write();
                *td = *td - score;
            }
        }
        processing.is_empty()
    }

    /// Mark given item and all its children as bad. pauses verification
    /// until complete.
    pub fn mark_as_bad(&self, hashes: &[H256]) {
        if hashes.is_empty() {
            return;
        }
        let mut verified_lock = self.verification.verified.lock();
        let verified = &mut *verified_lock;
        let mut bad = self.verification.bad.lock();
        let mut processing = self.processing.write();
        bad.reserve(hashes.len());
        for hash in hashes {
            bad.insert(hash.clone());
            if let Some(score) = processing.remove(hash) {
                let mut td = self.total_score.write();
                *td = *td - score;
            }
        }

        let mut new_verified = VecDeque::new();
        for output in verified.drain(..) {
            if bad.contains(&output.parent_hash()) {
                bad.insert(output.hash());
                if let Some(score) = processing.remove(&output.hash()) {
                    let mut td = self.total_score.write();
                    *td = *td - score;
                }
            } else {
                new_verified.push_back(output);
            }
        }

        *verified = new_verified;
    }

    /// Get the total score of all the blocks in the queue.
    pub fn total_score(&self) -> U256 {
        self.total_score.read().clone()
    }
}

struct Verification<K: Kind> {
    unverified: Mutex<VecDeque<K::Unverified>>,
    verifying: Mutex<VecDeque<Verifying<K>>>,
    verified: Mutex<VecDeque<K::Verified>>,
    bad: Mutex<HashSet<H256>>,
    check_seal: bool,
    empty_mutex: SMutex<()>,
    more_to_verify_mutex: SMutex<()>,
}

/// An item which is in the process of being verified.
pub struct Verifying<K: Kind> {
    hash: H256,
    output: Option<K::Verified>,
}

#[cfg(test)]
mod tests {
    use cio::IoChannel;
    use tests::helpers::*;

    use super::BlockQueue;
    use super::kind::blocks::Unverified;
    use super::super::super::error::{Error, ImportError};
    use super::super::super::spec::Spec;

    // create a test block queue.
    // auto_scaling enables verifier adjustment.
    fn get_test_queue() -> BlockQueue {
        let spec = get_test_spec();
        let engine = spec.engine;

        BlockQueue::new(engine, IoChannel::disconnected(), true)
    }

    #[test]
    fn can_be_created() {
        // TODO better test
        let spec = Spec::new_solo();
        let engine = spec.engine;
        let _ = BlockQueue::new(engine, IoChannel::disconnected(), true);
    }

    #[test]
    fn can_import_blocks() {
        let queue = get_test_queue();
        if let Err(e) = queue.import(Unverified::new(get_good_dummy_block())) {
            panic!("error importing block that is valid by definition({:?})", e);
        }
    }

    #[test]
    fn returns_error_for_duplicates() {
        let queue = get_test_queue();
        if let Err(e) = queue.import(Unverified::new(get_good_dummy_block())) {
            panic!("error importing block that is valid by definition({:?})", e);
        }

        let duplicate_import = queue.import(Unverified::new(get_good_dummy_block()));
        match duplicate_import {
            Err(e) => {
                match e {
                    Error::Import(ImportError::AlreadyQueued) => {},
                    _ => { panic!("must return AlreadyQueued error"); }
                }
            }
            Ok(_) => { panic!("must produce error"); }
        }
    }
}
