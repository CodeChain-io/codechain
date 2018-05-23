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

use ccore::{Block, BlockNumber, Header, UnverifiedParcel};
use ctypes::H256;

use rlp::Encodable;
use triehash::ordered_trie_root;

use super::message::RequestMessage;

const MAX_BODY_REQUEST_LENGTH: usize = 32;
const MAX_HEADER_REQUEST_LENGTH: usize = 128;

pub struct DownloadManager {
    best_hash: H256,
    best_number: BlockNumber,
    headers: HashMap<H256, Header>,
    // FIXME: Find more appropriate type for block body data
    bodies: HashMap<H256, Vec<UnverifiedParcel>>,

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
                cinfo!(Sync, "Unexpected headers");
                return false
            }
        }

        // Continuity check
        for neighbors in headers.windows(2) {
            let parent = &neighbors[0];
            let child = &neighbors[1];
            if child.number() != parent.number() + 1 || *child.parent_hash() != parent.hash() {
                cinfo!(Sync, "Headers are not continuous");
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
    pub fn import_bodies(&mut self, bodies: &[Vec<UnverifiedParcel>]) -> bool {
        let mut valid_bodies = HashMap::new();
        // Validity check
        for body in bodies {
            let parcels_root = ordered_trie_root(body.iter().map(|parcel| parcel.rlp_bytes()));
            let is_valid = self.downloading_bodies
                .iter()
                .map(|hash| self.headers.get(hash).expect("Downloading body's header must be known"))
                .any(|header| *header.parcels_root() == parcels_root);
            if is_valid {
                valid_bodies.insert(parcels_root, body);
            } else {
                cinfo!(Sync, "Unexpected body detected");
                return false
            }
        }

        for (parcels_root, body) in valid_bodies {
            for header in self.headers.values().filter(|header| *header.parcels_root() == parcels_root) {
                self.bodies.insert(header.hash(), body.clone());
                self.downloading_bodies.remove(&header.hash());
            }
        }
        true
    }

    pub fn create_request(&mut self) -> Option<RequestMessage> {
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
            return Some(RequestMessage::Bodies(hashes))
        }

        // Search for needed headers
        if self.downloading_header.is_none() {
            let mut target = self.best_hash;
            while let Some(child_hash) = child_map.get(&target) {
                target = *child_hash;
            }
            self.downloading_header = Some(target);
            return Some(RequestMessage::Headers {
                start_number: if target == self.best_hash {
                    self.best_number
                } else {
                    self.headers.get(&target).expect("Header download target must be known").number()
                },
                max_count: MAX_HEADER_REQUEST_LENGTH as u64,
            })
        }
        None
    }

    pub fn mark_as_failed(&mut self, message: &RequestMessage) {
        match message {
            RequestMessage::Headers {
                ..
            } => {
                // FIXME: validate this part better
                if self.downloading_header.is_some() {
                    self.downloading_header = None;
                }
            }
            RequestMessage::Bodies(hashes) => {
                for hash in hashes {
                    self.downloading_bodies.remove(hash);
                }
            }
        }
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
                let header = self.headers.remove(child_hash).expect("Header must exist to be drained");
                self.best_hash = header.hash();
                self.best_number = header.number();
                result.push(Block {
                    header,
                    parcels: body,
                });
            } else {
                break
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use ccore::{Block, BlockNumber, Header, Parcel, Transaction, UnverifiedParcel};
    use ckeys::ECDSASignature;
    use ctypes::{H256, U256};

    use rand::{thread_rng, Rng};
    use rlp::Encodable;
    use triehash::ordered_trie_root;

    use super::{DownloadManager, MAX_BODY_REQUEST_LENGTH};
    use block::message::RequestMessage;

    struct TestEnvironment {
        chain: Vec<Block>,
        headers: Vec<Header>,
        bodies: Vec<Vec<UnverifiedParcel>>,
        first_block: Block,
        manager: DownloadManager,
    }

    fn dummy_parcel(nonce: U256) -> UnverifiedParcel {
        let raw = Parcel {
            nonce,
            fee: U256::zero(),
            transaction: Transaction::default(),
            network_id: 0,
        };
        raw.with_signature(ECDSASignature::default())
    }

    fn dummy_block(number: BlockNumber, score: U256, nonces: Range<usize>) -> Block {
        let mut body = Vec::new();
        for n in nonces {
            body.push(dummy_parcel(U256::from(n)));
        }
        let mut header = Header::default();
        header.set_parent_hash(H256::default());
        header.set_number(number);
        header.set_score(score);
        header.set_parcels_root(ordered_trie_root(body.iter().map(|parcel| parcel.rlp_bytes())));

        Block {
            header,
            parcels: body,
        }
    }

    fn dummy_chain(length: usize) -> Vec<Block> {
        let mut last_nonce = 0;
        let mut chain: Vec<Block> = Vec::new();
        for i in 0..length {
            let body_length = thread_rng().gen_range(0, 10);
            let mut new_block = dummy_block(i as u64, U256::from(i), last_nonce..(last_nonce + body_length));
            new_block.header.set_parent_hash(chain.last().map_or(H256::default(), |block| block.header.hash()));
            chain.push(new_block);
            last_nonce += body_length;
        }
        chain
    }

    fn generate_test_environment(chain_length: usize) -> TestEnvironment {
        let chain = dummy_chain(chain_length);
        let headers: Vec<_> = chain.iter().map(|block| block.header.clone()).collect();
        let bodies: Vec<_> = chain.iter().map(|block| block.parcels.clone()).collect();
        let first_block = chain.first().unwrap().clone();
        let manager = DownloadManager::new(first_block.header.hash(), first_block.header.number());

        TestEnvironment {
            chain,
            headers,
            bodies,
            first_block,
            manager,
        }
    }

    #[test]
    fn test_best_header_download() {
        let TestEnvironment {
            headers,
            first_block,
            mut manager,
            ..
        } = generate_test_environment(10);

        // Create header download request
        let request = manager.create_request();
        match request {
            Some(RequestMessage::Headers {
                start_number,
                ..
            }) => {
                assert_eq!(start_number, first_block.header.number());
                assert_eq!(manager.downloading_header, Some(first_block.header.hash()));
            }
            _ => panic!(),
        }

        // Import requested headers
        assert!(manager.import_headers(headers.as_slice()));
        for header in headers {
            assert!(manager.headers.contains_key(&header.hash()));
        }
    }

    #[test]
    fn test_last_header_download() {
        let TestEnvironment {
            headers,
            mut manager,
            ..
        } = generate_test_environment(10);

        let _ = manager.create_request();
        assert!(manager.import_headers(headers.as_slice()));
        let hashes: Vec<_> = headers.iter().map(|h| h.hash()).collect();
        manager.downloading_bodies.extend(hashes.as_slice());

        let request = manager.create_request();
        match request {
            Some(RequestMessage::Headers {
                start_number,
                ..
            }) => {
                let last_header = headers.last().unwrap();
                assert_eq!(start_number, last_header.number());
                assert_eq!(manager.downloading_header, Some(last_header.hash()));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn should_import_empty_headers() {
        let TestEnvironment {
            mut manager,
            ..
        } = generate_test_environment(10);
        assert!(!manager.import_headers(Vec::new().as_slice()));
    }

    #[test]
    fn should_not_import_non_continuous_headers() {
        let TestEnvironment {
            mut headers,
            mut manager,
            ..
        } = generate_test_environment(10);

        manager.downloading_header = Some(headers.first().unwrap().hash());
        headers.drain(3..7);
        assert!(!manager.import_headers(headers.as_slice()));
    }

    #[test]
    fn should_not_import_unknown_headers() {
        let TestEnvironment {
            headers,
            mut manager,
            ..
        } = generate_test_environment(10);
        assert!(!manager.import_headers(headers.as_slice()));

        assert_eq!(manager.headers.len(), 0);
    }

    #[test]
    fn test_body_download() {
        let TestEnvironment {
            chain,
            headers,
            mut manager,
            ..
        } = generate_test_environment(MAX_BODY_REQUEST_LENGTH + 2);

        let _ = manager.create_request();
        assert!(manager.import_headers(headers.as_slice()));

        let request = manager.create_request();
        let requested_hashes = match request {
            Some(RequestMessage::Bodies(hashes)) => {
                assert!(hashes.len() <= MAX_BODY_REQUEST_LENGTH);
                for header in headers.iter().skip(1).take(MAX_BODY_REQUEST_LENGTH) {
                    assert!(hashes.contains(&header.hash()));
                }
                hashes
            }
            _ => panic!(),
        };

        let importing_bodies: Vec<_> = chain
            .into_iter()
            .filter(|block| requested_hashes.contains(&block.header.hash()))
            .map(|block| block.parcels)
            .collect();
        assert!(manager.import_bodies(importing_bodies.as_slice()));
        for hash in requested_hashes {
            assert!(manager.bodies.contains_key(&hash));
        }
    }

    #[test]
    fn should_request_middle_bodies() {
        let TestEnvironment {
            headers,
            mut manager,
            ..
        } = generate_test_environment(10);
        let _ = manager.create_request();
        assert!(manager.import_headers(headers.as_slice()));

        let mut imported_body_hashes: Vec<_> = headers.iter().map(|header| header.hash()).collect();
        imported_body_hashes.drain(3..7);
        manager.downloading_bodies.extend(imported_body_hashes);

        let request = manager.create_request();
        match request {
            Some(RequestMessage::Bodies(hashes)) => {
                assert_eq!(hashes.len(), 7 - 3);
                for header in headers.iter().skip(3).take(7 - 3) {
                    assert!(hashes.contains(&header.hash()));
                }
            }
            _ => panic!(),
        };
    }

    #[test]
    fn should_not_import_unknown_bodies() {
        let TestEnvironment {
            bodies,
            mut manager,
            ..
        } = generate_test_environment(10);
        assert!(!manager.import_bodies(bodies.as_slice()));
        assert_eq!(manager.bodies.len(), 0);
    }

    #[test]
    fn should_not_request_anything_on_complete() {
        let TestEnvironment {
            headers,
            bodies,
            mut manager,
            ..
        } = generate_test_environment(10);
        let _ = manager.create_request();
        assert!(manager.import_headers(headers.as_slice()));
        let _ = manager.create_request();
        assert!(manager.import_bodies(&bodies[1..]));
        let _ = manager.create_request();
        assert_eq!(manager.create_request(), None);
    }

    #[test]
    fn test_drain() {
        let TestEnvironment {
            chain,
            mut manager,
            ..
        } = generate_test_environment(MAX_BODY_REQUEST_LENGTH + 2);

        let mut importing_blocks = chain.clone();
        importing_blocks.drain(3..7);
        for block in &importing_blocks {
            manager.headers.insert(block.header.hash(), block.header.clone());
            manager.bodies.insert(block.header.hash(), block.parcels.clone());
        }

        let drained_blocks = manager.drain();
        let new_first_block = &chain[2];
        assert_eq!(manager.best_hash, new_first_block.header.hash());
        assert_eq!(manager.best_number, new_first_block.header.number());
        for block in drained_blocks {
            let original_block = chain[block.header.number() as usize].clone();
            assert_eq!(original_block, block);
        }
    }
}
