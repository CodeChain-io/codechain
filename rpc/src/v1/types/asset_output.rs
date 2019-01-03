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
use ctypes::transaction::{AssetMintOutput as AssetMintOutputType, AssetTransferOutput as AssetTransferOutputType};
use primitives::{Bytes, H160, H256};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub asset_type: H256,
    pub amount: Uint,
}

impl From<AssetTransferOutputType> for AssetTransferOutput {
    fn from(from: AssetTransferOutputType) -> Self {
        AssetTransferOutput {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
            asset_type: from.asset_type,
            amount: from.amount.into(),
        }
    }
}

impl From<AssetTransferOutput> for AssetTransferOutputType {
    fn from(from: AssetTransferOutput) -> Self {
        AssetTransferOutputType {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
            asset_type: from.asset_type,
            amount: from.amount.into(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMintOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub amount: Option<Uint>,
}

impl From<AssetMintOutputType> for AssetMintOutput {
    fn from(from: AssetMintOutputType) -> Self {
        AssetMintOutput {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
            amount: from.amount.map(|amount| amount.into()),
        }
    }
}

impl From<AssetMintOutput> for AssetMintOutputType {
    fn from(from: AssetMintOutput) -> Self {
        AssetMintOutputType {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
            amount: from.amount.map(|amount| amount.into()),
        }
    }
}
