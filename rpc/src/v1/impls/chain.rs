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

use std::sync::Arc;

use ccore::{
    AssetClient, BlockId, EngineInfo, ExecuteClient, MinerService, MiningBlockChainClient, RegularKey, RegularKeyOwner,
    Shard, SignedParcel, TextClient, UnverifiedParcel,
};
use cjson::bytes::Bytes;
use cjson::uint::Uint;
use ckey::{public_to_address, NetworkId, PlatformAddress, Public};
use cstate::{AssetScheme, AssetSchemeAddress, FindActionHandler, OwnedAsset};
use ctypes::invoice::Invoice;
use ctypes::parcel::Action;
use ctypes::transaction::ShardTransaction as TransactionType;
use ctypes::{BlockNumber, ShardId};
use primitives::{Bytes as BytesArray, H256};
use rlp::{DecoderError, UntrustedRlp};

use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Chain;
use super::super::types::{Block, BlockNumberAndHash, Text, Transaction, UnsignedTransaction};

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
            .and_then(|parcel: UnverifiedParcel| {
                if let Action::Custom {
                    handler_id,
                    ..
                } = &parcel.action
                {
                    if self.client.find_action_handler_for(*handler_id).is_none() {
                        return Err(errors::rlp(&DecoderError::Custom("Invalid custom action!")))
                    }
                }
                Ok(parcel)
            })
            .and_then(|parcel| SignedParcel::try_new(parcel).map_err(errors::parcel_core))
            .and_then(|signed| {
                let hash = signed.hash();
                self.miner.import_own_parcel(&*self.client, signed).map_err(errors::parcel_core).map(|_| hash)
            })
            .map(Into::into)
    }

    fn get_transaction(&self, transaction_hash: H256) -> Result<Option<Transaction>> {
        match self.client.parcel(&transaction_hash.into()) {
            Some(parcel) => Ok(Some(parcel.into())),
            None => Ok(None),
        }
    }

    fn get_invoice(&self, transaction_hash: H256) -> Result<Option<Invoice>> {
        Ok(self.client.parcel_invoice(&transaction_hash.into()))
    }

    fn get_transaction_with_payload_hash(&self, payload_hash: H256) -> Result<Option<Transaction>> {
        Ok(self.client.transaction(&payload_hash).map(Into::into))
    }

    fn get_invoices_with_payload_hash(&self, payload_hash: H256) -> Result<Vec<Invoice>> {
        Ok(self.client.transaction_invoices(&payload_hash))
    }

    fn get_asset_scheme_by_hash(
        &self,
        transaction_hash: H256,
        shard_id: ShardId,
        block_number: Option<u64>,
    ) -> Result<Option<AssetScheme>> {
        let address = AssetSchemeAddress::new(transaction_hash, shard_id);
        self.get_asset_scheme_by_type(address.into(), block_number)
    }

    fn get_asset_scheme_by_type(&self, asset_type: H256, block_number: Option<u64>) -> Result<Option<AssetScheme>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        match AssetSchemeAddress::from_hash(asset_type) {
            Some(address) => self.client.get_asset_scheme(address, block_id).map_err(errors::parcel_state),
            None => Ok(None),
        }
    }

    fn get_text(&self, transaction_hash: H256, block_number: Option<u64>) -> Result<Option<Text>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self
            .client
            .get_text(transaction_hash, block_id)
            .map_err(errors::parcel_state)?
            .map(|text| Text::from_core(text, self.client.common_params().network_id)))
    }

    fn get_asset(&self, transaction_hash: H256, index: usize, block_number: Option<u64>) -> Result<Option<OwnedAsset>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        self.client.get_asset(transaction_hash, index, block_id).map_err(errors::parcel_state)
    }

    fn is_asset_spent(
        &self,
        transaction_hash: H256,
        index: usize,
        shard_id: ShardId,
        block_number: Option<u64>,
    ) -> Result<Option<bool>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        self.client.is_asset_spent(transaction_hash, index, shard_id, block_id).map_err(errors::parcel_state)
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

    fn get_shard_root(&self, shard_id: ShardId, block_number: Option<u64>) -> Result<Option<H256>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.shard_root(shard_id, block_id.into()))
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
        Ok(self
            .client
            .block(&BlockId::Number(block_number))
            .map(|block| Block::from_core(block.decode(), self.client.common_params().network_id)))
    }

    fn get_block_by_hash(&self, block_hash: H256) -> Result<Option<Block>> {
        Ok(self
            .client
            .block(&BlockId::Hash(block_hash))
            .map(|block| Block::from_core(block.decode(), self.client.common_params().network_id)))
    }

    fn get_pending_transactions(&self) -> Result<Vec<Transaction>> {
        Ok(self.client.ready_parcels().into_iter().map(|signed| signed.into()).collect())
    }

    fn get_mining_reward(&self, block_number: u64) -> Result<Option<u64>> {
        Ok(self.client.mining_reward(block_number))
    }

    fn get_network_id(&self) -> Result<NetworkId> {
        Ok(self.client.common_params().network_id)
    }

    fn execute_transaction(&self, tx: UnsignedTransaction, sender: PlatformAddress) -> Result<Invoice> {
        let sender_address = sender.try_address().map_err(errors::core)?;
        let action = ::std::result::Result::from(tx.action).map_err(errors::core)?;
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
        let action = ::std::result::Result::from(tx.action).map_err(errors::core)?;
        if let Action::TransferAsset {
            inputs,
            ..
        } = &action
        {
            let transaction = Option::<TransactionType>::from(action.clone()).unwrap();
            Ok(self.client.execute_vm(&transaction, inputs, &params, &indices).map_err(errors::core)?)
        } else {
            Err(errors::transfer_only())
        }
    }
}
