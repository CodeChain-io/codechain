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

use ccore::{LocalizedTransaction, SignedTransaction};
use cjson::uint::Uint;
use ckey::{NetworkId, Signature};
use primitives::H256;

use super::ActionWithTracker;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub block_number: Option<u64>,
    pub block_hash: Option<H256>,
    pub transaction_index: Option<usize>,
    pub seq: u64,
    pub fee: Uint,
    pub network_id: NetworkId,
    pub action: ActionWithTracker,
    pub hash: H256,
    pub sig: Signature,
}

impl From<LocalizedTransaction> for Transaction {
    fn from(p: LocalizedTransaction) -> Self {
        let sig = p.signature();
        Self {
            block_number: Some(p.block_number),
            block_hash: Some(p.block_hash),
            transaction_index: Some(p.transaction_index),
            seq: p.seq,
            fee: p.fee.into(),
            network_id: p.network_id,
            action: ActionWithTracker::from_core(p.action.clone(), p.network_id),
            hash: p.hash(),
            sig,
        }
    }
}

impl From<SignedTransaction> for Transaction {
    fn from(p: SignedTransaction) -> Self {
        let sig = p.signature();
        Self {
            block_number: None,
            block_hash: None,
            transaction_index: None,
            seq: p.seq,
            fee: p.fee.into(),
            network_id: p.network_id,
            action: ActionWithTracker::from_core(p.action.clone(), p.network_id),
            hash: p.hash(),
            sig,
        }
    }
}
