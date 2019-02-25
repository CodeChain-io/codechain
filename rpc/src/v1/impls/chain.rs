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

use std::sync::Arc;

use ccore::{
    AssetClient, BlockId, EngineInfo, ExecuteClient, MinerService, MiningBlockChainClient, RegularKey, RegularKeyOwner,
    Shard, SignedTransaction, TextClient, UnverifiedTransaction,
};
use ccrypto::Blake;
use cjson::bytes::Bytes;
use cjson::uint::Uint;
use ckey::{public_to_address, NetworkId, PlatformAddress, Public};
use cstate::FindActionHandler;
use ctypes::invoice::Invoice;
use ctypes::transaction::{Action, ShardTransaction as ShardTransactionType};
use ctypes::{BlockNumber, ShardId};
use primitives::{Bytes as BytesArray, H160, H256};
use rlp::{DecoderError, UntrustedRlp};

use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Chain;
use super::super::types::{AssetScheme, Block, BlockNumberAndHash, OwnedAsset, Text, Transaction, UnsignedTransaction};

pub struct ChainClient<C, M>
where
    C: AssetClient + MiningBlockChainClient + Shard + RegularKey + RegularKeyOwner + ExecuteClient + EngineInfo,
    M: MinerService, {
    client: Arc<C>,
    miner: Arc<M>,
}

impl<C, M> ChainClient<C, M>
where
    C: AssetClient
        + MiningBlockChainClient
        + Shard
        + RegularKey
        + RegularKeyOwner
        + ExecuteClient
        + EngineInfo
        + TextClient,
    M: MinerService,
{
    pub fn new(client: Arc<C>, miner: Arc<M>) -> Self {
        ChainClient {
            client,
            miner,
        }
    }
}

