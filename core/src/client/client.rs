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
use ctypes::H256;
use kvdb::KeyValueDB;
use parking_lot::{Mutex, RwLock};

use super::super::blockchain::BlockChain;
use super::{EngineClient, BlockChainInfo, ChainInfo, ChainNotify};
use super::super::codechain_machine::CodeChainMachine;
use super::super::consensus::{CodeChainEngine, Solo};
use super::super::error::Error;
use super::super::service::ClientIoMessage;
use super::super::spec::Spec;

pub struct Client {
    engine: Arc<CodeChainEngine>,
    io_channel: Mutex<IoChannel<ClientIoMessage>>,

    chain: RwLock<Arc<BlockChain>>,

    /// List of actors to be notified on certain chain events
    notify: RwLock<Vec<Weak<ChainNotify>>>,
}

impl Client {
    pub fn new(
        spec: &Spec,
        db: Arc<KeyValueDB>,
        message_channel: IoChannel<ClientIoMessage>,
    ) -> Result<Arc<Client>, Error> {
        let engine = spec.engine.clone();
        let gb = spec.genesis_block();
        let chain = Arc::new(BlockChain::new(&gb, db.clone()));

        let client = Arc::new(Client {
            engine,
            io_channel: Mutex::new(message_channel),
            chain: RwLock::new(chain),
            notify: RwLock::new(Vec::new()),
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
}

impl ChainInfo for Client {
    fn chain_info(&self) -> BlockChainInfo {
        unimplemented!()
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

