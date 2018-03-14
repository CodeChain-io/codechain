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

use std::cmp::Ordering;
use std::convert::From;
use std::net::{ AddrParseError, IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr };
use std::str::FromStr;


#[derive(Clone, Debug, Eq)]
pub struct Address {
    addr: SocketAddr,
}

impl Address {
    pub fn new(ip: IpAddr, port: u16) -> Self {
        Address::from(SocketAddr::new(ip, port))
    }

    pub fn v4(a: u8, b: u8, c: u8, d: u8, port: u16) -> Self {
        Address::new(IpAddr::V4(Ipv4Addr::new(a, b, c, d)), port)
    }

    pub fn v6(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16, port: u16) -> Self {
        Address::new(IpAddr::V6(Ipv6Addr::new(a, b, c, d, e, f, g, h)), port)
    }

    pub fn ip(&self) -> IpAddr {
        self.addr.ip()
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn socket(&self) -> &SocketAddr {
        &self.addr
    }
}

impl From<SocketAddr> for Address {
    fn from(addr: SocketAddr) -> Self {
        Self {
            addr,
        }
    }
}

impl FromStr for Address {
    type Err = AddrParseError;
    fn from_str(addr: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(SocketAddr::from_str(addr)?))
    }
}

impl Ord for Address {
    fn cmp(&self, other: &Address) -> Ordering {
        match (self.addr, other.addr) {
            (SocketAddr::V4(_), SocketAddr::V6(_)) => Ordering::Less,
            (SocketAddr::V6(_), SocketAddr::V4(_)) => Ordering::Greater,
            (lhs, rhs) => {
                match lhs.ip().cmp(&rhs.ip()) {
                    Ordering::Equal => lhs.port().cmp(&rhs.port()),
                    order => order,
                }
            },
        }
    }
}

impl PartialEq for Address {
    fn eq(&self, other: &Address ) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for Address {
    fn partial_cmp(&self, other: &Address) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use super::Address;

    #[test]
    fn test_addresss_are_equal_if_they_have_same_id_and_port() {
        let a1 = Address::v4(127, 0, 0, 1, 3485);
        let a2 = Address::v4(127, 0, 0, 1, 3485);
        assert_eq!(a1, a2);
    }

    #[test]
    fn test_addresss_are_not_equal_if_their_ip_is_different() {
        let a1 = Address::v4(127, 0, 0, 1, 3485);
        let a2 = Address::v4(192, 168, 0, 1, 3485);
        assert_ne!(a1, a2);
    }

    #[test]
    fn test_addresss_are_not_equal_if_their_port_is_different() {
        let a1 = Address::v4(127, 0, 0, 1, 3485);
        let a2 = Address::v4(127, 0, 0, 1, 3486);
        assert_ne!(a1, a2);
    }

    #[test]
    fn test_address_is_less_than_if_port_is_less() {
        let a1 = Address::v4(127, 0, 0, 1, 3485);
        let a2 = Address::v4(127, 0, 0, 1, 3486);
        assert_eq!(Ordering::Less, a1.cmp(&a2));
    }

    #[test]
    fn test_address_is_greater_than_if_port_is_greater() {
        let a1 = Address::v4(127, 0, 0, 1, 3485);
        let a2 = Address::v4(127, 0, 0, 1, 3484);
        assert_eq!(Ordering::Greater, a1.cmp(&a2));
    }
}
