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

use std::fmt;
use std::net::IpAddr;

use super::SocketAddr;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, RlpEncodableWrapper, RlpDecodableWrapper)]
pub struct NodeId {
    addr: SocketAddr,
}

impl NodeId {
    pub fn new(ip: IpAddr, port: u16) -> Self {
        Self {
            addr: SocketAddr::new(ip, port),
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let internal = &self.addr;
        let port = internal.port();
        match internal.ip() {
            IpAddr::V4(ip) if ip.is_loopback() => write!(f, "Local V4:{}", port),
            IpAddr::V4(ip) if ip.is_private() => write!(f, "Private {}:{}", ip, port),
            IpAddr::V4(ip) => write!(f, "Global {}:{}", ip, port),
            IpAddr::V6(_) => unimplemented!(),
        }
    }
}

pub trait IntoSocketAddr {
    fn into_addr(self) -> SocketAddr;
}

impl IntoSocketAddr for NodeId {
    fn into_addr(self) -> SocketAddr {
        self.addr
    }
}
