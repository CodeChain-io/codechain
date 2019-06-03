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

use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use ccore::{
    AccountData, AssetClient, BlockId, EngineInfo, ExecuteClient, MetadataInfo, MiningBlockChainClient, Shard,
    TextClient,
};
use ccrypto::Blake;
use cjson::scheme::Params;
use cjson::uint::Uint;
use ckey::{public_to_address, NetworkId, PlatformAddress, Public};
use cstate::FindActionHandler;
use ctypes::transaction::{Action, ShardTransaction as ShardTransactionType};
use ctypes::{BlockNumber, ShardId};
use primitives::{Bytes as BytesArray, H160, H256};

use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Chain;
use super::super::types::{AssetScheme, Block, BlockNumberAndHash, OwnedAsset, Text, Transaction, UnsignedTransaction};

pub struct ChainClient<C>
where
    C: AssetClient + MiningBlockChainClient + Shard + ExecuteClient + EngineInfo, {
    client: Arc<C>,
}

impl<C> ChainClient<C>
where
    C: AssetClient + MiningBlockChainClient + Shard + AccountData + ExecuteClient + EngineInfo + TextClient,
{
    pub fn new(client: Arc<C>) -> Self {
        ChainClient {
            client,
        }
    }
}

impl<C> Chain for ChainClient<C>
where
    C: AssetClient
        + MiningBlockChainClient
        + Shard
        + AccountData
        + ExecuteClient
        + EngineInfo
        + FindActionHandler
        + TextClient
        + MetadataInfo
        + 'static,
{
    fn get_transaction(&self, transaction_hash: H256) -> Result<Option<Transaction>> {
        let id = transaction_hash.into();
        Ok(self.client.transaction(&id).map(From::from))
    }

    fn contains_transaction(&self, transaction_hash: H256) -> Result<bool> {
        Ok(self.client.transaction_block(&transaction_hash.into()).is_some())
    }

    fn contain_transaction(&self, transaction_hash: H256) -> Result<bool> {
        self.contains_transaction(transaction_hash)
    }

    fn get_transaction_by_tracker(&self, tracker: H256) -> Result<Option<Transaction>> {
        Ok(self.client.transaction_by_tracker(&tracker).map(From::from))
    }

    fn get_asset_scheme_by_tracker(
        &self,
        tracker: H256,
        shard_id: ShardId,
        block_number: Option<u64>,
    ) -> Result<Option<AssetScheme>> {
        let asset_type = Blake::blake(tracker);
        self.get_asset_scheme_by_type(asset_type, shard_id, block_number)
    }

    fn get_asset_scheme_by_type(
        &self,
        asset_type: H160,
        shard_id: ShardId,
        block_number: Option<u64>,
    ) -> Result<Option<AssetScheme>> {
        if block_number == Some(0) {
            return Ok(None)
        }
        let parent_block_id = block_number.map(|n| (n - 1).into()).unwrap_or(BlockId::ParentOfLatest);
        if let Some(common_params) = self.client.common_params(parent_block_id) {
            let network_id = common_params.network_id();
            let block_id = block_number.map(BlockId::from).unwrap_or(BlockId::Latest);
            Ok(self
                .client
                .get_asset_scheme(asset_type, shard_id, block_id)
                .map_err(errors::transaction_state)?
                .map(|asset_scheme| AssetScheme::from_core(asset_scheme, network_id)))
        } else {
            Ok(None)
        }
    }

    fn get_text(&self, transaction_hash: H256, block_number: Option<u64>) -> Result<Option<Text>> {
        if block_number == Some(0) {
            return Ok(None)
        }
        let block_id = block_number.map(BlockId::from).unwrap_or(BlockId::Latest);
        Ok(self.client.get_text(transaction_hash, block_id).map_err(errors::transaction_state)?.map(|text| {
            let parent_block_id = block_number.map(|n| (n - 1).into()).unwrap_or(BlockId::ParentOfLatest);
            Text::from_core(text, self.client.common_params(parent_block_id).unwrap().network_id())
        }))
    }

    fn get_asset(
        &self,
        tracker: H256,
        index: usize,
        shard_id: ShardId,
        block_number: Option<u64>,
    ) -> Result<Option<OwnedAsset>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let asset = self.client.get_asset(tracker, index, shard_id, block_id).map_err(errors::transaction_state)?;
        Ok(asset.map(From::from))
    }

    fn is_asset_spent(
        &self,
        transaction_hash: H256,
        index: usize,
        shard_id: ShardId,
        block_number: Option<u64>,
    ) -> Result<Option<bool>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        self.client.is_asset_spent(transaction_hash, index, shard_id, block_id).map_err(errors::transaction_state)
    }

    fn get_seq(&self, address: PlatformAddress, block_number: Option<u64>) -> Result<Option<u64>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let address = address.try_address().map_err(errors::core)?;
        Ok(self.client.seq(address, block_id))
    }

    fn get_balance(&self, aaddress: PlatformAddress, block_number: Option<u64>) -> Result<Option<Uint>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let address = aaddress.try_address().map_err(errors::core)?;
        Ok(self.client.balance(address, block_id.into()).map(Into::into))
    }

    fn get_regular_key(&self, address: PlatformAddress, block_number: Option<u64>) -> Result<Option<Public>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let address = address.try_address().map_err(errors::core)?;
        Ok(self.client.regular_key(address, block_id.into()))
    }

    fn get_regular_key_owner(&self, public: Public, block_number: Option<u64>) -> Result<Option<PlatformAddress>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.regular_key_owner(&public_to_address(&public), block_id.into()).and_then(|address| {
            let parent_block_id = block_number.map(|n| (n - 1).into()).unwrap_or(BlockId::ParentOfLatest);
            let network_id = self.client.common_params(parent_block_id).unwrap().network_id();
            Some(PlatformAddress::new_v1(network_id, address))
        }))
    }

    fn get_genesis_accounts(&self) -> Result<Vec<PlatformAddress>> {
        Ok(self.client.genesis_accounts())
    }

    fn get_number_of_shards(&self, block_number: Option<u64>) -> Result<Option<ShardId>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.number_of_shards(block_id.into()))
    }

    fn get_shard_id_by_hash(&self, create_shard_tx_hash: H256, block_number: Option<u64>) -> Result<Option<ShardId>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.shard_id_by_hash(&create_shard_tx_hash, block_id.into()))
    }

    fn get_shard_root(&self, shard_id: ShardId, block_number: Option<u64>) -> Result<Option<H256>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.shard_root(shard_id, block_id.into()))
    }

    fn get_shard_owners(&self, shard_id: ShardId, block_number: Option<u64>) -> Result<Option<Vec<PlatformAddress>>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.shard_owners(shard_id, block_id.into()).map(|owners| {
            let parent_block_id = block_number.map(|n| (n - 1).into()).unwrap_or(BlockId::ParentOfLatest);
            let network_id = self.client.common_params(parent_block_id).unwrap().network_id();
            owners.into_iter().map(|owner| PlatformAddress::new_v1(network_id, owner)).collect()
        }))
    }

    fn get_shard_users(&self, shard_id: ShardId, block_number: Option<u64>) -> Result<Option<Vec<PlatformAddress>>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.shard_users(shard_id, block_id.into()).map(|users| {
            let parent_block_id = block_number.map(|n| (n - 1).into()).unwrap_or(BlockId::ParentOfLatest);
            let network_id = self.client.common_params(parent_block_id).unwrap().network_id();
            users.into_iter().map(|user| PlatformAddress::new_v1(network_id, user)).collect()
        }))
    }

    fn get_best_block_number(&self) -> Result<BlockNumber> {
        Ok(self.client.chain_info().best_block_number)
    }

    fn get_best_block_id(&self) -> Result<BlockNumberAndHash> {
        let chain_info = self.client.chain_info();
        Ok(BlockNumberAndHash {
            number: chain_info.best_block_number,
            hash: chain_info.best_block_hash,
        })
    }

    fn get_block_hash(&self, block_number: u64) -> Result<Option<H256>> {
        Ok(self.client.block_hash(&BlockId::Number(block_number)))
    }

    fn get_block_by_number(&self, block_number: u64) -> Result<Option<Block>> {
        let id = BlockId::Number(block_number);
        Ok(self.client.block(&id).map(|block| {
            let block_id_to_read_params = if block_number == 0 {
                0.into()
            } else {
                (block_number - 1).into()
            };
            Block::from_core(block.decode(), self.client.common_params(block_id_to_read_params).unwrap().network_id())
        }))
    }

    fn get_block_by_hash(&self, block_hash: H256) -> Result<Option<Block>> {
        let id = BlockId::Hash(block_hash);
        Ok(self.client.block(&id).map(|block| {
            let block = block.decode();
            let block_id_to_read_params = if block.header.number() == 0 {
                0.into()
            } else {
                (*block.header.parent_hash()).into()
            };
            Block::from_core(block, self.client.common_params(block_id_to_read_params).unwrap().network_id())
        }))
    }

    fn get_block_transaction_count_by_hash(&self, block_hash: H256) -> Result<Option<usize>> {
        Ok(self.client.block(&BlockId::Hash(block_hash)).map(|block| block.transactions_count()))
    }

    fn get_min_transaction_fee(&self, action_type: String, block_number: Option<u64>) -> Result<Option<u64>> {
        if block_number == Some(0) {
            return Ok(None)
        }
        // Unlike other RPCs, use the latest parameters if the block number is `null`.
        let block_id = block_number.map(|n| (n - 1).into()).unwrap_or(BlockId::Latest);
        if let Some(common_parameters) = self.client.common_params(block_id) {
            Ok(match action_type.as_str() {
                "mintAsset" => Some(common_parameters.min_asset_mint_cost()),
                "transferAsset" => Some(common_parameters.min_asset_transfer_cost()),
                "changeAssetScheme" => Some(common_parameters.min_asset_scheme_change_cost()),
                "increaseAssetSupply" => Some(common_parameters.min_asset_supply_increase_cost()),
                "unwrapCCC" => Some(common_parameters.min_asset_unwrap_ccc_cost()),
                "pay" => Some(common_parameters.min_pay_transaction_cost()),
                "setRegularKey" => Some(common_parameters.min_set_regular_key_transaction_cost()),
                "createShard" => Some(common_parameters.min_create_shard_transaction_cost()),
                "setShardOwners" => Some(common_parameters.min_set_shard_owners_transaction_cost()),
                "setShardUsers" => Some(common_parameters.min_set_shard_users_transaction_cost()),
                "wrapCCC" => Some(common_parameters.min_wrap_ccc_transaction_cost()),
                "store" => Some(common_parameters.min_store_transaction_cost()),
                "remove" => Some(common_parameters.min_remove_transaction_cost()),
                "custom" => Some(common_parameters.min_custom_transaction_cost()),
                "composeAsset" => Some(common_parameters.min_asset_compose_cost()),
                "decomposeAsset" => Some(common_parameters.min_asset_decompose_cost()),

                _ => None,
            })
        } else {
            Ok(None)
        }
    }

    fn get_mining_reward(&self, block_number: u64) -> Result<Option<u64>> {
        Ok(self.client.mining_reward(block_number))
    }

    fn get_network_id(&self) -> Result<NetworkId> {
        Ok(self.client.common_params(BlockId::Latest).unwrap().network_id())
    }

    fn get_common_params(&self, block_number: Option<u64>) -> Result<Option<Params>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.common_params(block_id).map(Params::from))
    }

    fn get_term_metadata(&self, block_number: Option<u64>) -> Result<Option<(u64, u64)>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.metadata(block_id).map(|m| (m.last_term_finished_block_num(), m.current_term_id())))
    }

    fn execute_transaction(&self, tx: UnsignedTransaction, sender: PlatformAddress) -> Result<Option<String>> {
        let sender_address = sender.try_address().map_err(errors::core)?;
        let action = Action::try_from(tx.action).map_err(errors::conversion)?;
        if let Some(transaction) = action.asset_transaction() {
            let result = self.client.execute_transaction(&transaction, sender_address);
            match result {
                Ok(()) => Ok(None),
                Err(err) => Ok(Some(err.to_string())),
            }
        } else {
            Err(errors::asset_transaction_only())
        }
    }

    fn execute_vm(
        &self,
        tx: UnsignedTransaction,
        params: Vec<Vec<BytesArray>>,
        indices: Vec<usize>,
    ) -> Result<Vec<String>> {
        let action = tx.action.try_into().map_err(errors::conversion)?;
        if let Action::TransferAsset {
            inputs,
            ..
        } = &action
        {
            let transaction = Option::<ShardTransactionType>::from(action.clone()).unwrap();
            Ok(self.client.execute_vm(&transaction, inputs, &params, &indices).map_err(errors::core)?)
        } else {
            Err(errors::transfer_only())
        }
    }
}
