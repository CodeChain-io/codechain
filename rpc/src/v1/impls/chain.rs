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
    AssetClient, BlockId, ExecuteClient, MinerService, MiningBlockChainClient, RegularKey, Shard, SignedParcel,
};
use ckey::{Address, Public};
use cstate::{Asset, AssetScheme, AssetSchemeAddress};
use ctypes::invoice::{Invoice, ParcelInvoice};
use ctypes::parcel::ChangeShard;
use ctypes::transaction::Transaction;
use ctypes::{BlockNumber, ShardId};
use primitives::{H160, H256, U256};
use rlp::UntrustedRlp;

use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Chain;
use super::super::types::{Block, BlockNumberAndHash, Bytes, Parcel};

pub struct ChainClient<C, M>
where
    C: AssetClient + MiningBlockChainClient + Shard + RegularKey + ExecuteClient,
    M: MinerService, {
    client: Arc<C>,
    miner: Arc<M>,
}

impl<C, M> ChainClient<C, M>
where
    C: AssetClient + MiningBlockChainClient + Shard + RegularKey + ExecuteClient,
    M: MinerService,
{
    pub fn new(client: &Arc<C>, miner: &Arc<M>) -> Self {
        ChainClient {
            client: client.clone(),
            miner: miner.clone(),
        }
    }
}

impl<C, M> Chain for ChainClient<C, M>
where
    C: AssetClient + MiningBlockChainClient + Shard + RegularKey + ExecuteClient + 'static,
    M: MinerService + 'static,
{
    fn send_signed_parcel(&self, raw: Bytes) -> Result<H256> {
        UntrustedRlp::new(&raw.into_vec())
            .as_val()
            .map_err(errors::rlp)
            .and_then(|parcel| SignedParcel::new(parcel).map_err(errors::parcel_core))
            .and_then(|signed| {
                let hash = signed.hash();
                self.miner.import_own_parcel(&*self.client, signed).map_err(errors::parcel_core).map(|_| hash)
            })
            .map(Into::into)
    }

    fn get_parcel(&self, parcel_hash: H256) -> Result<Option<Parcel>> {
        match self.client.parcel(parcel_hash.into()) {
            Some(parcel) => Ok(Some(parcel.into())),
            None => Ok(None),
        }
    }

    fn get_parcel_invoice(&self, parcel_hash: H256) -> Result<Option<ParcelInvoice>> {
        Ok(self.client.parcel_invoice(parcel_hash.into()))
    }

    fn get_transaction_invoice(&self, transaction_hash: H256) -> Result<Option<Invoice>> {
        Ok(self.client.transaction_invoice(transaction_hash.into()))
    }

    fn get_asset_scheme_by_hash(&self, transaction_hash: H256, shard_id: ShardId) -> Result<Option<AssetScheme>> {
        let address = AssetSchemeAddress::new(transaction_hash, shard_id);
        self.get_asset_scheme_by_type(address.into())
    }

    fn get_asset_scheme_by_type(&self, asset_type: H256) -> Result<Option<AssetScheme>> {
        match AssetSchemeAddress::from_hash(asset_type) {
            Some(address) => self.client.get_asset_scheme(address).map_err(errors::parcel_state),
            None => Ok(None),
        }
    }

    fn get_asset(&self, transaction_hash: H256, index: usize, block_number: Option<u64>) -> Result<Option<Asset>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        self.client.get_asset(transaction_hash, index, block_id).map_err(errors::parcel_state)
    }

    fn get_nonce(&self, address: H160, block_number: Option<u64>) -> Result<Option<U256>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.nonce(&address.into(), block_id))
    }

    fn get_balance(&self, address: H160, block_number: Option<u64>) -> Result<Option<U256>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.balance(&address.into(), block_id.into()))
    }

    fn get_regular_key(&self, address: H160, block_number: Option<u64>) -> Result<Option<Public>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.regular_key(&address.into(), block_id.into()))
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
        Ok(BlockNumberAndHash {
            number: self.client.chain_info().best_block_number,
            hash: self.client.chain_info().best_block_hash,
        })
    }

    fn get_block_hash(&self, block_number: u64) -> Result<Option<H256>> {
        Ok(self.client.block_hash(BlockId::Number(block_number)))
    }

    fn get_block_by_number(&self, block_number: u64) -> Result<Option<Block>> {
        Ok(self.client.block(BlockId::Number(block_number)).map(|block| block.decode().into()))
    }

    fn get_block_by_hash(&self, block_hash: H256) -> Result<Option<Block>> {
        Ok(self.client.block(BlockId::Hash(block_hash)).map(|block| block.decode().into()))
    }

    fn get_pending_parcels(&self) -> Result<Vec<Parcel>> {
        Ok(self.client.ready_parcels().into_iter().map(|signed| signed.into()).collect())
    }

    fn get_coinbase(&self) -> Result<Option<Address>> {
        if self.miner.author().is_zero() {
            Ok(None)
        } else {
            Ok(Some(self.miner.author()))
        }
    }

    fn execute_change_shard_state(&self, raw: Bytes) -> Result<Vec<ChangeShard>> {
        let transactions: Vec<Transaction> =
            UntrustedRlp::new(&raw.into_vec()).as_list().map_err(errors::rlp).map(Into::into)?;

        Ok(self.client.execute_transactions(&transactions).map_err(errors::core)?)
    }
}
