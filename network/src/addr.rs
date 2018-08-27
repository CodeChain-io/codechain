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
use std::convert::{From, Into};
use std::fmt;
use std::net::{self, AddrParseError, IpAddr, Ipv4Addr};
use std::str::FromStr;

use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::NodeId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SocketAddr {
    addr: net::SocketAddr,
}

impl SocketAddr {
    pub fn new(ip: IpAddr, port: u16) -> Self {
        SocketAddr::from(net::SocketAddr::new(ip, port))
    }

    pub fn v4(a: u8, b: u8, c: u8, d: u8, port: u16) -> Self {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(a, b, c, d)), port)
    }

    pub fn v6(_a: u16, _b: u16, _c: u16, _d: u16, _e: u16, _f: u16, _g: u16, _h: u16, _port: u16) -> Self {
        unimplemented!();
    }

    pub fn ip(&self) -> IpAddr {
        self.addr.ip()
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn is_global(&self) -> bool {
        match self.ip() {
            net::IpAddr::V4(ip) => !ip.is_loopback() && !ip.is_private(),
            net::IpAddr::V6(_ip) => unimplemented!(),
        }
    }

    pub fn is_reachable(&self, other: &SocketAddr) -> bool {
        if self == other {
            return false
        }
        match (self.ip(), other.ip()) {
            (net::IpAddr::V4(self_ip), net::IpAddr::V4(other_ip)) => {
                debug_assert_eq!(false, other_ip.is_link_local());
                debug_assert_eq!(false, other_ip.is_broadcast());
                debug_assert_eq!(false, other_ip.is_multicast());
                debug_assert_eq!(false, other_ip.is_documentation());
                debug_assert_eq!(false, other_ip.is_unspecified());
                if self_ip.is_loopback() {
                    return true
                }
                if other_ip.is_loopback() {
                    return self_ip.is_loopback()
                }
                if other_ip.is_private() {
                    if !self_ip.is_private() {
                        return false
                    }
                    return is_same_private_subnet(&self_ip, &other_ip)
                }
                true
            }
            _ => unimplemented!(),
        }
    }
}

fn is_same_private_subnet(ip1: &Ipv4Addr, ip2: &Ipv4Addr) -> bool {
    debug_assert_eq!(true, ip1.is_private());
    debug_assert_eq!(true, ip2.is_private());

    match ((ip1.octets()[0], ip1.octets()[1]), (ip2.octets()[0], ip2.octets()[1])) {
        ((10, _), (10, _)) => true,
        ((172, b1), (172, b2)) if b1 >= 16 && b1 <= 31 && b2 >= 16 && b2 <= 31 => true,
        ((192, 168), (192, 168)) => true,
        _ => false,
    }
}

pub fn convert_to_node_id(ip: IpAddr, port: u16) -> NodeId {
    NodeId::new(ip, port)
}

impl Into<NodeId> for SocketAddr {
    fn into(self) -> NodeId {
        (&self).into()
    }
}

impl<'a> Into<NodeId> for &'a SocketAddr {
    fn into(self) -> NodeId {
        let ip = self.addr.ip();
        let port = self.addr.port();
        convert_to_node_id(ip, port)
    }
}

impl From<net::SocketAddr> for SocketAddr {
    fn from(addr: net::SocketAddr) -> Self {
        match addr {
            net::SocketAddr::V4(_) => Self {
                addr,
            },
            net::SocketAddr::V6(_) => unimplemented!(),
        }
    }
}

impl Into<net::SocketAddr> for SocketAddr {
    fn into(self) -> net::SocketAddr {
        self.addr
    }
}

impl<'a> Into<&'a net::SocketAddr> for &'a SocketAddr {
    fn into(self) -> &'a net::SocketAddr {
        &self.addr
    }
}

impl FromStr for SocketAddr {
    type Err = AddrParseError;
    fn from_str(addr: &str) -> Result<Self, Self::Err> {
        let addr = net::SocketAddrV4::from_str(addr)?;
        Ok(Self::from(net::SocketAddr::V4(addr)))
    }
}

impl Ord for SocketAddr {
    fn cmp(&self, other: &SocketAddr) -> Ordering {
        match (self.addr, other.addr) {
            (net::SocketAddr::V4(_), net::SocketAddr::V6(_)) => Ordering::Less,
            (net::SocketAddr::V6(_), net::SocketAddr::V4(_)) => Ordering::Greater,
            (lhs, rhs) => match lhs.ip().cmp(&rhs.ip()) {
                Ordering::Equal => lhs.port().cmp(&rhs.port()),
                order => order,
            },
        }
    }
}

impl PartialOrd for SocketAddr {
    fn partial_cmp(&self, other: &SocketAddr) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.addr.fmt(f)
    }
}

impl Encodable for SocketAddr {
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
            }
            IpAddr::V6(ref _addr) => unimplemented!(),
        }
    }
}

