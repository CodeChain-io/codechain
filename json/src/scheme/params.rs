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

use ckey::NetworkId;

use crate::uint::Uint;

/// Scheme params.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    /// Maximum size of extra data.
    pub max_extra_data_size: Uint,
    /// Maximum size of metadata of AssetScheme.
    pub max_asset_scheme_metadata_size: Uint,
    /// Maximum size of metadata of TransferAsset.
    pub max_transfer_metadata_size: Uint,
    /// Maximum size of the content of text used in store/remove actions.
    pub max_text_content_size: Uint,
    /// Network id.
    #[serde(rename = "networkID")]
    pub network_id: NetworkId,

    /// Minimum transaction cost.
    pub min_pay_cost: Uint,
    pub min_set_regular_key_cost: Uint,
    pub min_create_shard_cost: Uint,
    pub min_set_shard_owners_cost: Uint,
    pub min_set_shard_users_cost: Uint,
    pub min_wrap_ccc_cost: Uint,
    pub min_custom_cost: Uint,
    pub min_store_cost: Uint,
    pub min_remove_cost: Uint,
    pub min_mint_asset_cost: Uint,
    pub min_transfer_asset_cost: Uint,
    pub min_change_asset_scheme_cost: Uint,
    pub min_compose_asset_cost: Uint,
    pub min_decompose_asset_cost: Uint,
    pub min_unwrap_ccc_cost: Uint,

    /// Maximum size of block body.
    pub max_body_size: Uint,
    /// Snapshot creation period in unit of block numbers.
    pub snapshot_period: Uint,
}

#[cfg(test)]
mod tests {
    use primitives::U256;
    use serde_json;

    use super::Params;
    use crate::uint::Uint;

    #[test]
    fn params_deserialization() {
        let s = r#"{
            "maxExtraDataSize": "0x20",
            "maxAssetSchemeMetadataSize": "0x0400",
            "maxTransferMetadataSize": "0x0100",
            "maxTextContentSize": "0x0200",
            "networkID" : "tc",
            "minPayCost" : 10,
            "minSetRegularKeyCost" : 11,
            "minCreateShardCost" : 12,
            "minSetShardOwnersCost" : 13,
            "minSetShardUsersCost" : 14,
            "minWrapCccCost" : 15,
            "minCustomCost" : 16,
            "minStoreCost" : 17,
            "minRemoveCost" : 18,
            "minMintAssetCost" : 19,
            "minTransferAssetCost" : 20,
            "minChangeAssetSchemeCost" : 21,
            "minComposeAssetCost" : 22,
            "minDecomposeAssetCost" : 23,
            "minUnwrapCccCost" : 24,
            "maxBodySize" : 4194304,
            "snapshotPeriod": 16384
        }"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.max_extra_data_size, Uint(U256::from(0x20)));
        assert_eq!(deserialized.max_asset_scheme_metadata_size, Uint(U256::from(0x0400)));
        assert_eq!(deserialized.max_transfer_metadata_size, Uint(U256::from(0x0100)));
        assert_eq!(deserialized.max_text_content_size, Uint(U256::from(0x0200)));
        assert_eq!(deserialized.network_id, "tc".into());
        assert_eq!(deserialized.min_pay_cost, Uint(10.into()));
        assert_eq!(deserialized.min_set_regular_key_cost, Uint(11.into()));
        assert_eq!(deserialized.min_create_shard_cost, Uint(12.into()));
        assert_eq!(deserialized.min_set_shard_owners_cost, Uint(13.into()));
        assert_eq!(deserialized.min_set_shard_users_cost, Uint(14.into()));
        assert_eq!(deserialized.min_wrap_ccc_cost, Uint(15.into()));
        assert_eq!(deserialized.min_custom_cost, Uint(16.into()));
        assert_eq!(deserialized.min_store_cost, Uint(17.into()));
        assert_eq!(deserialized.min_remove_cost, Uint(18.into()));
        assert_eq!(deserialized.min_mint_asset_cost, Uint(19.into()));
        assert_eq!(deserialized.min_transfer_asset_cost, Uint(20.into()));
        assert_eq!(deserialized.min_change_asset_scheme_cost, Uint(21.into()));
        assert_eq!(deserialized.min_compose_asset_cost, Uint(22.into()));
        assert_eq!(deserialized.min_decompose_asset_cost, Uint(23.into()));
        assert_eq!(deserialized.min_unwrap_ccc_cost, Uint(24.into()));
        assert_eq!(deserialized.max_body_size, Uint(4_194_304.into()));
        assert_eq!(deserialized.snapshot_period, Uint(16_384.into()));
    }
}
