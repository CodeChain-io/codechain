// Copyright 2019 Kodebox, Inc.
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
use cstate::{Asset as AssetType, OwnedAsset as OwnedAssetType};
use primitives::{H160, H256};
use rustc_serialize::hex::ToHex;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    asset_type: H160,
    quantity: Uint,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedAsset {
    #[serde(flatten)]
    asset: Asset,
    lock_script_hash: H160,
    parameters: Vec<String>,
    order_hash: Option<H256>,
}

impl From<AssetType> for Asset {
    fn from(asset: AssetType) -> Self {
        Self {
            asset_type: *asset.asset_type(),
            quantity: asset.quantity().into(),
        }
    }
}

impl From<OwnedAssetType> for OwnedAsset {
    fn from(asset: OwnedAssetType) -> Self {
        Self {
            asset: Asset {
                asset_type: *asset.asset_type(),
                quantity: asset.quantity().into(),
            },
            lock_script_hash: *asset.lock_script_hash(),
            order_hash: *asset.order_hash(),
            parameters: asset.parameters().iter().map(|bytes| bytes.to_hex()).collect(),
        }
    }
}