impl Decodable for SocketAddr {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.item_count()? {
            5 => {
                let ip0 = rlp.val_at(0)?;
                let ip1 = rlp.val_at(1)?;
                let ip2 = rlp.val_at(2)?;
                let ip3 = rlp.val_at(3)?;
                let port = rlp.val_at(4)?;
                Ok(SocketAddr::v4(ip0, ip1, ip2, ip3, port))
            }
            17 => unimplemented!(),
            _ => Err(DecoderError::RlpIncorrectListLen),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresss_are_equal_if_they_have_same_id_and_port() {
        let a1 = SocketAddr::v4(127, 0, 0, 1, 3485);
        let a2 = SocketAddr::v4(127, 0, 0, 1, 3485);
        assert_eq!(a1, a2);
    }

    #[test]
    fn addresss_are_not_equal_if_their_ip_is_different() {
        let a1 = SocketAddr::v4(127, 0, 0, 1, 3485);
        let a2 = SocketAddr::v4(192, 168, 0, 1, 3485);
        assert_ne!(a1, a2);
    }

    #[test]
    fn addresss_are_not_equal_if_their_port_is_different() {
        let a1 = SocketAddr::v4(127, 0, 0, 1, 3485);
        let a2 = SocketAddr::v4(127, 0, 0, 1, 3486);
        assert_ne!(a1, a2);
    }

    #[test]
    fn address_is_less_than_if_port_is_less() {
        let a1 = SocketAddr::v4(127, 0, 0, 1, 3485);
        let a2 = SocketAddr::v4(127, 0, 0, 1, 3486);
        assert_eq!(Ordering::Less, a1.cmp(&a2));
    }

    #[test]
    fn address_is_greater_than_if_port_is_greater() {
        let a1 = SocketAddr::v4(127, 0, 0, 1, 3485);
        let a2 = SocketAddr::v4(127, 0, 0, 1, 3484);
        assert_eq!(Ordering::Greater, a1.cmp(&a2));
    }

    #[test]
    fn is_global_for_ipv4() {
        let a1 = SocketAddr::v4(127, 0, 0, 1, 3485);
        let a2 = SocketAddr::v4(192, 168, 0, 1, 3485);
        let a3 = SocketAddr::v4(1, 1, 1, 1, 3485);
        assert_eq!(false, a1.is_global());
        assert_eq!(false, a2.is_global());
        assert_eq!(true, a3.is_global());
    }

    #[test]
    fn is_reachable() {
        // Servers which have loopback addresses can connect to each other
        let loopback_1 = SocketAddr::v4(127, 0, 0, 1, 3485);
        let loopback_2 = SocketAddr::v4(127, 0, 0, 1, 3486);
        assert_eq!(true, loopback_1.is_reachable(&loopback_2));
        assert_eq!(true, loopback_2.is_reachable(&loopback_1));
        assert_eq!(false, loopback_1.is_reachable(&loopback_1));

        // Servers from same private subnet addresses can connect to each other
        let private_1 = SocketAddr::v4(192, 168, 0, 1, 3485);
        let private_2 = SocketAddr::v4(192, 168, 0, 2, 3485);
        assert_eq!(true, private_1.is_reachable(&private_2));
        assert_eq!(true, private_2.is_reachable(&private_1));
        assert_eq!(false, private_1.is_reachable(&private_1));

        // Servers from same private subnet addresses can connect to each other
        let private_3 = SocketAddr::v4(172, 16, 0, 1, 3485);
        let private_4 = SocketAddr::v4(172, 17, 0, 1, 3485);
        assert_eq!(true, private_3.is_reachable(&private_4));
        assert_eq!(true, private_4.is_reachable(&private_3));

        // Servers from different private subnet addresses can not connect to each other
        assert_eq!(false, private_1.is_reachable(&private_3));
        assert_eq!(false, private_3.is_reachable(&private_1));

        // Servers which have public addresses can connect to each other
        let public_1 = SocketAddr::v4(1, 1, 1, 1, 3485);
        let public_2 = SocketAddr::v4(2, 2, 2, 2, 3485);
        assert_eq!(true, public_1.is_reachable(&public_2));
        assert_eq!(true, public_2.is_reachable(&public_1));
        assert_eq!(false, public_1.is_reachable(&public_1));

        // Any server can connect to public address
        assert_eq!(true, private_1.is_reachable(&public_1));
        assert_eq!(true, loopback_1.is_reachable(&public_1));

        // Servers from public network can not connect to private server
        assert_eq!(false, public_1.is_reachable(&private_1));

        // Servers from loopback network can connect to private server
        assert_eq!(true, loopback_1.is_reachable(&private_1));

        // Only servers from loopback network can connect to loopback server
        assert_eq!(false, public_1.is_reachable(&loopback_1));
        assert_eq!(false, private_1.is_reachable(&loopback_1));
    }

    #[test]
    fn encode_and_decode_nonce() {
        rlp_encode_and_decode_test!(SocketAddr::v4(1, 2, 3, 4, 5678));
    }
}
