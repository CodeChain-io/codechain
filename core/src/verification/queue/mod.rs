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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use cio::IoChannel;
use ctypes::{H256, U256};
use parking_lot::{Mutex, RwLock};

use super::super::consensus::CodeChainEngine;
use super::super::error::{Error, BlockError, ImportError};
use super::super::service::ClientIoMessage;
use self::kind::{BlockLike, Kind};

/// Type alias for block queue convenience.
pub type BlockQueue = VerificationQueue<kind::Blocks>;

pub struct VerificationQueue<K: Kind> {
    engine: Arc<CodeChainEngine>,
    verification: Arc<Verification<K>>,
    processing: RwLock<HashMap<H256, U256>>, // hash to score
    ready_signal: Arc<QueueSignal>,
    total_score: RwLock<U256>,
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
    pub fn new(engine: Arc<CodeChainEngine>, message_channel: IoChannel<ClientIoMessage>) -> Self {
        let verification = Arc::new(Verification {
            unverified: Mutex::new(VecDeque::new()),
            verified: Mutex::new(VecDeque::new()),
            bad: Mutex::new(HashSet::new()),
        });
        let engine = engine.clone();
        let deleting = Arc::new(AtomicBool::new(false));
        let ready_signal = Arc::new(QueueSignal {
            deleting: deleting.clone(),
            signalled: AtomicBool::new(false),
            message_channel: Mutex::new(message_channel),
        });
        Self {
            engine,
            verification,
            processing: RwLock::new(HashMap::new()),
            ready_signal,
            total_score: RwLock::new(0.into()),
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

    /// Get the total score of all the blocks in the queue.
    pub fn total_score(&self) -> U256 {
        self.total_score.read().clone()
    }
}

struct Verification<K: Kind> {
    unverified: Mutex<VecDeque<K::Unverified>>,
    verified: Mutex<VecDeque<K::Verified>>,
    bad: Mutex<HashSet<H256>>,
}
