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

use std::collections::{VecDeque, HashSet};
use std::sync::Arc;

use ctypes::H256;
use parking_lot::Mutex;

use super::super::consensus::CodeChainEngine;
use super::super::error::{Error, BlockError, ImportError};
use self::kind::{BlockLike, Kind};

/// Type alias for block queue convenience.
pub type BlockQueue = VerificationQueue<kind::Blocks>;

pub struct VerificationQueue<K: Kind> {
    engine: Arc<CodeChainEngine>,
    verification: Arc<Verification<K>>,
}

impl<K: Kind> VerificationQueue<K> {
    pub fn new(engine: Arc<CodeChainEngine>) -> Self {
        let verification = Arc::new(Verification {
            unverified: Mutex::new(VecDeque::new()),
            verified: Mutex::new(VecDeque::new()),
            bad: Mutex::new(HashSet::new()),
        });
        let engine = engine.clone();
        Self {
            engine,
            verification,
        }
    }

    /// Add a block to the queue.
    pub fn import(&self, input: K::Input) -> Result<H256, Error> {
        let h = input.hash();
        {
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
}

struct Verification<K: Kind> {
    unverified: Mutex<VecDeque<K::Unverified>>,
    verified: Mutex<VecDeque<K::Verified>>,
    bad: Mutex<HashSet<H256>>,
}
