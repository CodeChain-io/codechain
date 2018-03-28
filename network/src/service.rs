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

use cio::{IoError, IoService};

use super::{Address, Api, Extension};
use super::client::Client;
use super::connection;
use super::handshake;

pub struct Service {
    handshake_service: IoService<handshake::HandlerMessage>,
    extension_service: IoService<connection::HandlerMessage>,
    client: Arc<Client>,
}

impl Service {
    pub fn start(address: Address, bootstrap_addresses: Vec<Address>) -> Result<Self, IoError> {
        let extension_service = IoService::start()?;
        let extension_channel =  extension_service.channel();

        let client = Client::new();
        extension_service.register_handler(Arc::new(connection::Handler::new(address.clone(), Arc::clone(&client))))?;

        let handshake_service = IoService::start()?;
        handshake_service.register_handler(Arc::new(handshake::Handler::new(address, extension_channel)))?;

        for address in bootstrap_addresses {
            if let Err(err) = handshake_service.send_message(handshake::HandlerMessage::ConnectTo(address)) {
                info!("Cannot ConnectTo : {:?}", err);
            }
        }
        Ok(Self {
            handshake_service,
            extension_service,
            client,
        })
    }

    pub fn register_extension(&self, extension: Arc<Extension>) -> Arc<Api> {
        Client::register_extension(Arc::clone(&self.client), extension)
    }
}
