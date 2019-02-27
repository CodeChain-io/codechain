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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use ccore::encoded::Header;
use ccore::{BlockChainClient, BlockId};
use primitives::{H256, U256};

use super::super::message::RequestMessage;

const MAX_HEADER_REQUEST_LENGTH: u64 = 128;
const MAX_RETRY: usize = 3;
const MAX_WAIT: u64 = 15;

#[derive(Clone)]
struct Pivot {
    hash: H256,
    total_score: U256,
}

#[derive(Clone)]
pub struct HeaderDownloader {
    // NOTE: Use this member as minimum as possible.
    client: Arc<BlockChainClient>,

    total_score: U256,
    best_hash: H256,

    pivot: Pivot,
    request_time: Option<Instant>,
    downloaded: HashMap<H256, Header>,
    trial: usize,
}

impl HeaderDownloader {
    pub fn total_score(&self) -> U256 {
        self.total_score
    }

    pub fn new(client: Arc<BlockChainClient>, total_score: U256, best_hash: H256) -> Self {
        let best_header_hash = client.best_block_header().hash();
        let best_score = client.block_total_score(&BlockId::Latest).expect("Best block always exist");

        Self {
            client,

            total_score,
            best_hash,

            pivot: Pivot {
                hash: best_header_hash,
                total_score: best_score,
            },
            request_time: None,
            downloaded: HashMap::new(),
            trial: 0,
        }
    }

    pub fn update(&mut self, total_score: U256, best_hash: H256) -> bool {
        if self.total_score < total_score {
            self.total_score = total_score;
            self.best_hash = best_hash;
            true
        } else {
            false
        }
    }

    fn is_valid(&self) -> bool {
        self.trial < MAX_RETRY
    }

    fn is_expired(&self) -> bool {
        self.request_time.map_or(false, |time| (Instant::now() - time).as_secs() > MAX_WAIT)
    }

    /// Find header from download cache, and then from blockchain
    /// Panics if header dosn't exist
    fn pivot_header(&self) -> Header {
        match self.downloaded.get(&self.pivot.hash) {
            Some(header) => header.clone(),
            None => self.client.block_header(&BlockId::Hash(self.pivot.hash)).unwrap(),
        }
    }

    pub fn is_idle(&self) -> bool {
        let can_request = self.request_time.is_none() && self.total_score > self.pivot.total_score;

        self.is_valid() && (can_request || self.is_expired())
    }

    pub fn create_request(&mut self) -> Option<RequestMessage> {
        if !self.is_idle() {
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
        let first_header_hash = headers.first().expect("First header must exist").hash();
        if first_header_hash == self.pivot.hash {
            for header in headers.iter() {
                self.downloaded.insert(header.hash(), header.clone());
            }

            // FIXME: skip known headers
            let new_scores = headers[1..].iter().fold(U256::zero(), |acc, header| acc + header.score());
            self.pivot = Pivot {
                hash: headers.last().expect("Last downloaded header must exist").hash(),
                total_score: self.pivot.total_score + new_scores,
            }
        } else {
            let pivot_header = self.pivot_header();
            if pivot_header.number() != 0 {
                self.pivot = Pivot {
                    hash: pivot_header.parent_hash(),
                    total_score: self.pivot.total_score - pivot_header.score(),
                }
            }
        }

        self.request_time = None;
        self.trial = 0;
    }

    pub fn downloaded(&self) -> Vec<Header> {
        self.downloaded.values().cloned().collect()
    }

    pub fn mark_as_imported(&mut self, hashes: Vec<H256>) {
        for hash in hashes {
            self.downloaded.remove(&hash);
        }
    }
}
