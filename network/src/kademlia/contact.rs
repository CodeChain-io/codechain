extern crate codechain_crypto as crypto;
extern crate rand;


use codechain_types::Public;
use std::cmp::Ordering;
use std::net::{ IpAddr, SocketAddr };
#[cfg(test)]
use std::str::FromStr;

pub type NodeId = Public;

#[derive(Clone, Debug, Eq)]
pub struct Contact {
    id: NodeId,
    addr: Option<SocketAddr>,
}

fn hash<T: AsRef<[u8]>>(block: T) -> Public {
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

    pub fn new(id: NodeId, addr: Option<SocketAddr>) -> Self {
        Contact {
            id,
            addr,
        }
    }

    #[cfg(test)]
    pub fn from_hash_with_addr(node_id: &str, ip: IpAddr, port: u16) -> Contact {
        Contact::new(NodeId::from_str(node_id).unwrap(), Some(SocketAddr::new(ip, port)))
    }

    #[cfg(test)]
    pub fn from_hash(hash: &str) -> Self {
        Contact {
            id: NodeId::from_str(hash).unwrap(),
            addr: None,
        }
    }

    pub fn log2_distance(&self, target: &Self) -> usize {
        let distance = self.id ^ target.id;
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
}

impl Ord for Contact {
    fn cmp(&self, other: &Contact) -> Ordering {
        if self.id < other.id {
            return Ordering::Less
        }
        if self.id > other.id {
            return Ordering::Greater
        }

        debug_assert_eq!(self.id, other.id);

        match (self.addr, other.addr) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(SocketAddr::V4(_)), Some(SocketAddr::V6(_))) => Ordering::Less,
            (Some(SocketAddr::V6(_)), Some(SocketAddr::V4(_))) => Ordering::Greater,
            (Some(lhs), Some(rhs)) => {
                match lhs.ip().cmp(&rhs.ip()) {
                    Ordering::Equal => lhs.port().cmp(&rhs.port()),
                    order => order,
                }
            },
        }
    }
}

impl PartialEq for Contact {
    fn eq(&self, other: &Contact ) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for Contact {
    fn partial_cmp(&self, other: &Contact) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


#[cfg(test)]
mod tests {
    use codechain_types::Public;
    use std::cmp::Ordering;
    use std::mem::size_of;
    use std::net::{ IpAddr, Ipv4Addr };
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

        assert_eq!(1, c1.log2_distance(&c2));
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

        assert_eq!(super::super::B, c1.log2_distance(&c2));
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

        let c1 = Contact::from_hash_with_addr(ID1, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3485);
        let c2 = Contact::from_hash_with_addr(ID2, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3485);
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

        let c1 = Contact::from_hash_with_addr(ID1, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3485);
        let c2 = Contact::from_hash_with_addr(ID2, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3486);
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

        let c1 = Contact::from_hash_with_addr(ID1, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3485);
        let c2 = Contact::from_hash_with_addr(ID2, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3486);
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

        let c1 = Contact::from_hash_with_addr(ID1, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3485);
        let c2 = Contact::from_hash_with_addr(ID2, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3486);
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

        let c1 = Contact::from_hash_with_addr(ID1, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3485);
        let c2 = Contact::from_hash_with_addr(ID2, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3486);
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

        let c1 = Contact::from_hash_with_addr(ID1, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3485);
        let c2 = Contact::from_hash_with_addr(ID2, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3484);
        assert_eq!(Some(Ordering::Greater), c1.partial_cmp(&c2));
    }
}
