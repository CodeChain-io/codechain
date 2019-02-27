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

use std::net::{self, IpAddr};
use std::sync::Arc;

use cidr::IpCidr;
use ckey::Public;
use cnetwork::{NetworkControl, SocketAddr};
use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Net;
use super::super::types::FilterStatus;

pub struct NetClient {
    network_control: Arc<NetworkControl>,
}

impl NetClient {
    pub fn new(network_control: Arc<NetworkControl>) -> Self {
        Self {
            network_control,
        }
    }
}

impl Net for NetClient {
    fn local_key_for(&self, address: IpAddr, port: u16) -> Result<Public> {
        self.network_control.local_key_for(address, port).map_err(|e| errors::network_control(&e))
    }

    fn register_remote_key_for(&self, address: IpAddr, port: u16, remote_pub_key: Public) -> Result<Public> {
        self.network_control
            .register_remote_key_for(address, port, remote_pub_key)
            .map_err(|e| errors::network_control(&e))
    }

    fn connect(&self, address: IpAddr, port: u16) -> Result<()> {
        self.network_control.connect(SocketAddr::new(address, port)).map_err(|e| errors::network_control(&e))?;
        Ok(())
    }

    fn disconnect(&self, address: IpAddr, port: u16) -> Result<()> {
        self.network_control.disconnect(SocketAddr::new(address, port)).map_err(|e| errors::network_control(&e))?;
        Ok(())
    }

    fn is_connected(&self, address: IpAddr, port: u16) -> Result<bool> {
        Ok(self
            .network_control
            .is_connected(&SocketAddr::new(address, port))
            .map_err(|e| errors::network_control(&e))?)
    }

    fn get_port(&self) -> Result<u16> {
        Ok(self.network_control.get_port().map_err(|e| errors::network_control(&e))?)
    }

    fn get_peer_count(&self) -> Result<usize> {
        Ok(self.network_control.get_peer_count().map_err(|e| errors::network_control(&e))?)
    }

    fn get_established_peers(&self) -> Result<Vec<net::SocketAddr>> {
        let peers = self.network_control.established_peers().map_err(|e| errors::network_control(&e))?;
        Ok(peers.into_iter().map(Into::into).collect())
    }

    fn add_to_whitelist(&self, addr: IpCidr, tag: Option<String>) -> Result<()> {
        self.network_control.add_to_whitelist(addr, tag).map_err(|e| errors::network_control(&e))
    }

    fn remove_from_whitelist(&self, addr: IpCidr) -> Result<()> {
        self.network_control.remove_from_whitelist(&addr).map_err(|e| errors::network_control(&e))
    }

    fn add_to_blacklist(&self, addr: IpCidr, tag: Option<String>) -> Result<()> {
        self.network_control.add_to_blacklist(addr, tag).map_err(|e| errors::network_control(&e))
    }

    fn remove_from_blacklist(&self, addr: IpCidr) -> Result<()> {
        self.network_control.remove_from_blacklist(&addr).map_err(|e| errors::network_control(&e))
    }

    fn enable_whitelist(&self) -> Result<()> {
        self.network_control.enable_whitelist().map_err(|e| errors::network_control(&e))
    }

    fn disable_whitelist(&self) -> Result<()> {
        self.network_control.disable_whitelist().map_err(|e| errors::network_control(&e))
    }

    fn enable_blacklist(&self) -> Result<()> {
        self.network_control.enable_blacklist().map_err(|e| errors::network_control(&e))
    }

    fn disable_blacklist(&self) -> Result<()> {
        self.network_control.disable_blacklist().map_err(|e| errors::network_control(&e))
    }

    fn get_whitelist(&self) -> Result<FilterStatus> {
        let (list, enabled) = self.network_control.get_whitelist().map_err(|e| errors::network_control(&e))?;
        Ok(FilterStatus {
            list: list.into_iter().map(|x| (x.cidr, x.tag)).collect(),
            enabled,
        })
    }

    fn get_blacklist(&self) -> Result<FilterStatus> {
        let (list, enabled) = self.network_control.get_blacklist().map_err(|e| errors::network_control(&e))?;
        Ok(FilterStatus {
            list: list.into_iter().map(|x| (x.cidr, x.tag)).collect(),
            enabled,
        })
    }
}
