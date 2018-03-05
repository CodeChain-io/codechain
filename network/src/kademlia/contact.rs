extern crate keccak_hash;

use codechain_types::H256;
use std::net::{ Ipv4Addr, Ipv6Addr, IpAddr, SocketAddr };

pub type NodeId = H256;

pub struct Contact {
	id: NodeId,
	addr: Option<SocketAddr>,
}

impl Contact {
	pub fn localhost(port: u16) -> Self {
        let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        Contact {
            id: Contact::hash(localhost, port),
            addr: Some(SocketAddr::new(localhost, port)),
        }
    }

	pub fn new(ip: IpAddr, port: u16) -> Self {
        let ip = match ip {
            IpAddr::V4(ip) => IpAddr::V4(ip),
            IpAddr::V6(ip) =>
                let localhost_v6: Ipv6Addr = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
                if ip == localhost_v6 {
                    let localhost_v4: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
                    IpAddr::V4(localhost_v4)
                } else {
                    IpAddr::V6(ip)
                }
        };
		Contact {
			id: Contact::hash(ip, port),
			addr: Some(SocketAddr::new(ip, port)),
		}
	}

	fn from_hash(id: NodeId) -> Self {
		Contact {
			id,
			addr: None,
		}
	}

	fn hash(ip: IpAddr, port: u16) -> NodeId {
		let mut block: [u8; 18] = [0; 18];
		match ip {
			IpAddr::V4(ip) => block[..16].clone_from_slice(&ip.to_ipv6_mapped().octets()),
			IpAddr::V6(ip) => block[..16].clone_from_slice(&ip.octets()),
		}
		block[16] = ((port >>8) & 0xff) as u8;
		block[17] = (port & 0xff) as u8;
		keccak_hash::keccak(block) // FIXME: Use blake2
	}

	pub fn log2_distance(&self, target: &Self) -> usize {
		let distance = self.id ^ target.id;
		let mut distance_as_bytes : [u8; 32] = [0; 32];
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

        return 256 - same_prefix_length;
	}
}

#[cfg(test)]
mod tests {
	use super::Contact;

	use codechain_types::H256;
	use std::net::{ IpAddr, Ipv4Addr, Ipv6Addr };
    use std::str::FromStr;

	#[test]
	fn test_log2_distance_is_0_if_two_host_are_the_same() {
		let c1 = Contact::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8000);
		let c2 = Contact::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8000);

		assert_eq!(0, c1.log2_distance(&c2));
	}

	#[test]
	fn test_log2_distance_of_localhost_v4_and_v6_is_0() {
		let c1 = Contact::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8000);
		let c2 = Contact::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 8000);

		assert_eq!(0, c1.log2_distance(&c2));
	}

	#[test]
	fn test_log2_distance_is_1_if_lsb_is_different() {
		let c1 = Contact::from_hash(H256::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap());
		let c2 = Contact::from_hash(H256::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap());

		assert_eq!(1, c1.log2_distance(&c2));
	}

	#[test]
	fn test_log2_distance_is_256_if_msb_is_different() {
		let c1 = Contact::from_hash(H256::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap());
		let c2 = Contact::from_hash(H256::from_str("8000000000000000000000000000000000000000000000000000000000000000").unwrap());

		assert_eq!(256, c1.log2_distance(&c2));
	}
}
