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

use ccore::{Block as CoreBlock, LocalizedTransaction};
use ckey::{NetworkId, PlatformAddress};
use ctypes::{BlockHash, BlockNumber};
use primitives::{H256, U256};

use super::Transaction;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderAndTxCount {
    parent_hash: BlockHash,
    timestamp: u64,
    number: u64,
    author: PlatformAddress,
    score: U256,
    seal: Vec<Vec<u8>>,
    hash: BlockHash,
    transaction_count: u32,
}

impl HeaderAndTxCount {
    pub fn from_core(block: CoreBlock, network_id: NetworkId) -> Self {
        let block_number = block.header.number();
        let block_hash = block.header.hash();
        let transactions =
            block.transactions.into_iter().enumerate().map(|(transaction_index, signed)| LocalizedTransaction {
                signed,
                block_number,
                block_hash,
                transaction_index,
                cached_signer_public: None,
            });
        HeaderAndTxCount {
            parent_hash: *block.header.parent_hash(),
            timestamp: block.header.timestamp(),
            number: block.header.number(),
            author: PlatformAddress::new_v1(network_id, *block.header.author()),
            score: *block.header.score(),
            seal: block.header.seal().to_vec(),
            hash: block.header.hash(),
            transaction_count: transactions.len() as u32,
        }
    }
}

