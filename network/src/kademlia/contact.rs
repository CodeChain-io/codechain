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

#[cfg(test)]
use std::str::FromStr;


use super::NodeId;
use super::node_id::{log2_distance_between_nodes, self};
use super::super::SocketAddr;


#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Contact {
    id: NodeId,
    addr: SocketAddr,
}

#[cfg(test)]
fn zero() -> SocketAddr {
    SocketAddr::v4(0, 0, 0, 0, 0)
}

impl Contact {
    pub fn random(addr: SocketAddr) -> Self {
        let id = node_id::random();
        Contact {
            id,
            addr,
        }
    }

    pub fn new(id: NodeId, addr: SocketAddr) -> Self {
        Contact {
            id,
            addr,
        }
    }

    #[cfg(test)]
    pub fn from_hash_with_addr(node_id: &str, a: u8, b: u8, c: u8, d: u8, port: u16) -> Contact {
        Contact::new(NodeId::from_str(node_id).unwrap(), SocketAddr::v4(a, b, c, d, port))
    }

    #[cfg(test)]
    pub fn from_hash(hash: &str) -> Self {
        Contact {
            id: NodeId::from_str(hash).unwrap(),
            addr: zero(),
        }
    }

    pub fn log2_distance(&self, target: &NodeId) -> usize {
        log2_distance_between_nodes(&self.id, target)
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }
}


#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::mem::size_of;
    use std::str::FromStr;
    use super::Contact;

    #[test]
    fn test_log2_distance_is_1_if_lsb_is_different() {
        let c1 = Contact::from_hash("0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000");
        let c2 = Contact::from_hash("0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000001");

        assert_eq!(1, c1.log2_distance(&c2.id));
    }

    #[test]
    fn test_log2_distance_is_node_id_size_if_msb_is_different() {
        let c1 = Contact::from_hash("0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000");
        let c2 = Contact::from_hash("8000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000");

        assert_eq!(super::super::B, c1.log2_distance(&c2.id));
    }

    #[test]
    fn test_size_of_address_is_b() {
        assert_eq!(super::super::B, size_of::<super::NodeId>() * 8);
    }

    #[test]
    fn test_contacts_are_not_equal_if_they_have_different_id() {
        const ID1: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
        const ID2: &str = "8700000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

        let c1 = Contact::from_hash(ID1);
        let c2 = Contact::from_hash(ID2);
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_contacts_are_equal_if_they_have_same_id_and_addresses_are_none() {
        const ID1: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
        const ID2: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

        let c1 = Contact::from_hash(ID1);
        let c2 = Contact::from_hash(ID2);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_contacts_are_equal_if_they_have_same_id_and_address() {
        const ID1: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
        const ID2: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

        let c1 = Contact::from_hash_with_addr(ID1, 127, 0, 0, 1, 3485);
        let c2 = Contact::from_hash_with_addr(ID2, 127, 0, 0, 1, 3485);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_contacts_are_not_equal_if_their_addresses_are_different() {
        const ID1: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
        const ID2: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

        let c1 = Contact::from_hash_with_addr(ID1, 127, 0, 0, 1, 3485);
        let c2 = Contact::from_hash_with_addr(ID2, 127, 0, 0, 1, 3486);
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_contact_greater_than_if_id_is_greater() {
        const ID1: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
        const ID2: &str = "7000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

        let c1 = Contact::from_hash_with_addr(ID1, 127, 0, 0, 1, 3485);
        let c2 = Contact::from_hash_with_addr(ID2, 127, 0, 0, 1, 3486);
        assert!(c1 > c2);
    }

    #[test]
    fn test_contact_less_than_if_id_is_less() {
        const ID1: &str = "7000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
        const ID2: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

        let c1 = Contact::from_hash_with_addr(ID1, 127, 0, 0, 1, 3485);
        let c2 = Contact::from_hash_with_addr(ID2, 127, 0, 0, 1, 3486);
        assert!(c1 < c2);
    }

    #[test]
    fn test_contacts_is_less_than_if_port_is_less() {
        const ID1: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
        const ID2: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

        let c1 = Contact::from_hash_with_addr(ID1, 127, 0, 0, 1, 3485);
        let c2 = Contact::from_hash_with_addr(ID2, 127, 0, 0, 1, 3486);
        assert_eq!(Some(Ordering::Less), c1.partial_cmp(&c2));
    }

    #[test]
    fn test_contacts_is_greater_than_if_port_is_greater() {
        const ID1: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";
        const ID2: &str = "8000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000\
            0000000000000000";

        let c1 = Contact::from_hash_with_addr(ID1, 127, 0, 0, 1, 3485);
        let c2 = Contact::from_hash_with_addr(ID2, 127, 0, 0, 1, 3484);
        assert_eq!(Some(Ordering::Greater), c1.partial_cmp(&c2));
    }
}
