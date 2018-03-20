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

use std::sync::{Arc, Weak};

use cbytes::Bytes;
use cio::IoChannel;
use ctypes::{Address, H256};
use kvdb::KeyValueDB;
use parking_lot::{Mutex, RwLock};

use super::{EngineClient, BlockChainInfo, BlockInfo, ChainInfo, ChainNotify, ClientConfig};
use super::importer::Importer;
use super::super::blockchain::{BlockChain, BlockProvider};
use super::super::codechain_machine::CodeChainMachine;
use super::super::consensus::{CodeChainEngine, Solo};
use super::super::encoded;
use super::super::error::Error;
use super::super::service::ClientIoMessage;
use super::super::spec::Spec;
use super::super::types::BlockId;

pub struct Client {
    engine: Arc<CodeChainEngine>,
    io_channel: Mutex<IoChannel<ClientIoMessage>>,

    chain: RwLock<Arc<BlockChain>>,

    /// List of actors to be notified on certain chain events
    notify: RwLock<Vec<Weak<ChainNotify>>>,

    importer: Importer,
}

impl Client {
    pub fn new(
        config: ClientConfig,
        spec: &Spec,
        db: Arc<KeyValueDB>,
        message_channel: IoChannel<ClientIoMessage>,
    ) -> Result<Arc<Client>, Error> {
        let engine = spec.engine.clone();
        let gb = spec.genesis_block();
        let chain = Arc::new(BlockChain::new(&gb, db.clone()));

        let importer = Importer::new(&config, engine.clone())?;

        let client = Arc::new(Client {
            engine,
            io_channel: Mutex::new(message_channel),
            chain: RwLock::new(chain),
            notify: RwLock::new(Vec::new()),
            importer,
        });

        Ok(client)
    }

    /// Returns engine reference.
    pub fn engine(&self) -> &CodeChainEngine {
        &*self.engine
    }

    /// Adds an actor to be notified on certain events
    pub fn add_notify(&self, target: Arc<ChainNotify>) {
        self.notify.write().push(Arc::downgrade(&target));
    }

    fn notify<F>(&self, f: F) where F: Fn(&ChainNotify) {
        for np in self.notify.read().iter() {
            if let Some(n) = np.upgrade() {
                f(&*n);
            }
        }
    }

    fn block_hash(chain: &BlockChain, id: BlockId) -> Option<H256> {
        match id {
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(number) => chain.block_hash(number),
            BlockId::Earliest => chain.block_hash(0),
            BlockId::Latest => Some(chain.best_block_hash()),
        }
    }
}

impl ChainInfo for Client {
    fn chain_info(&self) -> BlockChainInfo {
        let mut chain_info = self.chain.read().chain_info();
        // FIXME:: Take block_queue into consideration.
        chain_info
    }
}

impl EngineClient for Client {
    /// Broadcast a consensus message to the network.
    fn broadcast_consensus_message(&self, message: Bytes) {
        self.notify(|notify| notify.broadcast(message.clone()));
    }

    /// Make a new block and seal it.
    fn update_sealing(&self) {
        unimplemented!()
    }

    /// Submit a seal for a block in the mining queue.
    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>) {
        unimplemented!()
    }
}

impl BlockInfo for Client {
    fn block_header(&self, id: BlockId) -> Option<::encoded::Header> {
        let chain = self.chain.read();

        Self::block_hash(&chain, id).and_then(|hash| chain.block_header_data(&hash))
    }

    fn best_block_header(&self) -> encoded::Header {
        self.chain.read().best_block_header()
    }

    fn block(&self, id: BlockId) -> Option<encoded::Block> {
        let chain = self.chain.read();

        Self::block_hash(&chain, id).and_then(|hash| {
            chain.block(&hash)
        })
    }
}
