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

extern crate codechain_crypto as crypto;
extern crate rand;


#[cfg(test)]
use std::str::FromStr;
use super::NodeId;
use super::super::Address;


#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Contact {
    id: NodeId,
    addr: Option<Address>,
}

fn hash<T: AsRef<[u8]>>(block: T) -> NodeId {
    crypto::blake512(block.as_ref())
}

impl Contact {
    pub fn random() -> Self {
        const RAND_BLOCK_SIZE: usize = 16;
        let mut rand_block: [u8; RAND_BLOCK_SIZE] = [0; RAND_BLOCK_SIZE];
        for iter in rand_block.iter_mut() {
            *iter = rand::random::<u8>();
        }
        Contact {
            id: hash(rand_block),
            addr: None,
        }
    }

    pub fn new(id: NodeId, addr: Option<Address>) -> Self {
        Contact {
            id,
            addr,
        }
    }

    #[cfg(test)]
    pub fn from_hash_with_addr(node_id: &str, a: u8, b: u8, c: u8, d: u8, port: u16) -> Contact {
        Contact::new(NodeId::from_str(node_id).unwrap(), Some(Address::v4(a, b, c, d, port)))
    }

    #[cfg(test)]
    pub fn from_hash(hash: &str) -> Self {
        Contact {
            id: NodeId::from_str(hash).unwrap(),
            addr: None,
        }
    }

    pub fn log2_distance(&self, target: &NodeId) -> usize {
        let distance = &self.id ^ target;
        const BYTES_SIZE: usize = super::B / 8;
        debug_assert_eq!(super::B % 8, 0);
        let mut distance_as_bytes : [u8; BYTES_SIZE] = [0; BYTES_SIZE];
        distance.copy_to(&mut distance_as_bytes);

        let mut same_prefix_length: usize = 0;
        const MASKS: [u8; 8] = [0b1000_0000, 0b0100_0000, 0b0010_0000, 0b0001_0000, 0b0000_1000, 0b0000_0100, 0b0000_0010, 0b0000_0001];
        'outer: for byte in distance_as_bytes.iter() {
            for mask in MASKS.iter() {
                if byte & mask != 0 {
                    break 'outer;
                }
                same_prefix_length += 1
            }
        }

        return super::B - same_prefix_length;
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn addr(&self) -> &Option<Address> {
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
