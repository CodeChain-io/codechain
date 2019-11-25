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
#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone)]
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
    pub min_increase_asset_supply_cost: Uint,
    pub min_compose_asset_cost: Uint,
    pub min_decompose_asset_cost: Uint,
    pub min_unwrap_ccc_cost: Uint,

    /// Maximum size of block body.
    pub max_body_size: Uint,
    /// Snapshot creation period in unit of block numbers.
    pub snapshot_period: Uint,

    pub term_seconds: Option<Uint>,
    pub nomination_expiration: Option<Uint>,
    pub custody_period: Option<Uint>,
    pub release_period: Option<Uint>,
    pub max_num_of_validators: Option<Uint>,
    pub min_num_of_validators: Option<Uint>,
    pub delegation_threshold: Option<Uint>,
    pub min_deposit: Option<Uint>,
    pub max_candidate_metadata_size: Option<Uint>,

    /// A monotonically increasing number to denote the consensus version.
    /// It is increased when we fork.
    pub era: Option<Uint>,
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::Params;

    #[test]
    #[allow(clippy::cognitive_complexity)]
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
            "minIncreaseAssetSupplyCost": 25,
            "maxBodySize" : 4194304,
            "snapshotPeriod": 16384
        }"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.max_extra_data_size, 0x20.into());
        assert_eq!(deserialized.max_asset_scheme_metadata_size, 0x0400.into());
        assert_eq!(deserialized.max_transfer_metadata_size, 0x0100.into());
        assert_eq!(deserialized.max_text_content_size, 0x0200.into());
        assert_eq!(deserialized.network_id, "tc".into());
        assert_eq!(deserialized.min_pay_cost, 10.into());
        assert_eq!(deserialized.min_set_regular_key_cost, 11.into());
        assert_eq!(deserialized.min_create_shard_cost, 12.into());
        assert_eq!(deserialized.min_set_shard_owners_cost, 13.into());
        assert_eq!(deserialized.min_set_shard_users_cost, 14.into());
        assert_eq!(deserialized.min_wrap_ccc_cost, 15.into());
        assert_eq!(deserialized.min_custom_cost, 16.into());
        assert_eq!(deserialized.min_store_cost, 17.into());
        assert_eq!(deserialized.min_remove_cost, 18.into());
        assert_eq!(deserialized.min_mint_asset_cost, 19.into());
        assert_eq!(deserialized.min_transfer_asset_cost, 20.into());
        assert_eq!(deserialized.min_change_asset_scheme_cost, 21.into());
        assert_eq!(deserialized.min_compose_asset_cost, 22.into());
        assert_eq!(deserialized.min_decompose_asset_cost, 23.into());
        assert_eq!(deserialized.min_unwrap_ccc_cost, 24.into());
        assert_eq!(deserialized.min_increase_asset_supply_cost, 25.into());
        assert_eq!(deserialized.max_body_size, 4_194_304.into());
        assert_eq!(deserialized.snapshot_period, 16_384.into());
        assert_eq!(deserialized.term_seconds, None);
        assert_eq!(deserialized.nomination_expiration, None);
        assert_eq!(deserialized.custody_period, None);
        assert_eq!(deserialized.release_period, None);
        assert_eq!(deserialized.max_num_of_validators, None);
        assert_eq!(deserialized.min_num_of_validators, None);
        assert_eq!(deserialized.delegation_threshold, None);
        assert_eq!(deserialized.min_deposit, None);
        assert_eq!(deserialized.max_candidate_metadata_size, None);
    }


    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn params_deserialization_with_term_seconds() {
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
            "minIncreaseAssetSupplyCost": 25,
            "maxBodySize" : 4194304,
            "snapshotPeriod": 16384,
            "termSeconds": 3600
        }"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.max_extra_data_size, 0x20.into());
        assert_eq!(deserialized.max_asset_scheme_metadata_size, 0x0400.into());
        assert_eq!(deserialized.max_transfer_metadata_size, 0x0100.into());
        assert_eq!(deserialized.max_text_content_size, 0x0200.into());
        assert_eq!(deserialized.network_id, "tc".into());
        assert_eq!(deserialized.min_pay_cost, 10.into());
        assert_eq!(deserialized.min_set_regular_key_cost, 11.into());
        assert_eq!(deserialized.min_create_shard_cost, 12.into());
        assert_eq!(deserialized.min_set_shard_owners_cost, 13.into());
        assert_eq!(deserialized.min_set_shard_users_cost, 14.into());
        assert_eq!(deserialized.min_wrap_ccc_cost, 15.into());
        assert_eq!(deserialized.min_custom_cost, 16.into());
        assert_eq!(deserialized.min_store_cost, 17.into());
        assert_eq!(deserialized.min_remove_cost, 18.into());
        assert_eq!(deserialized.min_mint_asset_cost, 19.into());
        assert_eq!(deserialized.min_transfer_asset_cost, 20.into());
        assert_eq!(deserialized.min_change_asset_scheme_cost, 21.into());
        assert_eq!(deserialized.min_compose_asset_cost, 22.into());
        assert_eq!(deserialized.min_decompose_asset_cost, 23.into());
        assert_eq!(deserialized.min_unwrap_ccc_cost, 24.into());
        assert_eq!(deserialized.min_increase_asset_supply_cost, 25.into());
        assert_eq!(deserialized.max_body_size, 4_194_304.into());
        assert_eq!(deserialized.snapshot_period, 16_384.into());
        assert_eq!(deserialized.term_seconds, Some(3600.into()));
        assert_eq!(deserialized.nomination_expiration, None);
        assert_eq!(deserialized.custody_period, None);
        assert_eq!(deserialized.release_period, None);
        assert_eq!(deserialized.max_num_of_validators, None);
        assert_eq!(deserialized.min_num_of_validators, None);
        assert_eq!(deserialized.delegation_threshold, None);
        assert_eq!(deserialized.min_deposit, None);
        assert_eq!(deserialized.max_candidate_metadata_size, None);
    }


    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn params_deserialization_with_stake_params() {
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
            "minIncreaseAssetSupplyCost": 25,
            "maxBodySize" : 4194304,
            "snapshotPeriod": 16384,
            "termSeconds": 3600,
            "nominationExpiration": 26,
            "custodyPeriod": 27,
            "releasePeriod": 28,
            "maxNumOfValidators": 29,
            "minNumOfValidators": 30,
            "delegationThreshold": 31,
            "minDeposit": 32,
            "maxCandidateMetadataSize": 33
        }"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.max_extra_data_size, 0x20.into());
        assert_eq!(deserialized.max_asset_scheme_metadata_size, 0x0400.into());
        assert_eq!(deserialized.max_transfer_metadata_size, 0x0100.into());
        assert_eq!(deserialized.max_text_content_size, 0x0200.into());
        assert_eq!(deserialized.network_id, "tc".into());
        assert_eq!(deserialized.min_pay_cost, 10.into());
        assert_eq!(deserialized.min_set_regular_key_cost, 11.into());
        assert_eq!(deserialized.min_create_shard_cost, 12.into());
        assert_eq!(deserialized.min_set_shard_owners_cost, 13.into());
        assert_eq!(deserialized.min_set_shard_users_cost, 14.into());
        assert_eq!(deserialized.min_wrap_ccc_cost, 15.into());
        assert_eq!(deserialized.min_custom_cost, 16.into());
        assert_eq!(deserialized.min_store_cost, 17.into());
        assert_eq!(deserialized.min_remove_cost, 18.into());
        assert_eq!(deserialized.min_mint_asset_cost, 19.into());
        assert_eq!(deserialized.min_transfer_asset_cost, 20.into());
        assert_eq!(deserialized.min_change_asset_scheme_cost, 21.into());
        assert_eq!(deserialized.min_compose_asset_cost, 22.into());
        assert_eq!(deserialized.min_decompose_asset_cost, 23.into());
        assert_eq!(deserialized.min_unwrap_ccc_cost, 24.into());
        assert_eq!(deserialized.min_increase_asset_supply_cost, 25.into());
        assert_eq!(deserialized.max_body_size, 4_194_304.into());
        assert_eq!(deserialized.snapshot_period, 16_384.into());
        assert_eq!(deserialized.term_seconds, Some(3600.into()));
        assert_eq!(deserialized.nomination_expiration, Some(26.into()));
        assert_eq!(deserialized.custody_period, Some(27.into()));
        assert_eq!(deserialized.release_period, Some(28.into()));
        assert_eq!(deserialized.max_num_of_validators, Some(29.into()));
        assert_eq!(deserialized.min_num_of_validators, Some(30.into()));
        assert_eq!(deserialized.delegation_threshold, Some(31.into()));
        assert_eq!(deserialized.min_deposit, Some(32.into()));
        assert_eq!(deserialized.max_candidate_metadata_size, Some(33.into()));
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn params_deserialization_with_era() {
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
            "minIncreaseAssetSupplyCost": 25,
            "maxBodySize" : 4194304,
            "snapshotPeriod": 16384,
            "termSeconds": 3600,
            "nominationExpiration": 26,
            "custodyPeriod": 27,
            "releasePeriod": 28,
            "maxNumOfValidators": 29,
            "minNumOfValidators": 30,
            "delegationThreshold": 31,
            "minDeposit": 32,
            "maxCandidateMetadataSize": 33,
            "era": 34
        }"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.max_extra_data_size, 0x20.into());
        assert_eq!(deserialized.max_asset_scheme_metadata_size, 0x0400.into());
        assert_eq!(deserialized.max_transfer_metadata_size, 0x0100.into());
        assert_eq!(deserialized.max_text_content_size, 0x0200.into());
        assert_eq!(deserialized.network_id, "tc".into());
        assert_eq!(deserialized.min_pay_cost, 10.into());
        assert_eq!(deserialized.min_set_regular_key_cost, 11.into());
        assert_eq!(deserialized.min_create_shard_cost, 12.into());
        assert_eq!(deserialized.min_set_shard_owners_cost, 13.into());
        assert_eq!(deserialized.min_set_shard_users_cost, 14.into());
        assert_eq!(deserialized.min_wrap_ccc_cost, 15.into());
        assert_eq!(deserialized.min_custom_cost, 16.into());
        assert_eq!(deserialized.min_store_cost, 17.into());
        assert_eq!(deserialized.min_remove_cost, 18.into());
        assert_eq!(deserialized.min_mint_asset_cost, 19.into());
        assert_eq!(deserialized.min_transfer_asset_cost, 20.into());
        assert_eq!(deserialized.min_change_asset_scheme_cost, 21.into());
        assert_eq!(deserialized.min_compose_asset_cost, 22.into());
        assert_eq!(deserialized.min_decompose_asset_cost, 23.into());
        assert_eq!(deserialized.min_unwrap_ccc_cost, 24.into());
        assert_eq!(deserialized.min_increase_asset_supply_cost, 25.into());
        assert_eq!(deserialized.max_body_size, 4_194_304.into());
        assert_eq!(deserialized.snapshot_period, 16_384.into());
        assert_eq!(deserialized.term_seconds, Some(3600.into()));
        assert_eq!(deserialized.nomination_expiration, Some(26.into()));
        assert_eq!(deserialized.custody_period, Some(27.into()));
        assert_eq!(deserialized.release_period, Some(28.into()));
        assert_eq!(deserialized.max_num_of_validators, Some(29.into()));
        assert_eq!(deserialized.min_num_of_validators, Some(30.into()));
        assert_eq!(deserialized.delegation_threshold, Some(31.into()));
        assert_eq!(deserialized.min_deposit, Some(32.into()));
        assert_eq!(deserialized.max_candidate_metadata_size, Some(33.into()));
        assert_eq!(deserialized.era, Some(34.into()));
    }
}
