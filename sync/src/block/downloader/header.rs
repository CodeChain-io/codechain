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

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use ccore::encoded::Header;
use ccore::{BlockChainClient, BlockId};
use ctypes::BlockHash;
use primitives::U256;

use super::super::message::RequestMessage;

const MAX_HEADER_REQUEST_LENGTH: u64 = 128;
const MAX_HEADER_QUEUE_LENGTH: usize = 1024;
const MAX_RETRY: usize = 3;
const MAX_WAIT: u64 = 15;

#[derive(Clone)]
pub struct HeaderDownloader {
    // NOTE: Use this member as minimum as possible.
    client: Arc<dyn BlockChainClient>,

    nonce: U256,
    best_hash: BlockHash,
    pivot: BlockHash,
    request_time: Option<Instant>,
    downloaded: HashMap<BlockHash, Header>,
    queued: HashMap<BlockHash, Header>,
    trial: usize,
}

impl HeaderDownloader {
    pub fn new(client: Arc<dyn BlockChainClient>, nonce: U256, best_hash: BlockHash) -> Self {
        let best_header_hash = client.best_block_header().hash();

        Self {
            client,

            nonce,
            best_hash,
            pivot: best_header_hash,
            request_time: None,
            downloaded: HashMap::new(),
            queued: HashMap::new(),
            trial: 0,
        }
    }

    pub fn best_hash(&self) -> BlockHash {
        self.best_hash
    }

    pub fn update(&mut self, nonce: U256, best_hash: BlockHash) -> bool {
        match self.nonce.cmp(&nonce) {
            Ordering::Equal => true,
            Ordering::Less => {
                self.nonce = nonce;
                self.best_hash = best_hash;

                if self.client.block_header(&BlockId::Hash(best_hash)).is_some() {
                    self.pivot = best_hash;
                }
                true
            }
            Ordering::Greater => false,
        }
    }

    fn is_valid(&self) -> bool {
        self.trial < MAX_RETRY
    }

    fn is_expired(&self) -> bool {
        self.request_time.map_or(false, |time| (Instant::now() - time).as_secs() > MAX_WAIT)
    }

    /// Find header from queued headers, downloaded cache and then from blockchain
    /// Panics if header dosn't exist
    fn pivot_header(&self) -> Header {
        match self.queued.get(&self.pivot) {
            Some(header) => header.clone(),
            None => match self.downloaded.get(&self.pivot) {
                Some(header) => header.clone(),
                None => self.client.block_header(&BlockId::Hash(self.pivot)).unwrap(),
            },
        }
    }

    pub fn is_idle(&self) -> bool {
        let can_request = self.request_time.is_none() && self.best_hash != self.pivot;

        self.is_valid() && (can_request || self.is_expired())
    }

    pub fn is_caught_up(&self) -> bool {
        self.pivot == self.best_hash
    }

    pub fn create_request(&mut self) -> Option<RequestMessage> {
        if !self.is_idle() {
            return None
        }
        if self.queued.len() + self.downloaded.len() > MAX_HEADER_QUEUE_LENGTH {
            return None
        }

        let pivot_number = self.pivot_header().number();

        self.request_time = Some(Instant::now());

        Some(RequestMessage::Headers {
            start_number: pivot_number,
            max_count: MAX_HEADER_REQUEST_LENGTH,
        })
    }

    /// Imports headers and mark success
    /// Expects importing headers matches requested header
    pub fn import_headers(&mut self, headers: &[Header]) {
        let first_header = headers.first().expect("First header must exist");
        let first_header_hash = first_header.hash();
        let first_header_number = first_header.number();
        let pivot_header = self.pivot_header();

        // This happens when best_hash is imported by other peer.
        if self.best_hash == self.pivot {
            ctrace!(SYNC, "Ignore received headers, pivot already reached the best hash");
        } else if first_header_hash == self.pivot {
            for header in headers.iter() {
                self.downloaded.insert(header.hash(), header.clone());
            }

            // FIXME: skip known headers
            self.pivot = headers.last().expect("Last downloaded header must exist").hash();
        } else if first_header_number < pivot_header.number() {
            ctrace!(
                SYNC,
                "Ignore received headers, pivot is already updated since headers are imported by other peers"
            );
        } else if first_header_number == pivot_header.number() {
            if pivot_header.number() != 0 {
                self.pivot = pivot_header.parent_hash();
            }
        } else {
            cerror!(
                SYNC,
                "Invalid header update state. best_hash: {}, self.pivot: {}, first_header_hash: {}",
                self.best_hash,
                self.pivot,
                first_header_hash
            );
        }

        self.request_time = None;
        self.trial = 0;
    }

    pub fn downloaded(&self) -> Vec<Header> {
        self.downloaded.values().cloned().collect()
    }

    pub fn mark_as_imported(&mut self, hashes: Vec<BlockHash>) {
        for hash in hashes {
            self.queued.remove(&hash);
            self.downloaded.remove(&hash);

            if self.best_hash == hash {
                self.pivot = hash;
            }
        }
        self.queued.shrink_to_fit();
        self.downloaded.shrink_to_fit();
    }

    pub fn mark_as_queued(&mut self, hashes: Vec<BlockHash>) {
        for hash in hashes {
            if let Some(header) = self.downloaded.remove(&hash) {
                self.queued.insert(hash, header);
            }
        }
        self.downloaded.shrink_to_fit();
    }
}
