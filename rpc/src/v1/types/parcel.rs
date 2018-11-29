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

use ccore::{LocalizedParcel, SignedParcel};
use cjson::uint::Uint;
use ckey::{NetworkId, Signature};
use primitives::H256;

use super::ActionWithTxHash;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parcel {
    pub block_number: Option<u64>,
    pub block_hash: Option<H256>,
    pub parcel_index: Option<usize>,
    pub seq: u64,
    pub fee: Uint,
    pub network_id: NetworkId,
    pub action: ActionWithTxHash,
    pub hash: H256,
    pub sig: Signature,
}

impl From<LocalizedParcel> for Parcel {
    fn from(p: LocalizedParcel) -> Self {
        let sig = p.signature();
        Self {
            block_number: Some(p.block_number),
            block_hash: Some(p.block_hash),
            parcel_index: Some(p.parcel_index),
            seq: p.seq,
            fee: p.fee.into(),
            network_id: p.network_id,
            action: ActionWithTxHash::from_core(p.action.clone(), p.network_id),
            hash: p.hash(),
            sig,
        }
    }
}

impl From<SignedParcel> for Parcel {
    fn from(p: SignedParcel) -> Self {
        let sig = p.signature();
        Self {
            block_number: None,
            block_hash: None,
            parcel_index: None,
            seq: p.seq,
            fee: p.fee.into(),
            network_id: p.network_id,
            action: ActionWithTxHash::from_core(p.action.clone(), p.network_id),
            hash: p.hash(),
            sig,
        }
    }
}
