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

use std::collections::{HashMap, HashSet};

use ccore::{Block, BlockNumber, Header, UnverifiedTransaction};
use ctypes::H256;

use rlp::Encodable;
use triehash::ordered_trie_root;

use message::Message;

const MAX_BODY_REQUEST_LENGTH: usize = 32;
const MAX_HEADER_REQUEST_LENGTH: usize = 128;

pub struct DownloadManager {
    best_hash: H256,
    best_number: BlockNumber,
    headers: HashMap<H256, Header>,
    // FIXME: Find more appropriate type for block body data
    bodies: HashMap<H256, Vec<UnverifiedTransaction>>,

    /// Hash of currently downloading header. Should be either included in `headers` or equal to `best_hash`
    downloading_header: Option<H256>,
    /// Hash of currently downloading bodies. All elements should be included in `headers`
    downloading_bodies: HashSet<H256>,
}

impl DownloadManager {
    pub fn best_hash(&self) -> H256 {
        self.best_hash
    }
}

impl DownloadManager {
    pub fn new(best_hash: H256, best_number: BlockNumber) -> Self {
        Self {
            best_hash,
            best_number,
            headers: HashMap::new(),
            bodies: HashMap::new(),

            downloading_header: None,
            downloading_bodies: HashSet::new(),
        }
    }

    /// Import block headers to Download Manager
    /// Headers should be sorted by block number, and connected from start to end
    /// Returns true if at least one header was imported
    pub fn import_headers(&mut self, headers: &[Header]) -> bool {
        // Empty header list is valid case
        if headers.len() == 0 {
            return false
        }

        // Validity check
        let first_header_hash = headers.first().expect("Argument `headers` has more than one element").hash();
        match self.downloading_header {
            Some(downloading) if downloading == first_header_hash => {}
            _ => {
                info!("DownloadManager: Unexpected headers");
                return false
            }
        }

        // Continuity check
        for neighbors in headers.windows(2) {
            let parent = &neighbors[0];
            let child = &neighbors[1];
            if child.number() != parent.number() + 1 || *child.parent_hash() != parent.hash() {
                info!("DownloadManager: Headers are not continuous");
                return false
            }
        }

        // Import headers
        headers.iter().for_each(|header| {
            self.headers.insert(header.hash(), header.clone());
        });
        self.downloading_header = None;
        true
    }

    /// Returns true if bodies were imported
    pub fn import_bodies(&mut self, bodies: &[Vec<UnverifiedTransaction>]) -> bool {
        let mut valid_bodies = HashMap::new();
        // Validity check
        for body in bodies {
            let tx_root = ordered_trie_root(body.iter().map(|tx| tx.rlp_bytes()));
            let is_valid = self.downloading_bodies
                .iter()
                .map(|hash| self.headers.get(hash).expect("DownloadManager: downloading body's header should be known"))
                .any(|header| *header.transactions_root() == tx_root);
            if is_valid {
                valid_bodies.insert(tx_root, body);
            } else {
                info!("DownloadManager: Unexpected body detected");
                return false
            }
        }

        for (tx_root, body) in valid_bodies {
            for header in self.headers.values().filter(|header| *header.transactions_root() == tx_root) {
                self.bodies.insert(header.hash(), body.clone());
                self.downloading_bodies.remove(&header.hash());
            }
        }
        true
    }

    pub fn create_request(&mut self) -> Option<Message> {
        // FIXME: Maintain this map as member variable
        let mut child_map = HashMap::new();
        for header in self.headers.values() {
            child_map.insert(*header.parent_hash(), header.hash());
        }

        // Search for needed bodies
        let mut hashes = Vec::new();
        let mut parent_hash = self.best_hash;
        while let Some(child_hash) = child_map.get(&parent_hash) {
            if hashes.len() >= MAX_BODY_REQUEST_LENGTH {
                break
            }
            if !self.bodies.contains_key(child_hash) && !self.downloading_bodies.contains(child_hash) {
                hashes.push(*child_hash);
            }
            parent_hash = *child_hash;
        }
        if hashes.len() > 0 {
            self.downloading_bodies.extend(&hashes);
            return Some(Message::RequestBodies(hashes))
        }

        // Search for needed headers
        if self.downloading_header.is_none() {
            let mut target = self.best_hash;
            while let Some(child_hash) = child_map.get(&target) {
                target = *child_hash;
            }
            self.downloading_header = Some(target);
            return Some(Message::RequestHeaders {
                start_number: if target == self.best_hash {
                    self.best_number
                } else {
                    self.headers.get(&target).expect("Header download target should be known").number()
                },
                max_count: MAX_HEADER_REQUEST_LENGTH as u64,
            })
        }
        None
    }

