use cjson::scheme::Params;
use ckey::NetworkId;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Clone, Debug, PartialEq)]
pub struct CommonParams {
    size: usize,
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
            size: 23,
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

impl Encodable for CommonParams {
    fn rlp_append(&self, s: &mut RlpStream) {
        assert_eq!(23, self.size);
        s.begin_list(self.size)
            .append(&self.max_extra_data_size)
            .append(&self.max_asset_scheme_metadata_size)
            .append(&self.max_transfer_metadata_size)
            .append(&self.max_text_content_size)
            .append(&self.network_id)
            .append(&self.min_pay_transaction_cost)
            .append(&self.min_set_regular_key_transaction_cost)
            .append(&self.min_create_shard_transaction_cost)
            .append(&self.min_set_shard_owners_transaction_cost)
            .append(&self.min_set_shard_users_transaction_cost)
            .append(&self.min_wrap_ccc_transaction_cost)
            .append(&self.min_custom_transaction_cost)
            .append(&self.min_store_transaction_cost)
            .append(&self.min_remove_transaction_cost)
            .append(&self.min_asset_mint_cost)
            .append(&self.min_asset_transfer_cost)
            .append(&self.min_asset_scheme_change_cost)
            .append(&self.min_asset_supply_increase_cost)
            .append(&self.min_asset_compose_cost)
            .append(&self.min_asset_decompose_cost)
            .append(&self.min_asset_unwrap_ccc_cost)
            .append(&self.max_body_size)
            .append(&self.snapshot_period);
    }
}

impl Decodable for CommonParams {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let size = rlp.item_count()?;
        if size != 23 {
            return Err(DecoderError::RlpIncorrectListLen {
                expected: 23,
                got: size,
            })
        }

        let max_extra_data_size = rlp.val_at(0)?;
        let max_asset_scheme_metadata_size = rlp.val_at(1)?;
        let max_transfer_metadata_size = rlp.val_at(2)?;
        let max_text_content_size = rlp.val_at(3)?;
        let network_id = rlp.val_at(4)?;
        let min_pay_transaction_cost = rlp.val_at(5)?;
        let min_set_regular_key_transaction_cost = rlp.val_at(6)?;
        let min_create_shard_transaction_cost = rlp.val_at(7)?;
        let min_set_shard_owners_transaction_cost = rlp.val_at(8)?;
        let min_set_shard_users_transaction_cost = rlp.val_at(9)?;
        let min_wrap_ccc_transaction_cost = rlp.val_at(10)?;
        let min_custom_transaction_cost = rlp.val_at(11)?;
        let min_store_transaction_cost = rlp.val_at(12)?;
        let min_remove_transaction_cost = rlp.val_at(13)?;
        let min_asset_mint_cost = rlp.val_at(14)?;
        let min_asset_transfer_cost = rlp.val_at(15)?;
        let min_asset_scheme_change_cost = rlp.val_at(16)?;
        let min_asset_supply_increase_cost = rlp.val_at(17)?;
        let min_asset_compose_cost = rlp.val_at(18)?;
        let min_asset_decompose_cost = rlp.val_at(19)?;
        let min_asset_unwrap_ccc_cost = rlp.val_at(20)?;
        let max_body_size = rlp.val_at(21)?;
        let snapshot_period = rlp.val_at(22)?;
        Ok(Self {
            size,
            max_extra_data_size,
            max_asset_scheme_metadata_size,
            max_transfer_metadata_size,
            max_text_content_size,
            network_id,
            min_pay_transaction_cost,
            min_set_regular_key_transaction_cost,
            min_create_shard_transaction_cost,
            min_set_shard_owners_transaction_cost,
            min_set_shard_users_transaction_cost,
            min_wrap_ccc_transaction_cost,
            min_custom_transaction_cost,
            min_store_transaction_cost,
            min_remove_transaction_cost,
            min_asset_mint_cost,
            min_asset_transfer_cost,
            min_asset_scheme_change_cost,
            min_asset_supply_increase_cost,
            min_asset_compose_cost,
            min_asset_decompose_cost,
            min_asset_unwrap_ccc_cost,
            max_body_size,
            snapshot_period,
        })
    }
}

impl Default for CommonParams {
    fn default() -> Self {
        CommonParams {
            size: 23,
            max_extra_data_size: Default::default(),
            max_asset_scheme_metadata_size: Default::default(),
            max_transfer_metadata_size: Default::default(),
            max_text_content_size: Default::default(),
            network_id: Default::default(),
            min_pay_transaction_cost: Default::default(),
            min_set_regular_key_transaction_cost: Default::default(),
            min_create_shard_transaction_cost: Default::default(),
            min_set_shard_owners_transaction_cost: Default::default(),
            min_set_shard_users_transaction_cost: Default::default(),
            min_wrap_ccc_transaction_cost: Default::default(),
            min_custom_transaction_cost: Default::default(),
            min_store_transaction_cost: Default::default(),
            min_remove_transaction_cost: Default::default(),
            min_asset_mint_cost: Default::default(),
            min_asset_transfer_cost: Default::default(),
            min_asset_scheme_change_cost: Default::default(),
            min_asset_supply_increase_cost: Default::default(),
            min_asset_compose_cost: Default::default(),
            min_asset_decompose_cost: Default::default(),
            min_asset_unwrap_ccc_cost: Default::default(),
            max_body_size: Default::default(),
            snapshot_period: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlp::rlp_encode_and_decode_test;

    #[test]
    fn encode_and_decode_default() {
        rlp_encode_and_decode_test!(CommonParams::default());
    }
}
