// Copyright 2018-2020 Kodebox, Inc.
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

use super::super::message::RequestMessage;
use ccore::UnverifiedTransaction;
use ctypes::{BlockHash, Header};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::mem::replace;

#[derive(Debug, PartialEq)]
enum State {
    Queued,
    Downloading,
    Downloaded {
        transactions: Vec<UnverifiedTransaction>,
    },
    Drained,
}

impl Default for State {
    fn default() -> Self {
        State::Queued
    }
}

#[derive(Clone)]
struct Target {
    hash: BlockHash,
    is_empty: bool,
}

#[derive(Default)]
pub struct BodyDownloader {
    targets: Vec<Target>,
    states: HashMap<BlockHash, State>,
}

impl BodyDownloader {
    pub fn create_request(&mut self) -> Option<RequestMessage> {
        const MAX_BODY_REQEUST_LENGTH: usize = 128;
        let mut hashes = Vec::new();
        for t in &self.targets {
            let state = self.states.entry(t.hash).or_default();
            if *state != State::Queued {
                continue
            }
            *state = State::Downloading;
            hashes.push(t.hash);
            if hashes.len() >= MAX_BODY_REQEUST_LENGTH {
                break
            }
        }
        if hashes.is_empty() {
            None
        } else {
            Some(RequestMessage::Bodies(hashes))
        }
    }

    pub fn import_bodies(&mut self, hashes: Vec<BlockHash>, bodies: Vec<Vec<UnverifiedTransaction>>) {
        assert_eq!(hashes.len(), bodies.len());
        for (hash, transactions) in hashes.into_iter().zip(bodies) {
            if let Some(state) = self.states.get_mut(&hash) {
                if state != &State::Downloading {
                    continue
                }
                *state = State::Downloaded {
                    transactions,
                }
            }
        }
    }

    pub fn get_target_hashes(&self) -> Vec<BlockHash> {
        self.targets.iter().map(|t| t.hash).collect()
    }

    pub fn add_target(&mut self, header: &Header, is_empty: bool) {
        cdebug!(SYNC, "Add download target: {}", header.hash());
        self.states.insert(header.hash(), State::Queued);
        self.targets.push(Target {
            hash: header.hash(),
            is_empty,
        });
    }

    pub fn remove_targets(&mut self, targets: &[BlockHash]) {
        if targets.is_empty() {
            return
        }
        cdebug!(SYNC, "Remove download targets: {:?}", targets);
        // XXX: It can be slow.
        self.states.retain(|hash, _| !targets.contains(hash));
        self.targets.retain(|target| !targets.contains(&target.hash));
        self.states.shrink_to_fit();
        self.targets.shrink_to_fit();
    }

    pub fn reset_downloading(&mut self, hashes: &[BlockHash]) {
        cdebug!(SYNC, "Remove downloading by timeout {:?}", hashes);
        for hash in hashes {
            if let Some(state) = self.states.get_mut(hash) {
                if *state == State::Downloading {
                    *state = State::Queued;
                }
            }
        }
    }

    pub fn drain(&mut self) -> Vec<(BlockHash, Vec<UnverifiedTransaction>)> {
        let mut result = Vec::new();
        for t in &self.targets {
            let entry = self.states.entry(t.hash);
            let state = match entry {
                Entry::Vacant(_) => unreachable!(),
                Entry::Occupied(mut entry) => match entry.get_mut() {
                    state @ State::Downloaded {
                        ..
                    } => replace(state, State::Drained),
                    _ => break,
                },
            };
            match state {
                State::Downloaded {
                    transactions,
                } => {
                    result.push((t.hash, transactions));
                }
                _ => unreachable!(),
            }
        }
        result
    }

    pub fn re_request(&mut self, hash: BlockHash, remains: Vec<(BlockHash, Vec<UnverifiedTransaction>)>) {
        #[inline]
        fn insert(states: &mut HashMap<BlockHash, State>, hash: BlockHash, state: State) {
            let old = states.insert(hash, state);
            debug_assert_ne!(None, old);
        }
        // The implementation of extend method allocates an additional memory for new items.
        // However, our implementation guarantees that new items are already in the map and it just
        // update the states. So iterating over new items and calling the insert method is faster
        // than using the extend method and uses less memory.
        for (hash, transactions) in remains {
            insert(&mut self.states, hash, State::Downloaded {
                transactions,
            });
        }
        insert(&mut self.states, hash, State::Queued);
    }
}
