// Copyright 2018-2019 Kodebox, Inc.
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
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Condvar as SCondvar, Mutex as SMutex};
use std::thread::{self, JoinHandle};

use cio::IoChannel;
use num_cpus;
use parking_lot::{Mutex, RwLock};
use primitives::{H256, U256};

use self::kind::{BlockLike, Kind, MemUsage};
use crate::consensus::CodeChainEngine;
use crate::error::{BlockError, Error, ImportError};
use crate::service::ClientIoMessage;
use crate::types::{BlockStatus as Status, VerificationQueueInfo as QueueInfo};

const MIN_MEM_LIMIT: usize = 16384;
const MIN_QUEUE_LIMIT: usize = 512;

// maximum possible number of verification threads.
const MAX_VERIFIERS: usize = 8;

/// Type alias for block queue convenience.
pub type BlockQueue = VerificationQueue<kind::Blocks>;
pub type HeaderQueue = VerificationQueue<kind::Headers>;

/// Verification queue configuration
#[derive(Debug, PartialEq, Clone)]
pub struct Config {
    /// Maximum number of items to keep in unverified queue.
    /// When the limit is reached, is_full returns true.
    pub max_queue_size: usize,
    /// Maximum heap memory to use.
    /// When the limit is reached, is_full returns true.
    pub max_mem_use: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_queue_size: 30000,
            max_mem_use: 50 * 1024 * 1024,
        }
    }
}

pub struct VerificationQueue<K: Kind> {
    engine: Arc<CodeChainEngine>,
    verification: Arc<Verification<K>>,
    processing: RwLock<HashMap<H256, U256>>, // hash to score
    #[allow(dead_code)]
    deleting: Arc<AtomicBool>,
    ready_signal: Arc<QueueSignal>,
    total_score: RwLock<U256>,
    #[allow(dead_code)]
    empty: Arc<SCondvar>,
    more_to_verify: Arc<SCondvar>,
    #[allow(dead_code)]
    verifier_handles: Vec<JoinHandle<()>>,
    max_queue_size: usize,
    max_mem_use: usize,
}

struct QueueSignal {
    deleting: Arc<AtomicBool>,
    signalled: AtomicBool,
    message_channel: Mutex<IoChannel<ClientIoMessage>>,
    message: ClientIoMessage,
}

impl QueueSignal {
    fn set_sync(&self) {
        // Do not signal when we are about to close
        if self.deleting.load(AtomicOrdering::Relaxed) {
            return
        }

        if !self.signalled.compare_and_swap(false, true, AtomicOrdering::Relaxed) {
            let channel = self.message_channel.lock().clone();
            if let Err(e) = channel.send_sync(self.message.clone()) {
                debug!("Error sending verified message: {:?}", e);
            }
        }
    }

    fn set_async(&self) {
        // Do not signal when we are about to close
        if self.deleting.load(AtomicOrdering::Relaxed) {
            return
        }

        if !self.signalled.compare_and_swap(false, true, AtomicOrdering::Relaxed) {
            let channel = self.message_channel.lock().clone();
            if let Err(e) = channel.send(self.message.clone()) {
                debug!("Error sending verified message: {:?}", e);
            }
        }
    }

    fn reset(&self) {
        self.signalled.store(false, AtomicOrdering::Relaxed);
    }
}

