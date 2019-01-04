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

use std::cmp::PartialEq;
use std::fmt;
use std::ops::Sub;

use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::message::VoteStep;

pub type Height = usize;
pub type View = usize;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Step {
    Propose,
    Prevote,
    Precommit,
    Commit,
}

impl Step {
    pub fn is_pre(self) -> bool {
        match self {
            Step::Prevote | Step::Precommit => true,
            _ => false,
        }
    }

    pub fn number(self) -> u8 {
        match self {
            Step::Propose => 0,
            Step::Prevote => 1,
            Step::Precommit => 2,
            Step::Commit => 3,
        }
    }
}

impl Decodable for Step {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.as_val()? {
            0u8 => Ok(Step::Propose),
            1 => Ok(Step::Prevote),
            2 => Ok(Step::Precommit),
            // FIXME: Step::Commit case is not necessary if Api::send_local_message does not serialize message.
            3 => Ok(Step::Commit),
            _ => Err(DecoderError::Custom("Invalid step.")),
        }
    }
}

impl Encodable for Step {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_single_value(&self.number());
    }
}

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
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        rlp.decoder().decode_value(|bytes| {
            if bytes.len() > BITSET_SIZE {
                Err(DecoderError::RlpIsTooBig)
            } else if bytes.len() < BITSET_SIZE {
                Err(DecoderError::RlpIsTooShort)
            } else {
                let mut bit_set = BitSet::new();
                bit_set.0.copy_from_slice(bytes);
                Ok(bit_set)
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

pub struct PeerState {
    pub vote_step: VoteStep,
    pub proposal: Option<H256>,
    pub messages: BitSet,
}

impl PeerState {
    pub fn new() -> Self {
        PeerState {
            vote_step: VoteStep::new(0, 0, Step::Propose),
            proposal: None,
            messages: BitSet::new(),
        }
    }
}
