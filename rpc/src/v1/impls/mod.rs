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
    Asset, AssetAddress, AssetScheme, AssetSchemeAddress, BlockChainClient, BlockId, BlockInfo, ChainInfo, Client,
    Invoice, Miner, MinerService, Nonce, SignedTransaction,
};
use ctypes::{H160, H256, U256};
use rlp::UntrustedRlp;

use jsonrpc_core::Result;

use super::errors;
use super::traits::Chain;
use super::types::{Block, Bytes};

pub struct ChainClient {
    client: Arc<Client>,
    miner: Arc<Miner>,
}

impl ChainClient {
    pub fn new(client: &Arc<Client>, miner: &Arc<Miner>) -> Self {
        ChainClient {
            client: client.clone(),
            miner: miner.clone(),
        }
    }
}

impl Chain for ChainClient {
    fn send_signed_transaction(&self, raw: Bytes) -> Result<H256> {
        UntrustedRlp::new(&raw.into_vec())
            .as_val()
            .map_err(errors::rlp)
            .and_then(|tx| SignedTransaction::new(tx).map_err(errors::transaction))
            .and_then(|signed_transaction| {
                let hash = signed_transaction.hash();
                self.miner
                    .import_own_transaction(&*self.client, signed_transaction)
                    .map_err(errors::transaction)
                    .map(|_| hash)
            })
            .map(Into::into)
    }

    fn get_transaction_invoice(&self, hash: H256) -> Result<Option<Invoice>> {
        Ok(self.client.transaction_invoice(hash.into()))
    }

    fn get_asset_scheme(&self, transaction_hash: H256) -> Result<Option<AssetScheme>> {
        if let Some(state) = self.client.state_at(BlockId::Latest) {
            let address = AssetSchemeAddress::new(transaction_hash);
            Ok(state.asset_scheme(&address).map_err(errors::transaction)?)
        } else {
            Ok(None)
        }
    }

    fn get_asset(&self, transaction_hash: H256, index: usize) -> Result<Option<Asset>> {
        if let Some(state) = self.client.state_at(BlockId::Latest) {
            let address = AssetAddress::new(transaction_hash, index);
            Ok(state.asset(&address).map_err(errors::transaction)?)
        } else {
            Ok(None)
        }
    }

    fn get_nonce(&self, address: H160, block_number: Option<u64>) -> Result<Option<U256>> {
        let block_id = BlockId::Number(block_number.unwrap_or(self.client.chain_info().best_block_number));
        Ok(self.client.nonce(&address.into(), block_id))
    }

    fn get_block_number(&self) -> Result<u64> {
        Ok(self.client.chain_info().best_block_number)
    }

    fn get_block_hash(&self, block_number: u64) -> Result<Option<H256>> {
        Ok(self.client.block_hash(BlockId::Number(block_number)))
    }

    fn get_block_by_hash(&self, block_hash: H256) -> Result<Option<Block>> {
        Ok(self.client.block(BlockId::Hash(block_hash)).map(|block| block.decode().into()))
    }
}
