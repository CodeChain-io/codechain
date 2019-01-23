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
use ckey::{NetworkId, PlatformAddress};
use cstate::AssetScheme as AssetSchemeType;
use primitives::H160;

use super::Asset;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetScheme {
    metadata: String,
    supply: Uint,
    approver: Option<PlatformAddress>,
    administrator: Option<PlatformAddress>,
    allowed_script_hashes: Vec<H160>,
    pool: Vec<Asset>,
}

impl AssetScheme {
    pub fn from_core(asset_scheme: AssetSchemeType, network_id: NetworkId) -> Self {
        Self {
            metadata: asset_scheme.metadata().clone(),
            supply: asset_scheme.supply().into(),
            approver: asset_scheme.approver().as_ref().map(|approver| PlatformAddress::new_v1(network_id, *approver)),
            administrator: asset_scheme
                .administrator()
                .as_ref()
                .map(|administrator| PlatformAddress::new_v1(network_id, *administrator)),
            allowed_script_hashes: asset_scheme.allowed_script_hashes().to_owned(),
            pool: asset_scheme.pool().iter().map(|asset| asset.clone().into()).collect(),
        }
    }
}
