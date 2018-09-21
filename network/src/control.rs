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
use std::result::Result;

use primitives::H256;

use super::addr::SocketAddr;
use super::filters::FilterEntry;

pub trait Control: Send + Sync {
    fn register_secret(&self, secret: H256, addr: SocketAddr) -> Result<(), Error>;
    fn connect(&self, addr: SocketAddr) -> Result<(), Error>;
    fn disconnect(&self, addr: SocketAddr) -> Result<(), Error>;
    fn is_connected(&self, addr: &SocketAddr) -> Result<bool, Error>;
    fn get_port(&self) -> Result<u16, Error>;
    fn get_peer_count(&self) -> Result<usize, Error>;
    fn established_peers(&self) -> Result<Vec<SocketAddr>, Error>;

    fn add_to_whitelist(&self, addr: IpAddr, tag: Option<String>) -> Result<(), Error>;
    fn remove_from_whitelist(&self, addr: &IpAddr) -> Result<(), Error>;

    fn add_to_blacklist(&self, addr: IpAddr, tag: Option<String>) -> Result<(), Error>;
    fn remove_from_blacklist(&self, addr: &IpAddr) -> Result<(), Error>;

    fn enable_whitelist(&self) -> Result<(), Error>;
    fn disable_whitelist(&self) -> Result<(), Error>;

    fn enable_blacklist(&self) -> Result<(), Error>;
    fn disable_blacklist(&self) -> Result<(), Error>;

    fn get_whitelist(&self) -> Result<(Vec<FilterEntry>, bool), Error>;
    fn get_blacklist(&self) -> Result<(Vec<FilterEntry>, bool), Error>;
}

#[derive(Clone, Debug)]
pub enum Error {
    Disabled,
    NotConnected,
}
