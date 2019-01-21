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

use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::vec::Vec;

use ccore::{DatabaseClient, MinerService, MiningBlockChainClient, COL_STATE};
use cjson::bytes::Bytes;
use cnetwork::IntoSocketAddr;
use csync::BlockSyncInfo;
use jsonrpc_core::Result;
use kvdb::KeyValueDB;
use primitives::H256;
use rlp::UntrustedRlp;

use super::super::errors;
use super::super::traits::Devel;

pub struct DevelClient<C, M, B>
where
    C: DatabaseClient + MiningBlockChainClient,
    M: MinerService,
    B: BlockSyncInfo, {
    client: Arc<C>,
    db: Arc<KeyValueDB>,
    miner: Arc<M>,
    block_sync: Option<Arc<B>>,
}

impl<C, M, B> DevelClient<C, M, B>
where
    C: DatabaseClient + MiningBlockChainClient,
    M: MinerService,
    B: BlockSyncInfo,
{
    pub fn new(client: Arc<C>, miner: Arc<M>, block_sync: Option<Arc<B>>) -> Self {
        let db = client.database();
        Self {
            client,
            db,
            miner,
            block_sync,
        }
    }
}

impl<C, M, B> Devel for DevelClient<C, M, B>
where
    C: DatabaseClient + MiningBlockChainClient + 'static,
    M: MinerService + 'static,
    B: BlockSyncInfo + 'static,
{
    fn get_state_trie_keys(&self, offset: usize, limit: usize) -> Result<Vec<H256>> {
        let iter = self.db.iter(COL_STATE);
        Ok(iter.skip(offset).take(limit).map(|val| H256::from(val.0.deref())).collect())
    }

    fn get_state_trie_value(&self, key: H256) -> Result<Vec<Bytes>> {
        match self.db.get(COL_STATE, &key).map_err(|e| errors::kvdb(&e))? {
            Some(value) => {
                let rlp = UntrustedRlp::new(&value);
                Ok(rlp.as_list::<Vec<u8>>().map_err(|e| errors::rlp(&e))?.into_iter().map(Bytes::from).collect())
            }
            None => Ok(Vec::new()),
        }
    }

    fn start_sealing(&self) -> Result<()> {
        self.miner.start_sealing(&*self.client);
        Ok(())
    }

    fn stop_sealing(&self) -> Result<()> {
        self.miner.stop_sealing();
        Ok(())
    }

    fn get_block_sync_peers(&self) -> Result<Vec<SocketAddr>> {
        if let Some(block_sync) = self.block_sync.as_ref() {
            Ok(block_sync.get_peers().into_iter().map(|node_id| node_id.into_addr().into()).collect())
        } else {
            Ok(Vec::new())
        }
    }
}
