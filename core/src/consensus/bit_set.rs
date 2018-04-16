// Copyright 2019 Kodebox, Inc.
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

use std::cmp::{Ordering, PartialEq};
use std::convert::TryFrom;
use std::fmt;
use std::ops::Sub;

use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

const MAX_VALIDATOR_SIZE: usize = 800;
const BITSET_SIZE: usize = MAX_VALIDATOR_SIZE / 8;

#[derive(Copy, Clone)]
pub struct BitSet([u8; BITSET_SIZE]);

impl BitSet {
    pub fn new() -> Self {
        BitSet([0; BITSET_SIZE])
    }

    pub fn new_with_indices(indices: &[usize]) -> Self {
        let mut bitset = BitSet::new();
        for index in indices {
            bitset.set(*index);
        }
        bitset
    }

    pub fn all_set() -> Self {
        let mut bit_set = BitSet::new();
        for i in 0..bit_set.0.len() {
            bit_set.0[i] = u8::max_value()
        }
        bit_set
    }

    pub fn is_empty(&self) -> bool {
        // An array whose size is bigger than 32 does not have PartialEq trait
        // So coerce it to slice to compare
        let own: &[u8] = &self.0;
        let empty: &[u8] = &[0; BITSET_SIZE];
        own == empty
    }

    pub fn is_set(&self, index: usize) -> bool {
        let array_index = index / 8;
        let bit_index = index % 8;

        self.0[array_index] & (1 << bit_index) != 0
    }

    pub fn set(&mut self, index: usize) {
        let array_index = index / 8;
        let bit_index = index % 8;

        self.0[array_index] |= 1u8 << bit_index;
    }

    pub fn reset(&mut self, index: usize) {
        let array_index = index / 8;
        let bit_index = index % 8;

        self.0[array_index] &= 0b1111_1111 ^ (1 << bit_index);
    }

    pub fn count(&self) -> usize {
        self.0
            .iter()
            .map(|v| usize::try_from(v.count_ones()).expect("CodeChain doesn't support 16-bits architecture"))
            .sum()
    }

    pub fn true_index_iter(&self) -> BitSetIndexIterator {
        BitSetIndexIterator {
            index: 0,
            bitset: self,
        }
    }
}

impl Default for BitSet {
    fn default() -> Self {
        BitSet::new()
    }
}

impl fmt::Debug for BitSet {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.0[..].fmt(formatter)
    }
}

impl PartialEq for BitSet {
    fn eq(&self, other: &Self) -> bool {
        // An array whose size is bigger than 32 does not have PartialEq trait
        // So coerce it to slice to compare
        let lhs: &[u8] = &self.0;
        let rhs: &[u8] = &other.0;
        lhs == rhs
    }
}

impl Encodable for BitSet {
    fn rlp_append(&self, s: &mut RlpStream) {
        let slice: &[u8] = &self.0;
        s.append(&slice);
    }
}

impl Decodable for BitSet {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        rlp.decoder().decode_value(|bytes| {
            let expected = BITSET_SIZE;
            let got = bytes.len();
            match got.cmp(&expected) {
                Ordering::Greater => Err(DecoderError::RlpIsTooBig {
                    expected,
                    got,
                }),
                Ordering::Less => Err(DecoderError::RlpIsTooShort {
                    expected,
                    got,
                }),
                Ordering::Equal => {
                    let mut bit_set = BitSet::new();
                    bit_set.0.copy_from_slice(bytes);
                    Ok(bit_set)
                }
            }
        })
    }
}

impl<'a> Sub for &'a BitSet {
    type Output = BitSet;

    fn sub(self, rhs: &'a BitSet) -> <Self as Sub<&BitSet>>::Output {
        let mut bit_set = BitSet::new();
        for i in 0..bit_set.0.len() {
            bit_set.0[i] = self.0[i] & (!rhs.0[i]);
        }
        bit_set
    }
}

pub struct BitSetIndexIterator<'a> {
    index: usize,
    bitset: &'a BitSet,
}

impl<'a> Iterator for BitSetIndexIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= MAX_VALIDATOR_SIZE {
            return None
        }

        while !self.bitset.is_set(self.index) {
            self.index += 1;

            if self.index >= MAX_VALIDATOR_SIZE {
                return None
            }
        }

        let result = Some(self.index);
        self.index += 1;
        result
    }
}