impl<C, M> Chain for ChainClient<C, M>
where
    C: AssetClient
        + MiningBlockChainClient
        + Shard
        + RegularKey
        + RegularKeyOwner
        + ExecuteClient
        + EngineInfo
        + FindActionHandler
        + TextClient
        + 'static,
    M: MinerService + 'static,
{
    fn send_signed_transaction(&self, raw: Bytes) -> Result<H256> {
        UntrustedRlp::new(&raw.into_vec())
            .as_val()
            .map_err(|e| errors::rlp(&e))
            .and_then(|tx: UnverifiedTransaction| {
                if let Action::Custom {
                    handler_id,
                    ..
                } = &tx.action
                {
                    if self.client.find_action_handler_for(*handler_id).is_none() {
                        return Err(errors::rlp(&DecoderError::Custom("Invalid custom action!")))
                    }
                }
                Ok(tx)
            })
            .and_then(|tx| SignedTransaction::try_new(tx).map_err(errors::transaction_core))
            .and_then(|signed| {
                let hash = signed.hash();
                self.miner.import_own_transaction(&*self.client, signed).map_err(errors::transaction_core).map(|_| hash)
            })
            .map(Into::into)
    }

    fn get_transaction(&self, transaction_hash: H256) -> Result<Option<Transaction>> {
        let id = transaction_hash.into();
        Ok(self.client.transaction(&id).map(|tx| {
            let invoice = self.client.invoice(&id).expect("Invoice must exist when transaction exists");
            Transaction::from(tx, invoice.to_bool())
        }))
    }

    fn get_transaction_result(&self, transaction_hash: H256) -> Result<Option<bool>> {
        Ok(self.client.invoice(&transaction_hash.into()).map(|invoice| invoice.to_bool()))
    }

    fn get_transaction_by_tracker(&self, tracker: H256) -> Result<Option<Transaction>> {
        Ok(self.client.transaction_by_tracker(&tracker).map(|tx| {
            let transaction_id = tx.hash().into();
            let invoice = self.client.invoice(&transaction_id).expect("Invoice must exist when transaction exists");
            Transaction::from(tx, invoice.to_bool())
        }))
    }

    fn get_transaction_results_by_tracker(&self, tracker: H256) -> Result<Vec<bool>> {
        Ok(self.client.invoices_by_tracker(&tracker).into_iter().map(|invoice| invoice.to_bool()).collect())
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
        let network_id = self.client.common_params().network_id;
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self
            .client
            .get_asset_scheme(asset_type, shard_id, block_id)
            .map_err(errors::transaction_state)?
            .map(|asset_scheme| AssetScheme::from_core(asset_scheme, network_id)))
    }

    fn get_text(&self, transaction_hash: H256, block_number: Option<u64>) -> Result<Option<Text>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self
            .client
            .get_text(transaction_hash, block_id)
            .map_err(errors::transaction_state)?
            .map(|text| Text::from_core(text, self.client.common_params().network_id)))
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

    fn get_error_hint(&self, transaction_hash: H256) -> Result<Option<String>> {
        if let Some(Invoice::Failure(error_string)) = self.client.invoice(&transaction_hash.into()) {
            Ok(Some(error_string))
        } else {
            Ok(None)
        }
    }

    fn get_regular_key(&self, address: PlatformAddress, block_number: Option<u64>) -> Result<Option<Public>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let address = address.try_address().map_err(errors::core)?;
        Ok(self.client.regular_key(address, block_id.into()))
    }

    fn get_regular_key_owner(&self, public: Public, block_number: Option<u64>) -> Result<Option<PlatformAddress>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let network_id = self.client.common_params().network_id;
        Ok(self
            .client
            .regular_key_owner(&public_to_address(&public), block_id.into())
            .and_then(|address| Some(PlatformAddress::new_v1(network_id, address))))
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
        let network_id = self.client.common_params().network_id;
        Ok(self
            .client
            .shard_owners(shard_id, block_id.into())
            .map(|owners| owners.into_iter().map(|owner| PlatformAddress::new_v1(network_id, owner)).collect()))
    }

    fn get_shard_users(&self, shard_id: ShardId, block_number: Option<u64>) -> Result<Option<Vec<PlatformAddress>>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        let network_id = self.client.common_params().network_id;
        Ok(self
            .client
            .shard_users(shard_id, block_id.into())
            .map(|users| users.into_iter().map(|user| PlatformAddress::new_v1(network_id, user)).collect()))
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
            let invoices: Vec<_> = self
                .client
                .block_invoices(&id)
                .unwrap_or_else(|| {
                    assert_eq!(0, block_number);
                    Default::default()
                })
                .invoices
                .into_iter()
                .map(|invoice| invoice.to_bool())
                .collect();
            Block::from_core(block.decode(), self.client.common_params().network_id, &invoices)
        }))
    }

    fn get_block_by_hash(&self, block_hash: H256) -> Result<Option<Block>> {
        let id = BlockId::Hash(block_hash);
        Ok(self.client.block(&id).map(|block| {
            let invoices: Vec<_> = self
                .client
                .block_invoices(&id)
                .unwrap_or_else(|| {
                    assert_eq!(0, block.number());
                    Default::default()
                })
                .invoices
                .into_iter()
                .map(|invoice| invoice.to_bool())
                .collect();
            Block::from_core(block.decode(), self.client.common_params().network_id, &invoices)
        }))
    }

    fn get_block_transaction_count_by_hash(&self, block_hash: H256) -> Result<Option<usize>> {
        Ok(self.client.block(&BlockId::Hash(block_hash)).map(|block| block.transactions_count()))
    }

    fn get_pending_transactions(&self) -> Result<Vec<Transaction>> {
        Ok(self.client.ready_transactions().into_iter().map(|signed| signed.into()).collect())
    }

    fn get_pending_transactions_count(&self) -> Result<usize> {
        Ok(self.client.count_pending_transactions())
    }

    fn get_mining_reward(&self, block_number: u64) -> Result<Option<u64>> {
        Ok(self.client.mining_reward(block_number))
    }

    fn get_network_id(&self) -> Result<NetworkId> {
        Ok(self.client.common_params().network_id)
    }

    fn execute_transaction(&self, tx: UnsignedTransaction, sender: PlatformAddress) -> Result<Invoice> {
        let sender_address = sender.try_address().map_err(errors::core)?;
        let action = ::std::result::Result::from(tx.action).map_err(errors::conversion)?;
        if let Some(transaction) = action.asset_transaction() {
            Ok(self.client.execute_transaction(&transaction, sender_address).map_err(errors::core)?)
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
        let action = ::std::result::Result::from(tx.action).map_err(errors::conversion)?;
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
