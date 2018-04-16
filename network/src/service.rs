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
use ctypes::Secret;

use super::client::Client;
use super::p2p;
use super::session_initiator;
use super::timer;
use super::{Api, DiscoveryApi, NetworkExtension, SocketAddr};

pub struct Service {
    _handshake_service: IoService<session_initiator::Message>,
    connection_service: IoService<p2p::Message>,
    timer_service: IoService<timer::Message>,
    client: Arc<Client>,
}

impl Service {
    pub fn start(
        address: SocketAddr,
        bootstrap_addresses: Vec<SocketAddr>,
        secret_key: Secret,
        discovery: Arc<DiscoveryApi>,
    ) -> Result<Self, IoError> {
        let connection_service = IoService::start()?;
        let connection_channel = connection_service.channel();

        let timer_service = IoService::start()?;

        let client = Client::new();
        let connection_handler = Arc::new(p2p::Handler::new(address.clone(), Arc::clone(&client)));
        discovery.set_address_converter(connection_handler.clone());
        connection_service.register_handler(connection_handler)?;
        timer_service.register_handler(Arc::new(timer::Handler::new(Arc::clone(&client))))?;

        let handshake_service = IoService::start()?;
        let handshake_handler =
            Arc::new(session_initiator::Handler::new(address, secret_key, connection_channel, discovery));
        handshake_service.register_handler(handshake_handler)?;

        for address in bootstrap_addresses {
            if let Err(err) = handshake_service.send_message(session_initiator::Message::ConnectTo(address)) {
                info!("Cannot ConnectTo : {:?}", err);
            }
        }
        Ok(Self {
            _handshake_service: handshake_service,
            connection_service,
            timer_service,
            client,
        })
    }

    pub fn register_extension(&self, extension: Arc<NetworkExtension>) -> Arc<Api> {
        let connection_channel = self.connection_service.channel();
        let timer_channel = self.timer_service.channel();
        self.client.register_extension(extension, connection_channel, timer_channel)
    }
}
