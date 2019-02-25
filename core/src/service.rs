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

use cio::{IoContext, IoHandler, IoHandlerResult, IoService};
use cnetwork::NodeId;
use ctimer::TimerApi;
use kvdb::KeyValueDB;
use primitives::{Bytes, H256};

use crate::client::{Client, ClientConfig};
use crate::error::Error;
use crate::miner::Miner;
use crate::scheme::Scheme;
use crate::BlockId;

/// Client service setup.
pub struct ClientService {
    _io_service: IoService<ClientIoMessage>,
    client: Arc<Client>,
}

impl ClientService {
    pub fn start(
        config: &ClientConfig,
        scheme: &Scheme,
        db: Arc<KeyValueDB>,
        miner: Arc<Miner>,
        reseal_timer: TimerApi,
    ) -> Result<ClientService, Error> {
        let io_service = IoService::<ClientIoMessage>::start("Client")?;

        let client = Client::try_new(config, &scheme, db, miner, io_service.channel(), reseal_timer)?;

        let client_io = Arc::new(ClientIoHandler {
            client: client.clone(),
        });
        io_service.register_handler(client_io)?;

        scheme.engine.register_client(Arc::downgrade(&client) as _);

        Ok(ClientService {
            _io_service: io_service,
            client,
        })
    }

    pub fn client(&self) -> Arc<Client> {
        Arc::clone(&self.client)
    }
}

/// Message type for external and internal events
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClientIoMessage {
    /// A block is ready
    BlockVerified,
    /// A header is ready
    HeaderVerified,
    /// New transaction RLPs are ready to be imported
    NewTransactions(Vec<Bytes>, NodeId),
    /// Block generation is required
    NewBlockRequired {
        parent_block: BlockId,
        allow_empty_block: bool,
    },
    /// Update the best block by the given hash
    /// Only used in Tendermint
    UpdateBestAsCommitted(H256),
}

/// IO interface for the Client handler
struct ClientIoHandler {
    client: Arc<Client>,
}

impl IoHandler<ClientIoMessage> for ClientIoHandler {
    fn message(&self, _io: &IoContext<ClientIoMessage>, net_message: ClientIoMessage) -> IoHandlerResult<()> {
        match net_message {
            ClientIoMessage::BlockVerified => {
                self.client.import_verified_blocks();
            }
            ClientIoMessage::HeaderVerified => {
                self.client.import_verified_headers();
            }
            ClientIoMessage::NewTransactions(transactions, peer_id) => {
                self.client.import_queued_transactions(&transactions, peer_id);
            }
            ClientIoMessage::NewBlockRequired {
                parent_block,
                allow_empty_block,
            } => {
                self.client.update_sealing(parent_block, allow_empty_block);
            }
            ClientIoMessage::UpdateBestAsCommitted(block_hash) => {
                self.client.update_best_as_committed(block_hash);
            }
        }
        Ok(())
    }
}
