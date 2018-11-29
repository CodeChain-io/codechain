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
use ckey::{NetworkId, PlatformAddress};
use ctypes::BlockNumber;
use primitives::{H256, U256};

use super::{Action, Parcel};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    parent_hash: H256,
    timestamp: u64,
    number: u64,
    author: PlatformAddress,

    extra_data: Vec<u8>,

    parcels_root: H256,
    state_root: H256,
    invoices_root: H256,

    score: U256,
    seal: Vec<Vec<u8>>,

    hash: H256,
    parcels: Vec<Parcel>,
}

impl Block {
    pub fn from_core(block: CoreBlock, network_id: NetworkId) -> Self {
        let block_number = block.header.number();
        let block_hash = block.header.hash();
        Block {
            parent_hash: *block.header.parent_hash(),
            timestamp: block.header.timestamp(),
            number: block.header.number(),
            author: PlatformAddress::new_v1(network_id, *block.header.author()),

            extra_data: block.header.extra_data().clone(),

            parcels_root: *block.header.parcels_root(),
            state_root: *block.header.state_root(),
            invoices_root: *block.header.invoices_root(),

            score: *block.header.score(),
            seal: block.header.seal().to_vec(),

            hash: block.header.hash(),
            parcels: block
                .parcels
                .into_iter()
                .enumerate()
                .map(|(i, unverified)| {
                    let sig = unverified.signature();
                    let network_id = unverified.network_id;
                    Parcel {
                        block_number: Some(block_number),
                        block_hash: Some(block_hash),
                        parcel_index: Some(i),
                        seq: unverified.seq.into(),
                        fee: unverified.fee.into(),
                        network_id,
                        action: Action::from_core(unverified.action.clone(), network_id),
                        hash: unverified.hash(),
                        sig: sig.into(),
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockNumberAndHash {
    pub number: BlockNumber,
    pub hash: H256,
}
