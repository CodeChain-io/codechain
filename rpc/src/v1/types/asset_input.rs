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

use cjson::uint::Uint;
use ctypes::transaction::{AssetOutPoint as AssetOutPointType, AssetTransferInput as AssetTransferInputType, Timelock};
use primitives::{Bytes, H256};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetOutPoint {
    pub transaction_id: H256,
    pub index: usize,
    pub asset_type: H256,
    pub amount: Uint,
}

impl From<AssetOutPointType> for AssetOutPoint {
    fn from(from: AssetOutPointType) -> Self {
        AssetOutPoint {
            transaction_id: from.transaction_hash,
            index: from.index,
            asset_type: from.asset_type,
            amount: from.amount.into(),
        }
    }
}

impl From<AssetOutPoint> for AssetOutPointType {
    fn from(from: AssetOutPoint) -> Self {
        AssetOutPointType {
            transaction_hash: from.transaction_id,
            index: from.index,
            asset_type: from.asset_type,
            amount: from.amount.into(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferInput {
    pub prev_out: AssetOutPoint,
    pub timelock: Option<Timelock>,
    pub lock_script: Bytes,
    pub unlock_script: Bytes,
}

impl From<AssetTransferInputType> for AssetTransferInput {
    fn from(from: AssetTransferInputType) -> Self {
        AssetTransferInput {
            prev_out: from.prev_out.into(),
            timelock: from.timelock,
            lock_script: from.lock_script,
            unlock_script: from.unlock_script,
        }
    }
}

impl From<AssetTransferInput> for AssetTransferInputType {
    fn from(from: AssetTransferInput) -> Self {
        AssetTransferInputType {
            prev_out: from.prev_out.into(),
            timelock: from.timelock,
            lock_script: from.lock_script,
            unlock_script: from.unlock_script,
        }
    }
}
