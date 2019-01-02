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

use ckey::NetworkId;

use crate::uint::Uint;

/// Scheme params.
#[derive(Debug, PartialEq, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    /// Maximum size of extra data.
    pub max_extra_data_size: Uint,
    /// Maximum size of metadata.
    pub max_metadata_size: Uint,
    /// Maximum size of the content of text used in store/remove actions.
    pub max_text_content_size: Uint,
    /// Network id.
    #[serde(rename = "networkID")]
    pub network_id: NetworkId,

    /// Minimum parcel cost.
    pub min_pay_parcel_cost: Uint,
    pub min_set_regular_key_parcel_cost: Uint,
    pub min_create_shard_parcel_cost: Uint,
    pub min_set_shard_owners_parcel_cost: Uint,
    pub min_set_shard_users_parcel_cost: Uint,
    pub min_wrap_ccc_parcel_cost: Uint,
    pub min_custom_parcel_cost: Uint,
    pub min_store_parcel_cost: Uint,
    pub min_remove_parcel_cost: Uint,
    pub min_asset_mint_cost: Uint,
    pub min_asset_transfer_cost: Uint,
    pub min_asset_scheme_change_cost: Uint,
    pub min_asset_compose_cost: Uint,
    pub min_asset_decompose_cost: Uint,
    pub min_asset_unwrap_ccc_cost: Uint,

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
            "maxMetadataSize": "0x0400",
            "maxTextContentSize": "0x0200",
            "networkID" : "tc",
            "minPayParcelCost" : 10,
            "minSetRegularKeyParcelCost" : 11,
            "minCreateShardParcelCost" : 12,
            "minSetShardOwnersParcelCost" : 13,
            "minSetShardUsersParcelCost" : 14,
            "minWrapCccParcelCost" : 15,
            "minCustomParcelCost" : 16,
            "minStoreParcelCost" : 17,
            "minRemoveParcelCost" : 18,
            "minAssetMintCost" : 19,
            "minAssetTransferCost" : 20,
            "minAssetSchemeChangeCost" : 21,
            "minAssetComposeCost" : 22,
            "minAssetDecomposeCost" : 23,
            "minAssetUnwrapCccCost" : 24,
            "maxBodySize" : 4194304,
            "snapshotPeriod": 16384
        }"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.max_extra_data_size, Uint(U256::from(0x20)));
        assert_eq!(deserialized.max_metadata_size, Uint(U256::from(0x0400)));
        assert_eq!(deserialized.max_text_content_size, Uint(U256::from(0x0200)));
        assert_eq!(deserialized.network_id, "tc".into());
        assert_eq!(deserialized.min_pay_parcel_cost, Uint(10.into()));
        assert_eq!(deserialized.min_set_regular_key_parcel_cost, Uint(11.into()));
        assert_eq!(deserialized.min_create_shard_parcel_cost, Uint(12.into()));
        assert_eq!(deserialized.min_set_shard_owners_parcel_cost, Uint(13.into()));
        assert_eq!(deserialized.min_set_shard_users_parcel_cost, Uint(14.into()));
        assert_eq!(deserialized.min_wrap_ccc_parcel_cost, Uint(15.into()));
        assert_eq!(deserialized.min_custom_parcel_cost, Uint(16.into()));
        assert_eq!(deserialized.min_store_parcel_cost, Uint(17.into()));
        assert_eq!(deserialized.min_remove_parcel_cost, Uint(18.into()));
        assert_eq!(deserialized.min_asset_mint_cost, Uint(19.into()));
        assert_eq!(deserialized.min_asset_transfer_cost, Uint(20.into()));
        assert_eq!(deserialized.min_asset_scheme_change_cost, Uint(21.into()));
        assert_eq!(deserialized.min_asset_compose_cost, Uint(22.into()));
        assert_eq!(deserialized.min_asset_decompose_cost, Uint(23.into()));
        assert_eq!(deserialized.min_asset_unwrap_ccc_cost, Uint(24.into()));
        assert_eq!(deserialized.max_body_size, Uint(4_194_304.into()));
        assert_eq!(deserialized.snapshot_period, Uint(16_384.into()));
    }
}