impl<K: Kind> VerificationQueue<K> {
    pub fn new(
        config: &Config,
        engine: Arc<CodeChainEngine>,
        message_channel: IoChannel<ClientIoMessage>,
        check_seal: bool,
    ) -> Self {
        let verification = Arc::new(Verification {
            unverified: Mutex::new(VecDeque::new()),
            verifying: Mutex::new(VecDeque::new()),
            verified: Mutex::new(VecDeque::new()),
            bad: Mutex::new(HashSet::new()),
            sizes: Sizes {
                unverified: AtomicUsize::new(0),
                verifying: AtomicUsize::new(0),
                verified: AtomicUsize::new(0),
            },
            check_seal,
            empty_mutex: SMutex::new(()),
            more_to_verify_mutex: SMutex::new(()),
        });
        let deleting = Arc::new(AtomicBool::new(false));
        let ready_signal = Arc::new(QueueSignal {
            deleting: deleting.clone(),
            signalled: AtomicBool::new(false),
            message_channel: Mutex::new(message_channel),
            message: K::signal(),
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
                    VerificationQueue::verify(&verification, &*engine, &*ready_signal, &*empty, &*more_to_verify, i)
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
            verifier_handles,
            max_queue_size: cmp::max(config.max_queue_size, MIN_QUEUE_LIMIT),
            max_mem_use: cmp::max(config.max_mem_use, MIN_MEM_LIMIT),
        }
    }

    fn verify(
        verification: &Verification<K>,
        engine: &CodeChainEngine,
        ready_signal: &QueueSignal,
        empty: &SCondvar,
        more_to_verify: &SCondvar,
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

                verification.sizes.unverified.fetch_sub(item.mem_usage(), AtomicOrdering::SeqCst);
                verifying.push_back(Verifying {
                    hash: item.hash(),
                    output: None,
                });
                item
            };

            let hash = item.hash();
            let is_ready = match K::verify(item, engine, verification.check_seal) {
                Ok(verified) => {
                    let mut verifying = verification.verifying.lock();
                    let mut idx = None;
                    for (i, e) in verifying.iter_mut().enumerate() {
                        if e.hash == hash {
                            idx = Some(i);

                            verification.sizes.verifying.fetch_add(verified.mem_usage(), AtomicOrdering::SeqCst);
                            e.output = Some(verified);
                            break
                        }
                    }

                    if idx == Some(0) {
                        // we're next!
                        let mut verified = verification.verified.lock();
                        let mut bad = verification.bad.lock();
                        VerificationQueue::drain_verifying(
                            &mut verifying,
                            &mut verified,
                            &mut bad,
                            &verification.sizes,
                        );
                        true
                    } else {
                        false
                    }
                }
                Err(_) => {
                    let mut verifying = verification.verifying.lock();
                    let mut verified = verification.verified.lock();
                    let mut bad = verification.bad.lock();

                    bad.insert(hash);
                    verifying.retain(|e| e.hash != hash);

                    if verifying.front().map_or(false, |x| x.output.is_some()) {
                        VerificationQueue::drain_verifying(
                            &mut verifying,
                            &mut verified,
                            &mut bad,
                            &verification.sizes,
                        );
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
        sizes: &Sizes,
    ) {
        let mut removed_size = 0;
        let mut inserted_size = 0;

        while let Some(output) = verifying.front_mut().and_then(|x| x.output.take()) {
            assert!(verifying.pop_front().is_some());
            let size = output.mem_usage();
            removed_size += size;

            if bad.contains(&output.parent_hash()) {
                bad.insert(output.hash());
            } else {
                inserted_size += size;
                verified.push_back(output);
            }
        }

        sizes.verifying.fetch_sub(removed_size, AtomicOrdering::SeqCst);
        sizes.verified.fetch_add(inserted_size, AtomicOrdering::SeqCst);
    }

    /// Check if the item is currently in the queue
    pub fn status(&self, hash: &H256) -> Status {
        if self.processing.read().contains_key(hash) {
            return Status::Queued
        }
        if self.verification.bad.lock().contains(hash) {
            return Status::Bad
        }
        Status::Unknown
    }

    /// Add a block to the queue.
    pub fn import(&self, input: K::Input) -> Result<H256, Error> {
        let h = input.hash();
        {
            if self.processing.read().contains_key(&h) {
                return Err(ImportError::AlreadyQueued.into())
            }

            let mut bad = self.verification.bad.lock();
            if bad.contains(&h) {
                return Err(ImportError::KnownBad.into())
            }

            if bad.contains(&input.parent_hash()) {
                bad.insert(h);
                return Err(ImportError::KnownBad.into())
            }
        }
        match K::create(input, &*self.engine) {
            Ok(item) => {
                self.verification.sizes.unverified.fetch_add(item.mem_usage(), AtomicOrdering::SeqCst);

                self.processing.write().insert(h, item.score());
                {
                    let mut ts = self.total_score.write();
                    *ts += item.score();
                }

                self.verification.unverified.lock().push_back(item);
                self.more_to_verify.notify_all();
                Ok(h)
            }
            Err(err) => {
                match err {
                    // Don't mark future blocks as bad.
                    Error::Block(BlockError::TemporarilyInvalid(_)) => {}
                    _ => {
                        self.verification.bad.lock().insert(h);
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

        let drained_size = result.iter().map(MemUsage::mem_usage).sum::<usize>();
        self.verification.sizes.verified.fetch_sub(drained_size, AtomicOrdering::SeqCst);

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
            return self.processing.read().is_empty()
        }
        let mut processing = self.processing.write();
        for hash in hashes {
            if let Some(score) = processing.remove(hash) {
                let mut td = self.total_score.write();
                *td -= score;
            }
        }
        processing.is_empty()
    }

    /// Mark given item and all its children as bad. pauses verification
    /// until complete.
    pub fn mark_as_bad(&self, hashes: &[H256]) {
        if hashes.is_empty() {
            return
        }
        let mut verified_lock = self.verification.verified.lock();
        let verified = &mut *verified_lock;
        let mut bad = self.verification.bad.lock();
        let mut processing = self.processing.write();
        bad.reserve(hashes.len());
        for hash in hashes {
            bad.insert(*hash);
            if let Some(score) = processing.remove(hash) {
                let mut td = self.total_score.write();
                *td -= score;
            }
        }

        let mut new_verified = VecDeque::new();
        let mut removed_size = 0;
        for output in verified.drain(..) {
            if bad.contains(&output.parent_hash()) {
                removed_size += output.mem_usage();
                bad.insert(output.hash());
                if let Some(score) = processing.remove(&output.hash()) {
                    let mut td = self.total_score.write();
                    *td -= score;
                }
            } else {
                new_verified.push_back(output);
            }
        }

        self.verification.sizes.verified.fetch_sub(removed_size, AtomicOrdering::SeqCst);
        *verified = new_verified;
    }

    /// Get queue status.
    pub fn queue_info(&self) -> QueueInfo {
        use std::mem::size_of;

        let (unverified_len, unverified_bytes) = {
            let len = self.verification.unverified.lock().len();
            let size = self.verification.sizes.unverified.load(AtomicOrdering::Acquire);

            (len, size + len * size_of::<K::Unverified>())
        };
        let (verifying_len, verifying_bytes) = {
            let len = self.verification.verifying.lock().len();
            let size = self.verification.sizes.verifying.load(AtomicOrdering::Acquire);
            (len, size + len * size_of::<Verifying<K>>())
        };
        let (verified_len, verified_bytes) = {
            let len = self.verification.verified.lock().len();
            let size = self.verification.sizes.verified.load(AtomicOrdering::Acquire);
            (len, size + len * size_of::<K::Verified>())
        };

        QueueInfo {
            unverified_queue_size: unverified_len,
            verifying_queue_size: verifying_len,
            verified_queue_size: verified_len,
            max_queue_size: self.max_queue_size,
            max_mem_use: self.max_mem_use,
            mem_used: unverified_bytes + verifying_bytes + verified_bytes,
        }
    }

    /// Get the total score of all the blocks in the queue.
    pub fn total_score(&self) -> U256 {
        *self.total_score.read()
    }
}

// the internal queue sizes.
struct Sizes {
    unverified: AtomicUsize,
    verifying: AtomicUsize,
    verified: AtomicUsize,
}

struct Verification<K: Kind> {
    unverified: Mutex<VecDeque<K::Unverified>>,
    verifying: Mutex<VecDeque<Verifying<K>>>,
    verified: Mutex<VecDeque<K::Verified>>,
    bad: Mutex<HashSet<H256>>,
    sizes: Sizes,
    check_seal: bool,
    #[allow(dead_code)]
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

    use super::kind::blocks::Unverified;
    use super::{BlockQueue, Config};
    use crate::error::{Error, ImportError};
    use crate::scheme::Scheme;

    // create a test block queue.
    // auto_scaling enables verifier adjustment.
    fn get_test_queue() -> BlockQueue {
        let scheme = Scheme::new_test();
        let engine = scheme.engine;

        let config = Config::default();
        BlockQueue::new(&config, engine, IoChannel::disconnected(), true)
    }

    #[test]
    fn create() {
        // TODO better test
        let scheme = Scheme::new_test();
        let engine = scheme.engine;

        let config = Config::default();
        let _ = BlockQueue::new(&config, engine, IoChannel::disconnected(), true);
    }

    #[test]
    fn import_blocks() {
        let queue = get_test_queue();
        if let Err(e) = queue.import(Unverified::new(get_good_dummy_block())) {
            panic!("error importing block that is valid by definition({:?})", e);
        }
    }

    #[test]
    fn return_error_for_duplicates() {
        let queue = get_test_queue();
        if let Err(e) = queue.import(Unverified::new(get_good_dummy_block())) {
            panic!("error importing block that is valid by definition({:?})", e);
        }

        let duplicate_import = queue.import(Unverified::new(get_good_dummy_block()));
        match duplicate_import {
            Err(e) => match e {
                Error::Import(ImportError::AlreadyQueued) => {}
                _ => {
                    panic!("must return AlreadyQueued error");
                }
            },
            Ok(_) => {
                panic!("must produce error");
            }
        }
    }
}
