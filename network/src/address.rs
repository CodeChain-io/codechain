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
use std::fmt;
use std::net::{AddrParseError, IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

#[derive(Clone, Debug, Eq, Hash)]
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

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Address({})", self.addr)
    }
}

impl Encodable for Address {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self.ip() {
            IpAddr::V4(ref addr) => {
                let octets = addr.octets();
                assert_eq!(4, octets.len());
                s.begin_list(octets.len() + 1);
                for octet in octets.iter() {
                    s.append(octet);
                }
                s.append(&self.port());
            },
            IpAddr::V6(ref addr) => {
                let octets = addr.octets();
                assert_eq!(16, octets.len());
                s.begin_list(octets.len() + 1);
                for octet in octets.iter() {
                    s.append(octet);
                }
                s.append(&self.port());
            },
        }
    }
}

impl Decodable for Address {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.item_count()? {
            5 => {
                let ip0 = rlp.val_at(0)?;
                let ip1 = rlp.val_at(1)?;
                let ip2 = rlp.val_at(2)?;
                let ip3 = rlp.val_at(3)?;
                let port = rlp.val_at(4)?;
                Ok(Address::v4(ip0, ip1, ip2, ip3, port))
            },
            17 => {
                let mut octets: [u8; 16] = [0; 16];
                for i in 0..16 {
                    octets[i] = rlp.val_at(i)?;
                }
                let port = rlp.val_at(16)?;
                let ip = IpAddr::V6(Ipv6Addr::from(octets));
                Ok(Address::new(ip, port))
            },
            _ => Err(DecoderError::RlpIncorrectListLen),
        }
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
