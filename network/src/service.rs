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

use crate::client::Client;
use crate::control::{Control, Error as ControlError};
use crate::filters::{FilterEntry, FiltersControl};
use crate::routing_table::RoutingTable;
use crate::{p2p, Api, ManagingPeerdb, NetworkExtension, SocketAddr};
use cidr::IpCidr;
use cio::{IoError, IoService};
use ckey::{NetworkId, Public};
use crossbeam_channel::Sender;
use ctimer::TimerLoop;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

pub struct Service {
    p2p: IoService<p2p::Message>,
    client: Arc<Client>,
    routing_table: Arc<RoutingTable>,
    p2p_handler: Arc<p2p::Handler>,
    filters_control: Arc<dyn FiltersControl>,
}

impl Service {
    pub fn start(
        network_id: NetworkId,
        timer_loop: TimerLoop,
        address: SocketAddr,
        bootstrap_addresses: Vec<SocketAddr>,
        min_peers: usize,
        max_peers: usize,
        filters_control: Arc<dyn FiltersControl>,
        routing_table: Arc<RoutingTable>,
        peer_db: Box<dyn ManagingPeerdb>,
    ) -> Result<Arc<Self>, Error> {
        let p2p = IoService::start("P2P")?;

        let client = Client::new(p2p.channel(), timer_loop);

        let p2p_handler = Arc::new(p2p::Handler::try_new(
            p2p.channel(),
            network_id,
            address,
            Arc::clone(&client),
            Arc::clone(&routing_table),
            Arc::clone(&filters_control),
            bootstrap_addresses,
            min_peers,
            max_peers,
            peer_db,
        )?);
        p2p.register_handler(p2p_handler.clone())?;

        Ok(Arc::new(Self {
            p2p,
            client,
            routing_table,
            p2p_handler,
            filters_control,
        }))
    }

    pub fn register_extension<T, E, F>(&self, factory: F) -> Sender<E>
    where
        T: 'static + Sized + NetworkExtension<E>,
        E: 'static + Sized + Send,
        F: 'static + FnOnce(Box<dyn Api>) -> T + Send, {
        self.client.register_extension(factory)
    }

    pub fn connect_to(&self, address: SocketAddr) -> Result<(), String> {
        self.p2p.send_message(p2p::Message::RequestConnection(address)).map_err(|e| format!("{:?}", e))?;
        Ok(())
    }
}

impl Control for Service {
    fn local_key_for(&self, address: IpAddr, port: u16) -> Result<Public, ControlError> {
        self.routing_table.touch(SocketAddr::new(address, port)).ok_or(ControlError::NotConnected)
    }

    fn register_remote_key_for(
        &self,
        address: IpAddr,
        port: u16,
        remote_pub_key: Public,
    ) -> Result<Public, ControlError> {
        self.routing_table
            .register_remote_public(SocketAddr::new(address, port), remote_pub_key)
            .ok_or(ControlError::NotConnected)
    }

    fn connect(&self, addr: SocketAddr) -> Result<(), ControlError> {
        self.connect_to(addr).map_err(|e| {
            cwarn!(NETWORK, "Cannot connect to {}: {}", addr, e);
            ControlError::NotConnected
        })
    }

    fn disconnect(&self, addr: SocketAddr) -> Result<(), ControlError> {
        if !self.routing_table.is_established(&addr) {
            return Err(ControlError::NotConnected)
        }
        if let Err(err) = self.p2p.send_message(p2p::Message::Disconnect(addr)) {
            cerror!(NETWORK, "Error occurred while sending message Disconnect: {:?}", err);
        }
        Ok(())
    }

    fn is_connected(&self, addr: &SocketAddr) -> Result<bool, ControlError> {
        Ok(self.routing_table.is_established(addr))
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

    fn add_to_whitelist(&self, addr: IpCidr, tag: Option<String>) -> Result<(), ControlError> {
        self.filters_control.add_to_whitelist(addr, tag);
        Ok(())
    }

    fn remove_from_whitelist(&self, addr: &IpCidr) -> Result<(), ControlError> {
        self.filters_control.remove_from_whitelist(addr);
        if let Err(err) = self.p2p.send_message(p2p::Message::ApplyFilters) {
            cerror!(NETWORK, "Error occurred while apply filters: {:?}", err);
        }
        Ok(())
    }

    fn add_to_blacklist(&self, addr: IpCidr, tag: Option<String>) -> Result<(), ControlError> {
        self.filters_control.add_to_blacklist(addr, tag);
        if let Err(err) = self.p2p.send_message(p2p::Message::ApplyFilters) {
            cerror!(NETWORK, "Error occurred while apply filters: {:?}", err);
        }
        Ok(())
    }

    fn remove_from_blacklist(&self, addr: &IpCidr) -> Result<(), ControlError> {
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

    fn recent_network_usage(&self) -> Result<HashMap<String, usize>, ControlError> {
        Ok(self.p2p_handler.recent_network_usage())
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
