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

use ccore::{BlockNumber, Header, UnverifiedTransaction};
use ctypes::H256;

use rlp::Encodable;
use triehash::ordered_trie_root;

const MAX_BUFFER_LENGTH: BlockNumber = 32 * 1024;

pub struct DownloadManager {
    best_hash: H256,
    best_number: BlockNumber,
    hashes: HashSet<H256>,
    headers: HashMap<H256, Header>,
    // FIXME: Find more appropriate type for block body data
    bodies: HashMap<H256, Vec<UnverifiedTransaction>>,
}

impl DownloadManager {
    pub fn new(best_hash: H256, best_number: BlockNumber) -> Self {
        Self {
            best_hash,
            best_number,
            hashes: HashSet::new(),
            headers: HashMap::new(),
            bodies: HashMap::new(),
        }
    }

    pub fn import_hashes(&mut self, hashes: Vec<H256>) {
        hashes.into_iter().for_each(|hash| { self.hashes.insert(hash); });
    }

    pub fn import_headers(&mut self, headers: Vec<Header>) {
        if headers.len() != 0 && !headers.iter().any(|h| self.hashes.contains(&h.hash())) {
            info!("DownloadManager: Unexpected headers");
            return;
        }
        for header in headers {
            let hash = header.hash();
            if header.number() <= self.best_number + MAX_BUFFER_LENGTH {
                self.hashes.insert(hash);
                self.headers.insert(hash, header);
            } else {
                self.hashes.remove(&hash);
            }
        }
    }

    pub fn import_bodies(&mut self, bodies: Vec<Vec<UnverifiedTransaction>>) {
        let mut valid_bodies = HashMap::new();
        // Validity check
        for body in bodies {
            let tx_root = ordered_trie_root(body.iter().map(|tx| tx.rlp_bytes()));
            if self.headers.values().any(|header| *header.transactions_root() == tx_root) {
                valid_bodies.insert(tx_root, body);
            } else {
                info!("DownloadManager: Unexpected body detected");
                return;
            }
        }

        for (tx_root, body) in valid_bodies {
            for header in self.headers.values().filter(|header| *header.transactions_root() == tx_root) {
                self.bodies.insert(header.hash(), body.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ccore::{Header, UnverifiedTransaction};
    use ctypes::{H256, U256};

    use rlp::Encodable;
    use triehash::ordered_trie_root;

    use super::DownloadManager;

    #[test]
    fn should_import_known_blocks() {
        let best_hash = H256::default();
        let best_number = 0;
        let mut manager = DownloadManager::new(best_hash, best_number);
        let mut blocks = Vec::new();
        for i in 1..10 {
            let mut header = Header::default();
            let body: Vec<UnverifiedTransaction> = Vec::new();
            let tx_root = ordered_trie_root(body.iter().map(|tx| tx.rlp_bytes()));
            header.set_number(best_number + i);
            header.set_score(U256::from(i * 2));
            header.set_transactions_root(tx_root);
            blocks.push((header, body));
        }
        manager.import_hashes(vec![blocks.first().unwrap().0.hash()]);
        manager.import_headers(blocks.iter().map(|&(ref header, _)| header.clone()).collect());
        manager.import_bodies(blocks.iter().map(|&(_, ref body)| body.clone()).collect());

        for (header, body) in blocks {
            let hash = header.hash();
            assert!(manager.hashes.contains(&hash));
            assert!(manager.headers.contains_key(&hash));
            assert_eq!(*manager.headers.get(&hash).unwrap(), header);
            assert!(manager.bodies.contains_key(&hash));
            assert_eq!(*manager.bodies.get(&hash).unwrap(), body);
        }
    }

    #[test]
    fn should_not_import_unknown_headers() {
        let best_hash = H256::default();
        let best_number = 0;
        let mut manager = DownloadManager::new(best_hash, best_number);
        let mut headers = Vec::new();
        for i in 1..10 {
            let mut header = Header::default();
            header.set_number(best_number + i);
            header.set_score(U256::from(i * 2));
            headers.push(header);
        }
        manager.import_headers(headers.clone());

        for header in headers {
            assert!(!manager.headers.contains_key(&header.hash()));
        }
    }

    #[test]
    fn should_not_import_too_far_headers() {
        let best_hash = H256::default();
        let best_number = 0;
        let mut manager = DownloadManager::new(best_hash, best_number);
        let mut headers = Vec::new();
        for i in 1..10 {
            let mut header = Header::default();
            header.set_number(best_number + i + super::MAX_BUFFER_LENGTH);
            header.set_score(U256::from(i * 2));
            headers.push(header);
        }
        manager.import_hashes(vec![headers.first().unwrap().hash()]);
        manager.import_headers(headers.clone());

        for header in headers {
            assert!(!manager.headers.contains_key(&header.hash()));
        }
    }

    #[test]
    fn should_not_import_unknown_bodies() {
        let best_hash = H256::default();
        let best_number = 0;
        let mut manager = DownloadManager::new(best_hash, best_number);
        let mut bodies = Vec::new();
        for _ in 1..10 {
            bodies.push(Vec::new());
        }
        manager.import_bodies(bodies.clone());

        assert_eq!(manager.bodies.len(), 0);
    }
}
