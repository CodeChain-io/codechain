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

use super::client::Client;
use super::p2p;
use super::session_initiator;
use super::timer;
use super::{DiscoveryApi, NetworkExtension, SocketAddr};

pub struct Service {
    session_initiator: IoService<session_initiator::Message>,
    session_initiator_handler: Arc<session_initiator::Handler>,
    p2p: IoService<p2p::Message>,
    timer: IoService<timer::Message>,
    client: Arc<Client>,
}

impl Service {
    pub fn start(address: SocketAddr, min_peers: usize, max_peers: usize) -> Result<Self, Error> {
        let p2p = IoService::start()?;
        let timer = IoService::start()?;
        let session_initiator = IoService::start()?;

        let client = Client::new(p2p.channel(), timer.channel());

        let p2p_handler = Arc::new(p2p::Handler::try_new(address.clone(), Arc::clone(&client), min_peers, max_peers)?);
        p2p.register_handler(p2p_handler)?;

        timer.register_handler(Arc::new(timer::Handler::new(Arc::clone(&client))))?;

        let session_initiator_handler = Arc::new(session_initiator::Handler::new(address, p2p.channel()));
        session_initiator.register_handler(session_initiator_handler.clone())?;

        Ok(Self {
            session_initiator,
            session_initiator_handler,
            p2p,
            timer,
            client,
        })
    }

    pub fn register_extension(&self, extension: Arc<NetworkExtension>) -> Result<(), String> {
        let extension_name = extension.name();
        self.client.register_extension(extension);
        if let Err(err) = self.timer.send_message(timer::Message::InitializeExtension {
            extension_name,
        }) {
            Err(format!("{:?}", err))
        } else {
            Ok(())
        }
    }

    pub fn set_discovery_api(&self, api: Arc<DiscoveryApi>) {
        self.session_initiator_handler.set_discovery_api(api);
    }

    pub fn connect_to(&self, address: SocketAddr) -> Result<(), String> {
        if let Err(err) = self.session_initiator.send_message(session_initiator::Message::ConnectTo(address)) {
            return Err(format!("{:?}", err))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
    General(String),
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::IoError(err)
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::General(err)
    }
}
