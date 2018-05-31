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

use ccore::Block as CoreBlock;
use ctypes::{H160, H256, U256};

use super::Parcel;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    parent_hash: H256,
    timestamp: u64,
    number: u64,
    author: H160,

    extra_data: Vec<u8>,

    parcels_root: H256,
    state_root: H256,
    invoices_root: H256,

    score: U256,
    seal: Vec<Vec<u8>>,

    hash: H256,
    parcels: Vec<Parcel>,
}

impl From<CoreBlock> for Block {
    fn from(block: CoreBlock) -> Self {
        let block_number = block.header.number();
        let block_hash = block.header.hash();
        Block {
            parent_hash: block.header.parent_hash().clone(),
            timestamp: block.header.timestamp(),
            number: block.header.number(),
            author: block.header.author().clone(),

            extra_data: block.header.extra_data().clone(),

            parcels_root: block.header.parcels_root().clone(),
            state_root: block.header.state_root().clone(),
            invoices_root: block.header.invoices_root().clone(),

            score: block.header.score().clone(),
            seal: block.header.seal().clone().to_vec(),

            hash: block.header.hash(),
            parcels: block
                .parcels
                .into_iter()
                .enumerate()
                .map(|(i, unverified)| {
                    let sig = unverified.signature();
                    Parcel {
                        block_number: Some(block_number),
                        block_hash: Some(block_hash),
                        parcel_index: Some(i),
                        nonce: unverified.as_unsigned().nonce.clone(),
                        fee: unverified.as_unsigned().fee.clone(),
                        transactions: unverified.as_unsigned().transactions.clone(),
                        network_id: unverified.as_unsigned().network_id,
                        hash: unverified.hash(),
                        v: sig.v(),
                        r: sig.r().into(),
                        s: sig.s().into(),
                    }
                })
                .collect(),
        }
    }
}
