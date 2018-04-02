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

use ccrypto::blake512;
use ctypes::Public;
use rand;

use super::B;

pub type NodeId = Public;

fn hash<T: AsRef<[u8]>>(block: T) -> NodeId {
    blake512(block.as_ref())
}

pub fn random() -> NodeId {
    const RAND_BLOCK_SIZE: usize = 16;
    let mut rand_block: [u8; RAND_BLOCK_SIZE] = [0; RAND_BLOCK_SIZE];
    for iter in rand_block.iter_mut() {
        *iter = rand::random::<u8>();
    }
    hash(rand_block)
}

pub fn log2_distance_between_nodes(lhs: &NodeId, rhs: &NodeId) -> usize {
    let distance = lhs ^ rhs;
    const BYTES_SIZE: usize = B / 8;
    debug_assert_eq!(B % 8, 0);
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

    return B - same_prefix_length;
}