    pub fn drain(&mut self) -> Vec<Block> {
        // FIXME: Maintain this map as member variable
        let mut child_map = HashMap::new();
        for header in self.headers.values() {
            child_map.insert(*header.parent_hash(), header.hash());
        }

        let mut result = Vec::new();
        while let Some(child_hash) = child_map.get(&self.best_hash) {
            if let Some(body) = self.bodies.remove(child_hash) {
                let header = self.headers.remove(child_hash).expect("Header should exist to be drained");
                self.best_hash = header.hash();
                self.best_number = header.number();
                result.push(Block {
                    header,
                    transactions: body,
                });
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use ccore::{Block, BlockNumber, Header, UnverifiedTransaction};
    use ctypes::{H256, U256};

    use rlp::Encodable;
    use triehash::ordered_trie_root;

    use super::DownloadManager;

    fn create_dummy_block(number: BlockNumber, score: U256, body: Vec<UnverifiedTransaction>) -> Block {
        let mut header = Header::default();
        header.set_parent_hash(H256::default());
        header.set_number(number);
        header.set_score(score);
        header.set_transactions_root(ordered_trie_root(body.iter().map(|tx| tx.rlp_bytes())));

        Block {
            header,
            transactions: body,
        }
    }

    #[test]
    fn should_import_known_blocks() {
        let best_block = create_dummy_block(0, U256::from(0), Vec::new());
        let mut manager = DownloadManager::new(best_block.header.hash(), best_block.header.number());
        let mut blocks: Vec<Block> = vec![best_block];
        for i in 1..10 {
            let mut block = create_dummy_block(manager.best_number + i, U256::from(i * 2), Vec::new());
            block.header.set_parent_hash(blocks.last().unwrap().header.hash());
            blocks.push(block);
        }
        manager.downloading_header = Some(manager.best_hash);
        let headers: Vec<_> = blocks.iter().map(|block| block.header.clone()).collect();
        manager.import_headers(headers.as_slice());
        for (hash, _) in &manager.headers {
            manager.downloading_bodies.insert(*hash);
        }
        let bodies: Vec<_> = blocks.iter().map(|block| block.transactions.clone()).collect();
        manager.import_bodies(bodies.as_slice());

        for block in blocks {
            let hash = block.header.hash();
            assert!(manager.headers.contains_key(&hash));
            assert_eq!(*manager.headers.get(&hash).unwrap(), block.header);
            assert!(manager.bodies.contains_key(&hash));
            assert_eq!(*manager.bodies.get(&hash).unwrap(), block.transactions);
        }
    }

    #[test]
    fn should_not_import_unknown_headers() {
        let best_block = create_dummy_block(0, U256::from(0), Vec::new());
        let mut manager = DownloadManager::new(best_block.header.hash(), best_block.header.number());
        let mut headers: Vec<Header> = Vec::new();
        for i in 0..10 {
            let mut header = Header::default();
            header.set_number(best_block.header.number() + i);
            header.set_score(U256::from(i * 2));
            header.set_parent_hash(headers.last().map_or(manager.best_hash, |h| h.hash()));
            headers.push(header);
        }
        manager.import_headers(&headers[1..]);

        for header in headers {
            assert!(!manager.headers.contains_key(&header.hash()));
        }
    }

    #[test]
    fn should_not_import_unknown_bodies() {
        let best_block = create_dummy_block(0, U256::from(0), Vec::new());
        let mut manager = DownloadManager::new(best_block.header.hash(), best_block.header.number());
        let mut bodies = Vec::new();
        for _ in 1..10 {
            bodies.push(Vec::new());
        }
        manager.import_bodies(bodies.as_slice());

        assert_eq!(manager.bodies.len(), 0);
    }
}
