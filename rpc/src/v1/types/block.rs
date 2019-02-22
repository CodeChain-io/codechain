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
use ctypes::BlockNumber;
use primitives::{H256, U256};

use super::Transaction;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    parent_hash: H256,
    timestamp: u64,
    number: u64,
    author: PlatformAddress,

    extra_data: Vec<u8>,

    transactions_root: H256,
    state_root: H256,
    invoices_root: H256,

    score: U256,
    seal: Vec<Vec<u8>>,

    hash: H256,
    transactions: Vec<Transaction>,
}

impl Block {
    pub fn from_core(block: CoreBlock, network_id: NetworkId, invoices: &[bool]) -> Self {
        assert_eq!(invoices.len(), block.transactions.len());
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
        Block {
            parent_hash: *block.header.parent_hash(),
            timestamp: block.header.timestamp(),
            number: block.header.number(),
            author: PlatformAddress::new_v1(network_id, *block.header.author()),

            extra_data: block.header.extra_data().clone(),

            transactions_root: *block.header.transactions_root(),
            state_root: *block.header.state_root(),
            invoices_root: *block.header.invoices_root(),

            score: *block.header.score(),
            seal: block.header.seal().to_vec(),

            hash: block.header.hash(),
            transactions: transactions.enumerate().map(|(index, tx)| Transaction::from(tx, invoices[index])).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockNumberAndHash {
    pub number: BlockNumber,
    pub hash: H256,
}
