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
extern crate rustc_serialize;

use std::iter::FromIterator;

use cjson::uint::Uint;
use ctypes::transaction::{AssetMintOutput as AssetMintOutputType, AssetTransferOutput as AssetTransferOutputType};
use ctypes::ShardId;
use primitives::H160;
use rustc_serialize::hex::{FromHex, FromHexError, ToHex};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<String>,
    pub asset_type: H160,
    pub shard_id: ShardId,
    pub quantity: Uint,
}

impl From<AssetTransferOutputType> for AssetTransferOutput {
    fn from(from: AssetTransferOutputType) -> Self {
        AssetTransferOutput {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters.iter().map(|bytes| bytes.to_hex()).collect(),
            asset_type: from.asset_type,
            shard_id: from.shard_id,
            quantity: from.quantity.into(),
        }
    }
}

impl From<AssetTransferOutput> for Result<AssetTransferOutputType, FromHexError> {
    fn from(from: AssetTransferOutput) -> Self {
        Ok(AssetTransferOutputType {
            lock_script_hash: from.lock_script_hash,
            parameters: Result::from_iter(from.parameters.iter().map(|hexstr| hexstr.from_hex()))?,
            asset_type: from.asset_type,
            shard_id: from.shard_id,
            quantity: from.quantity.into(),
        })
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMintOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<String>,
    pub supply: Option<Uint>,
}

impl From<AssetMintOutputType> for AssetMintOutput {
    fn from(from: AssetMintOutputType) -> Self {
        AssetMintOutput {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters.iter().map(|bytes| bytes.to_hex()).collect(),
            supply: from.supply.map(|supply| supply.into()),
        }
    }
}

impl From<AssetMintOutput> for Result<AssetMintOutputType, FromHexError> {
    fn from(from: AssetMintOutput) -> Self {
        Ok(AssetMintOutputType {
            lock_script_hash: from.lock_script_hash,
            parameters: Result::from_iter(from.parameters.iter().map(|hexstr| hexstr.from_hex()))?,
            supply: from.supply.map(|supply| supply.into()),
        })
    }
}
