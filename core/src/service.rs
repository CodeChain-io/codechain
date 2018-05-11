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

use std::path::Path;
use std::sync::Arc;

use cbytes::Bytes;
use cio::{IoContext, IoHandler, IoHandlerResult, IoService};
use kvdb_rocksdb::{Database, DatabaseConfig};

use super::client::{Client, ClientConfig};
use super::error::Error;
use super::miner::Miner;
use super::spec::Spec;

/// Client service setup.
pub struct ClientService {
    io_service: Arc<IoService<ClientIoMessage>>,
    client: Arc<Client>,
    database: Arc<Database>,
}

impl ClientService {
    pub fn start(
        config: ClientConfig,
        spec: &Spec,
        client_path: &Path,
        miner: Arc<Miner>,
    ) -> Result<ClientService, Error> {
        let io_service = IoService::<ClientIoMessage>::start()?;

        let mut db_config = DatabaseConfig::with_columns(super::db::NUM_COLUMNS);

        db_config.memory_budget = config.db_cache_size;
        db_config.compaction = config.db_compaction.compaction_profile(client_path);
        db_config.wal = config.db_wal;

        let db = Arc::new(Database::open(
            &db_config,
            &client_path.to_str().expect("DB path could not be converted to string."),
        ).map_err(::client::Error::Database)?);

        let client = Client::new(config, &spec, db.clone(), miner, io_service.channel())?;

        let client_io = Arc::new(ClientIoHandler {
            client: client.clone(),
        });
        io_service.register_handler(client_io)?;

        spec.engine.register_client(Arc::downgrade(&client) as _);

        Ok(ClientService {
            io_service: Arc::new(io_service),
            client,
            database: db,
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
    /// New parcel RLPs are ready to be imported
    NewParcels(Vec<Bytes>, usize),
}

/// IO interface for the Client handler
struct ClientIoHandler {
    client: Arc<Client>,
}

impl IoHandler<ClientIoMessage> for ClientIoHandler {
    fn message(&self, _io: &IoContext<ClientIoMessage>, net_message: &ClientIoMessage) -> IoHandlerResult<()> {
        match net_message {
            ClientIoMessage::BlockVerified => {
                self.client.import_verified_blocks();
            }
            ClientIoMessage::NewParcels(parcels, peer_id) => {
                self.client.import_queued_parcels(parcels, *peer_id);
            }
        }
        Ok(())
    }
}
