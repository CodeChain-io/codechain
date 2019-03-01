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

use cidr::IpCidr;
use std::net::IpAddr;

use ckey::Public;
use cnetwork::{FilterEntry, NetworkControl, NetworkControlError, SocketAddr};

pub struct DummyNetworkService {}

impl DummyNetworkService {
    pub fn new() -> Self {
        DummyNetworkService {}
    }
}

impl NetworkControl for DummyNetworkService {
    fn local_key_for(&self, _: IpAddr, _port: u16) -> Result<Public, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn register_remote_key_for(
        &self,
        _: IpAddr,
        _port: u16,
        _remote_pub_key: Public,
    ) -> Result<Public, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn connect(&self, _addr: SocketAddr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn disconnect(&self, _addr: SocketAddr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn is_connected(&self, _addr: &SocketAddr) -> Result<bool, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn get_port(&self) -> Result<u16, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn get_peer_count(&self) -> Result<usize, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn established_peers(&self) -> Result<Vec<SocketAddr>, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn add_to_whitelist(&self, _addr: IpCidr, _tag: Option<String>) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn remove_from_whitelist(&self, _addr: &IpCidr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn add_to_blacklist(&self, _addr: IpCidr, _tag: Option<String>) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn remove_from_blacklist(&self, _addr: &IpCidr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn enable_whitelist(&self) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn disable_whitelist(&self) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn enable_blacklist(&self) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn disable_blacklist(&self) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn get_whitelist(&self) -> Result<(Vec<FilterEntry>, bool), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn get_blacklist(&self) -> Result<(Vec<FilterEntry>, bool), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }
}
