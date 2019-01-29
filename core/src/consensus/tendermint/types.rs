// Copyright 2018-2019 Kodebox, Inc.
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

use ckey::SchnorrSignature;
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::message::VoteStep;
use crate::block::{IsBlock, SealedBlock};
use crate::error::Error;

pub type Height = usize;
pub type View = usize;

pub enum TendermintState {
    Propose,
    ProposeWaitBlockGeneration {
        parent_hash: H256,
    },
    ProposeWaitImported {
        block: Box<SealedBlock>,
    },
    ProposeWaitEmptyBlockTimer {
        block: Box<SealedBlock>,
    },
    Prevote,
    Precommit,
    Commit,
}

impl TendermintState {
    pub fn to_step(&self) -> Step {
        match self {
            TendermintState::Propose => Step::Propose,
            TendermintState::ProposeWaitBlockGeneration {
                ..
            } => Step::Propose,
            TendermintState::ProposeWaitImported {
                ..
            } => Step::Propose,
            TendermintState::ProposeWaitEmptyBlockTimer {
                ..
            } => Step::Propose,
            TendermintState::Prevote => Step::Prevote,
            TendermintState::Precommit => Step::Precommit,
            TendermintState::Commit => Step::Commit,
        }
    }

    pub fn is_commit(&self) -> bool {
        match self {
            TendermintState::Commit => true,
            _ => false,
        }
    }
}

impl fmt::Debug for TendermintState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TendermintState::Propose => write!(f, "TendermintState::Propose"),
            TendermintState::ProposeWaitBlockGeneration {
                parent_hash,
            } => write!(f, "TendermintState::ProposeWaitBlockGeneration({})", parent_hash),
            TendermintState::ProposeWaitImported {
                block,
            } => write!(f, "TendermintState::ProposeWaitImported({})", block.header().hash()),
            TendermintState::ProposeWaitEmptyBlockTimer {
                block,
            } => write!(f, "TendermintState::ProposeWaitEmptyBlockTimer({})", block.header().hash()),
            TendermintState::Prevote => write!(f, "TendermintState::Prevote"),
            TendermintState::Precommit => write!(f, "TendermintState::Precommit"),
            TendermintState::Commit => write!(f, "TendermintState::Commit"),
        }
    }
}

impl From<Step> for TendermintState {
    fn from(s: Step) -> Self {
        match s {
            Step::Propose => TendermintState::Propose,
            Step::Prevote => TendermintState::Prevote,
            Step::Precommit => TendermintState::Precommit,
            Step::Commit => TendermintState::Commit,
        }
    }
}

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

    pub fn count(&self) -> u32 {
        self.0.iter().cloned().map(u8::count_ones).sum()
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

pub struct TendermintSealView<'a> {
    seal: &'a [Bytes],
}

impl<'a> TendermintSealView<'a> {
    pub fn new(bytes: &'a [Bytes]) -> TendermintSealView<'a> {
        TendermintSealView {
            seal: bytes,
        }
    }

    pub fn bitset(&self) -> Result<BitSet, Error> {
        Ok(UntrustedRlp::new(
            &self.seal.get(3).expect("block went through verify_block_basic; block has .seal_fields() fields; qed"),
        )
        .as_val()?)
    }

    pub fn precommits(&self) -> UntrustedRlp<'a> {
        UntrustedRlp::new(
            &self.seal.get(2).expect("block went through verify_block_basic; block has .seal_fields() fields; qed"),
        )
    }

    pub fn signatures(&self) -> Result<Vec<(usize, SchnorrSignature)>, Error> {
        let precommits = self.precommits();
        let bitset = self.bitset()?;

        let bitset_iter = bitset.true_index_iter();
        let signatures: Vec<SchnorrSignature> =
            precommits.iter().map(|rlp| rlp.as_val::<SchnorrSignature>()).collect::<Result<_, _>>()?;
        Ok(bitset_iter.zip(signatures).collect())
    }
}
