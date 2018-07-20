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

use ckey::Address;
use std::sync::Arc;

use ccore::{AssetClient, BlockId, MinerService, MiningBlockChainClient, RegularKey, Shard, SignedParcel};
use ckey::Public;
use cstate::{Asset, AssetScheme};
use ctypes::invoice::{Invoice, ParcelInvoice};
use ctypes::BlockNumber;
use primitives::{H160, H256, U256};
use rlp::UntrustedRlp;

use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Chain;
use super::super::types::{Block, BlockNumberAndHash, Bytes, Parcel};

pub struct ChainClient<C, M>
where
    C: AssetClient + MiningBlockChainClient + Shard + RegularKey,
    M: MinerService, {
    client: Arc<C>,
    miner: Arc<M>,
}

impl<C, M> ChainClient<C, M>
where
    C: AssetClient + MiningBlockChainClient + Shard + RegularKey,
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
    C: AssetClient + MiningBlockChainClient + Shard + RegularKey + 'static,
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

    fn get_asset_scheme(&self, transaction_hash: H256) -> Result<Option<AssetScheme>> {
        self.client.get_asset_scheme(transaction_hash).map_err(errors::parcel_state)
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


    fn get_number_of_shards(&self, block_number: Option<u64>) -> Result<Option<u32>> {
        let block_id = block_number.map(BlockId::Number).unwrap_or(BlockId::Latest);
        Ok(self.client.number_of_shards(block_id.into()))
    }

    fn get_shard_root(&self, shard_id: u32, block_number: Option<u64>) -> Result<Option<H256>> {
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

    fn get_coinbase(&self) -> Result<Address> {
        Ok(self.miner.author())
    }
}
