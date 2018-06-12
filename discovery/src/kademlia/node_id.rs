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

use ccrypto::Blake;

use cnetwork::IntoSocketAddr;
use cnetwork::NodeId as NetworkNodeId;
use ctypes::H256;

use super::B;

pub type NodeId = NetworkNodeId;

fn node_id_to_hash(lhs: &NodeId) -> H256 {
    let addr = lhs.into_addr();
    let ip = addr.ip();
    let port = addr.port();
    match ip {
        IpAddr::V4(ip) => {
            if ip.is_loopback() || ip.is_private() {
                let mut octets = [0u8; 18];
                octets[0..16].clone_from_slice(&ip.to_ipv6_compatible().octets());
                octets[16] = (port >> 8) as u8;
                octets[17] = (port & 0xFF) as u8;
                return Blake::blake(&octets)
            }
            let octets: [u8; 16] = ip.to_ipv6_compatible().octets();
            let mut hash = H256::blake(&octets);
            hash[14] ^= (port >> 8) as u8;
            hash[15] ^= (port & 0xFF) as u8;
            hash
        }
        IpAddr::V6(ip) => {
            if ip.is_loopback() {
                let mut octets = [0u8; 18];
                octets.clone_from_slice(&ip.octets());
                octets[16] = (port >> 8) as u8;
                octets[17] = (port & 0xFF) as u8;
                return Blake::blake(&octets)
            }
            let octets: [u8; 16] = ip.octets();
            let mut hash = H256::blake(&octets);
            hash[14] ^= (port >> 8) as u8;
            hash[15] ^= (port & 0xFF) as u8;
            hash
        }
    }
}

pub fn log2_distance_between_nodes(lhs: &NodeId, rhs: &NodeId) -> usize {
    let lhs = node_id_to_hash(lhs);
    let rhs = node_id_to_hash(rhs);

    let distance = lhs ^ rhs;
    const BYTES_SIZE: usize = B / 8;
    debug_assert_eq!(B % 8, 0);
    let mut distance_as_bytes: [u8; BYTES_SIZE] = [0; BYTES_SIZE];
    distance.copy_to(&mut distance_as_bytes);

    let mut same_prefix_length: usize = 0;
    const MASKS: [u8; 8] =
        [0b1000_0000, 0b0100_0000, 0b0010_0000, 0b0001_0000, 0b0000_1000, 0b0000_0100, 0b0000_0010, 0b0000_0001];
    'outer: for byte in distance_as_bytes.iter() {
        for mask in MASKS.iter() {
            if byte & mask != 0 {
                break 'outer
            }
            same_prefix_length += 1
        }
    }

    return B - same_prefix_length
}
