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

use cbytes::Bytes;
use cio::IoChannel;
use ctypes::H256;
use parking_lot::Mutex;

use super::{EngineClient, BlockChainInfo, ChainInfo};
use super::super::consensus::{ConsensusEngine, Solo};
use super::super::codechain_machine::CodeChainMachine;
use super::super::error::Error;
use super::super::service::ClientIoMessage;

pub struct Client {
    engine: Arc<ConsensusEngine<CodeChainMachine>>,
    io_channel: Mutex<IoChannel<ClientIoMessage>>,
}

impl Client {
    pub fn new(
        message_channel: IoChannel<ClientIoMessage>,
    ) -> Result<Arc<Client>, Error> {
        // FIXME: Make it possible to choose the consensus engine.
        let machine = CodeChainMachine::new();
        let engine = Solo::new(machine);

        let client = Arc::new(Client {
            engine: Arc::new(engine),
            io_channel: Mutex::new(message_channel),
        });

        Ok(client)
    }

    /// Returns engine reference.
    pub fn engine(&self) -> &ConsensusEngine<CodeChainMachine> {
        &*self.engine
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
        unimplemented!()
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

