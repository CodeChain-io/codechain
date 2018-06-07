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

use std::sync::Arc;
use std::time::Instant;

use ccore::encoded::Header;
use ccore::{BlockChainClient, BlockId, BlockNumber};
use ctypes::{H256, U256};

use super::message::RequestMessage;

const MAX_HEADER_REQUEST_LENGTH: u64 = 128;
const MAX_RETRY: usize = 3;
const MAX_WAIT: u64 = 15;

#[derive(Clone)]
struct RequestInfo {
    number: BlockNumber,
    total_score: U256,
    time: Instant,
}

#[derive(Clone)]
pub struct Peer {
    // NOTE: Use this member as minimum as possible.
    client: Arc<BlockChainClient>,

    total_score: U256,
    best_hash: H256,

    pivot: H256,
    last_request: Option<RequestInfo>,
    downloaded: Vec<Header>,
    trial: usize,
}

impl Peer {
    pub fn new(client: Arc<BlockChainClient>, total_score: U256, best_hash: H256) -> Self {
        let best_header_hash = client.best_block_header().hash();

        Self {
            client,

            total_score,
            best_hash,

            pivot: best_header_hash,
            last_request: None,
            downloaded: Vec::new(),
            trial: 0,
        }
    }

    pub fn update(&mut self, total_score: U256, best_hash: H256) {
        self.total_score = total_score;
        self.best_hash = best_hash;
    }

    fn is_valid(&self) -> bool {
        self.trial < MAX_RETRY
    }

    fn is_expired(&self) -> bool {
        if let Some(info) = &self.last_request {
            (Instant::now() - info.time).as_secs() > MAX_WAIT
        } else {
            false
        }
    }

    pub fn is_idle(&self) -> bool {
        let pivot_total_score =
            self.client.block_total_score(BlockId::Hash(self.pivot)).expect("Pivot must exist in chain");

        let can_request = self.last_request.is_none() && self.total_score > pivot_total_score;

        self.is_valid() && (can_request || self.is_expired())
    }

    pub fn create_request(&mut self) -> Option<RequestMessage> {
        if !self.is_idle() {
            return None
        }

        let pivot_number = self.client.block_number(BlockId::Hash(self.pivot)).expect("Pivot must exist in chain");
        let pivot_score = self.client.block_total_score(BlockId::Hash(self.pivot)).expect("Pivot must exist in chain");
        self.last_request = Some(RequestInfo {
            number: pivot_number,
            total_score: pivot_score,
            time: Instant::now(),
        });
        Some(RequestMessage::Headers {
            start_number: pivot_number,
            max_count: MAX_HEADER_REQUEST_LENGTH,
        })
    }

    /// Imports headers and mark success
    /// Expects importing headers matches requested header
    pub fn import_headers(&mut self, headers: Vec<Header>) {
        let first_header_hash = headers.first().expect("First header must exist").hash();
        if first_header_hash == self.pivot {
            self.downloaded.extend(headers);
            self.downloaded.sort_unstable_by_key(|h| h.number());

            // FIXME: skip known headers
            self.pivot = self.downloaded.last().expect("Last downloaded header must exist").hash();
        } else {
            self.downloaded.drain(..);
            self.pivot =
                self.client.block_header(BlockId::Hash(self.pivot)).expect("Pivot must exist in chain").parent_hash();
        }

        self.last_request = None;
        self.trial = 0;
    }

    pub fn drain(&mut self) -> Vec<Header> {
        self.downloaded.drain(..).collect()
    }
}
