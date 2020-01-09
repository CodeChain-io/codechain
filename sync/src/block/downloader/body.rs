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

use super::super::message::RequestMessage;
use ccore::UnverifiedTransaction;
use ctypes::{BlockHash, Header};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct Target {
    hash: BlockHash,
    is_empty: bool,
}

#[derive(Default)]
pub struct BodyDownloader {
    targets: Vec<Target>,
    downloading: HashSet<BlockHash>,
    downloaded: HashMap<BlockHash, Vec<UnverifiedTransaction>>,
}

impl BodyDownloader {
    pub fn create_request(&mut self) -> Option<RequestMessage> {
        const MAX_BODY_REQEUST_LENGTH: usize = 128;
        let mut hashes = Vec::new();
        for t in &self.targets {
            if !self.downloading.contains(&t.hash) && !self.downloaded.contains_key(&t.hash) {
                hashes.push(t.hash);
            }
            if hashes.len() >= MAX_BODY_REQEUST_LENGTH {
                break
            }
        }
        if hashes.is_empty() {
            None
        } else {
            self.downloading.extend(&hashes);
            Some(RequestMessage::Bodies(hashes))
        }
    }

    pub fn import_bodies(&mut self, hashes: Vec<BlockHash>, bodies: Vec<Vec<UnverifiedTransaction>>) {
        for (hash, body) in hashes.into_iter().zip(bodies) {
            if self.downloading.remove(&hash) {
                if body.is_empty() {
                    let target = self.targets.iter().find(|t| t.hash == hash).expect("Downloading target must exist");
                    if !target.is_empty {
                        continue
                    }
                }
                self.downloaded.insert(hash, body);
            }
        }
        self.downloading.shrink_to_fit();
    }

    pub fn add_target(&mut self, header: &Header, is_empty: bool) {
        cdebug!(SYNC, "Add download target: {}", header.hash());
        self.targets.push(Target {
            hash: header.hash(),
            is_empty,
        });
    }

    pub fn remove_target(&mut self, targets: &[BlockHash]) {
        if targets.is_empty() {
            return
        }
        cdebug!(SYNC, "Remove download targets: {:?}", targets);
        for hash in targets {
            if let Some(index) = self.targets.iter().position(|t| t.hash == *hash) {
                self.targets.remove(index);
            }
            self.downloading.remove(hash);
            self.downloaded.remove(hash);
        }
        self.targets.shrink_to_fit();
        self.downloading.shrink_to_fit();
        self.downloaded.shrink_to_fit();
    }

    pub fn reset_downloading(&mut self, hashes: &[BlockHash]) {
        cdebug!(SYNC, "Remove downloading by timeout {:?}", hashes);
        for hash in hashes {
            self.downloading.remove(&hash);
        }
        self.downloading.shrink_to_fit();
    }

    pub fn drain(&mut self) -> Vec<(BlockHash, Vec<UnverifiedTransaction>)> {
        let mut result = Vec::new();
        for t in &self.targets {
            if let Some(body) = self.downloaded.remove(&t.hash) {
                result.push((t.hash, body));
            } else {
                break
            }
        }
        self.downloaded.shrink_to_fit();
        self.targets.drain(0..result.len());
        self.targets.shrink_to_fit();
        result
    }
}
