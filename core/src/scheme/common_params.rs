use cjson::scheme::Params;
use ckey::NetworkId;

#[derive(Clone, Debug, PartialEq, Default, RlpEncodable)]
pub struct CommonParams {
    /// Maximum size of extra data.
    pub max_extra_data_size: usize,
    /// Maximum size of metadata of AssetScheme.
    pub max_asset_scheme_metadata_size: usize,
    /// Maximum size of metadata of TransferAsset.
    pub max_transfer_metadata_size: usize,
    /// Maximum size of the content of text used in store/remove actions.
    pub max_text_content_size: usize,
    /// Network id.
    pub network_id: NetworkId,
    /// Minimum transaction cost.
    pub min_pay_transaction_cost: u64,
    pub min_set_regular_key_transaction_cost: u64,
    pub min_create_shard_transaction_cost: u64,
    pub min_set_shard_owners_transaction_cost: u64,
    pub min_set_shard_users_transaction_cost: u64,
    pub min_wrap_ccc_transaction_cost: u64,
    pub min_custom_transaction_cost: u64,
    pub min_store_transaction_cost: u64,
    pub min_remove_transaction_cost: u64,
    pub min_asset_mint_cost: u64,
    pub min_asset_transfer_cost: u64,
    pub min_asset_scheme_change_cost: u64,
    pub min_asset_supply_increase_cost: u64,
    pub min_asset_compose_cost: u64,
    pub min_asset_decompose_cost: u64,
    pub min_asset_unwrap_ccc_cost: u64,
    /// Maximum size of block body.
    pub max_body_size: usize,
    /// Snapshot creation period in unit of block numbers.
    pub snapshot_period: u64,
}

impl From<Params> for CommonParams {
    fn from(p: Params) -> Self {
        Self {
            max_extra_data_size: p.max_extra_data_size.into(),
            max_asset_scheme_metadata_size: p.max_asset_scheme_metadata_size.into(),
            max_transfer_metadata_size: p.max_transfer_metadata_size.into(),
            max_text_content_size: p.max_text_content_size.into(),
            network_id: p.network_id,
            min_pay_transaction_cost: p.min_pay_cost.into(),
            min_set_regular_key_transaction_cost: p.min_set_regular_key_cost.into(),
            min_create_shard_transaction_cost: p.min_create_shard_cost.into(),
            min_set_shard_owners_transaction_cost: p.min_set_shard_owners_cost.into(),
            min_set_shard_users_transaction_cost: p.min_set_shard_users_cost.into(),
            min_wrap_ccc_transaction_cost: p.min_wrap_ccc_cost.into(),
            min_custom_transaction_cost: p.min_custom_cost.into(),
            min_store_transaction_cost: p.min_store_cost.into(),
            min_remove_transaction_cost: p.min_remove_cost.into(),
            min_asset_mint_cost: p.min_mint_asset_cost.into(),
            min_asset_transfer_cost: p.min_transfer_asset_cost.into(),
            min_asset_scheme_change_cost: p.min_change_asset_scheme_cost.into(),
            min_asset_supply_increase_cost: p.min_increase_asset_supply_cost.into(),
            min_asset_compose_cost: p.min_compose_asset_cost.into(),
            min_asset_decompose_cost: p.min_decompose_asset_cost.into(),
            min_asset_unwrap_ccc_cost: p.min_unwrap_ccc_cost.into(),
            max_body_size: p.max_body_size.into(),
            snapshot_period: p.snapshot_period.into(),
        }
    }
}
