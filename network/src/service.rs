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


use std::net::IpAddr;
use std::sync::Arc;

use cio::{IoError, IoService};
use ctimer::TimerLoop;
use primitives::H256;

use crate::client::Client;
use crate::control::{Control, Error as ControlError};
use crate::filters::{FilterEntry, FiltersControl};
use crate::p2p;
use crate::routing_table::RoutingTable;
use crate::session_initiator;
use crate::DiscoveryApi;
use crate::{Api, NetworkExtension, SocketAddr};

pub struct Service {
    session_initiator: IoService<session_initiator::Message>,
    p2p: IoService<p2p::Message>,
    client: Arc<Client>,
    routing_table: Arc<RoutingTable>,
    p2p_handler: Arc<p2p::Handler>,
    filters_control: Arc<FiltersControl>,
}

impl Service {
    pub fn start(
        timer_loop: TimerLoop,
        address: SocketAddr,
        min_peers: usize,
        max_peers: usize,
        filters_control: Arc<FiltersControl>,
    ) -> Result<Arc<Self>, Error> {
        let p2p = IoService::start("P2P")?;
        let session_initiator = IoService::start("SessionInitiator")?;

        let routing_table = RoutingTable::new();

        let client = Client::new(p2p.channel(), timer_loop);

        let p2p_handler = Arc::new(p2p::Handler::try_new(
            address,
            Arc::clone(&client),
            Arc::clone(&routing_table),
            Arc::clone(&filters_control),
            min_peers,
            max_peers,
        )?);
        p2p.register_handler(p2p_handler.clone())?;

        let session_initiator_handler = Arc::new(session_initiator::Handler::new(
            address,
            Arc::clone(&routing_table),
            p2p.channel(),
            Arc::clone(&filters_control),
        ));
        session_initiator.register_handler(session_initiator_handler.clone())?;

        Ok(Arc::new(Self {
            session_initiator,
            p2p,
            client,
            routing_table,
            p2p_handler,
            filters_control,
        }))
    }

    pub fn new_extension<T, F>(&self, factory: F) -> Arc<T>
    where
        T: 'static + Sized + NetworkExtension,
        F: FnOnce(Arc<Api>) -> T, {
        self.client.new_extension(factory)
    }

    pub fn connect_to(&self, address: SocketAddr) -> Result<(), String> {
        if let Err(err) = self.session_initiator.send_message(session_initiator::Message::ConnectTo(address)) {
            return Err(format!("{:?}", err))
        } else {
            Ok(())
        }
    }

    pub fn set_routing_table(&self, disc: &DiscoveryApi) {
        disc.set_routing_table(Arc::clone(&self.routing_table));
    }
}

impl Control for Service {
    fn register_secret(&self, secret: H256, addr: SocketAddr) -> Result<(), ControlError> {
        let message = session_initiator::Message::PreimportSecret(secret, addr);
        if let Err(err) = self.session_initiator.send_message(message) {
            cerror!(NETWORK, "Error occurred while sending message PreimportSecret : {:?}", err);
        }
        Ok(())
    }

    fn connect(&self, addr: SocketAddr) -> Result<(), ControlError> {
        let message = session_initiator::Message::ManuallyConnectTo(addr);
        if let Err(err) = self.session_initiator.send_message(message) {
            cerror!(NETWORK, "Error occurred while sending message ManuallyConnectTo: {:?}", err);
        }
        Ok(())
    }

    fn disconnect(&self, addr: SocketAddr) -> Result<(), ControlError> {
        if !self.routing_table.is_connected(&addr) {
            return Err(ControlError::NotConnected)
        }
        if let Err(err) = self.p2p.send_message(p2p::Message::Disconnect(addr)) {
            cerror!(NETWORK, "Error occurred while sending message Disconnect: {:?}", err);
        }
        Ok(())
    }

    fn is_connected(&self, addr: &SocketAddr) -> Result<bool, ControlError> {
        Ok(self.routing_table.is_connected(addr))
    }

    fn get_port(&self) -> Result<u16, ControlError> {
        Ok(self.p2p_handler.get_port())
    }

    fn get_peer_count(&self) -> Result<usize, ControlError> {
        Ok(self.p2p_handler.get_peer_count())
    }

    fn established_peers(&self) -> Result<Vec<SocketAddr>, ControlError> {
        Ok(self.p2p_handler.established_peers())
    }

    fn add_to_whitelist(&self, addr: IpAddr, tag: Option<String>) -> Result<(), ControlError> {
        self.filters_control.add_to_whitelist(addr, tag);
        Ok(())
    }

    fn remove_from_whitelist(&self, addr: &IpAddr) -> Result<(), ControlError> {
        self.filters_control.remove_from_whitelist(addr);
        if let Err(err) = self.p2p.send_message(p2p::Message::ApplyFilters) {
            cerror!(NETWORK, "Error occurred while apply filters: {:?}", err);
        }
        Ok(())
    }

    fn add_to_blacklist(&self, addr: IpAddr, tag: Option<String>) -> Result<(), ControlError> {
        self.filters_control.add_to_blacklist(addr, tag);
        if let Err(err) = self.p2p.send_message(p2p::Message::ApplyFilters) {
            cerror!(NETWORK, "Error occurred while apply filters: {:?}", err);
        }
        Ok(())
    }

    fn remove_from_blacklist(&self, addr: &IpAddr) -> Result<(), ControlError> {
        self.filters_control.remove_from_blacklist(addr);
        Ok(())
    }

    fn enable_whitelist(&self) -> Result<(), ControlError> {
        self.filters_control.enable_whitelist();
        if let Err(err) = self.p2p.send_message(p2p::Message::ApplyFilters) {
            cerror!(NETWORK, "Error occurred while apply filters: {:?}", err);
        }
        Ok(())
    }

    fn disable_whitelist(&self) -> Result<(), ControlError> {
        self.filters_control.disable_whitelist();
        Ok(())
    }

    fn enable_blacklist(&self) -> Result<(), ControlError> {
        self.filters_control.enable_blacklist();
        if let Err(err) = self.p2p.send_message(p2p::Message::ApplyFilters) {
            cerror!(NETWORK, "Error occurred while apply filters: {:?}", err);
        }
        Ok(())
    }

    fn disable_blacklist(&self) -> Result<(), ControlError> {
        self.filters_control.disable_blacklist();
        Ok(())
    }

    fn get_whitelist(&self) -> Result<(Vec<FilterEntry>, bool), ControlError> {
        Ok(self.filters_control.get_whitelist())
    }

    fn get_blacklist(&self) -> Result<(Vec<FilterEntry>, bool), ControlError> {
        Ok(self.filters_control.get_blacklist())
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
