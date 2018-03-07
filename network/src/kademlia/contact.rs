extern crate codechain_crypto as crypto;
extern crate rand;


use codechain_types::Public;
use std::net::{ IpAddr, SocketAddr };

pub type NodeId = Public;

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
}

#[cfg(test)]
mod tests {
    use super::Contact;
    use std::mem::size_of;

    use codechain_types::Public;
    use std::net::{ IpAddr, Ipv4Addr, Ipv6Addr };
    use std::str::FromStr;

    fn new_contact(node_id: &str) -> Contact {
        Contact::new(Public::from_str(node_id).unwrap(), None)
    }

    #[test]
    fn test_log2_distance_is_1_if_lsb_is_different() {
        let c1 = new_contact("0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000");
        let c2 = new_contact("0000000000000000\
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
        let c1 = new_contact("0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000\
                        0000000000000000");
        let c2 = new_contact("8000000000000000\
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
}
