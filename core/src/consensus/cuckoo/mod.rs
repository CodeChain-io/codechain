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

mod params;

use std::cmp::{max, min};

use byteorder::{ByteOrder, LittleEndian};
use ccrypto::blake256;
use ctypes::machine::WithBalances;
use ctypes::util::unexpected::{Mismatch, OutOfBounds};
use cuckoo::Cuckoo as CuckooVerifier;
use primitives::U256;
use rlp::UntrustedRlp;

use self::params::CuckooParams;
use super::ConsensusEngine;
use crate::block::{ExecutedBlock, IsBlock};
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::EngineType;
use crate::error::{BlockError, Error};
use crate::header::Header;

/// Cuckoo specific seal
#[derive(Debug, PartialEq)]
pub struct Seal {
    pub nonce: u64,
    pub proof: Vec<u32>,
}

impl Seal {
    /// Tries to parse rlp as cuckoo seal.
    pub fn parse_seal<T: AsRef<[u8]>>(seal: &[T]) -> Result<Self, Error> {
        if seal.len() != 2 {
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: 2,
                found: seal.len(),
            })
            .into())
        }

        Ok(Seal {
            nonce: UntrustedRlp::new(seal[0].as_ref()).as_val()?,
            proof: UntrustedRlp::new(seal[1].as_ref()).as_list()?,
        })
    }
}

pub struct Cuckoo {
    params: CuckooParams,
    machine: CodeChainMachine,
    verifier: CuckooVerifier,
}

impl Cuckoo {
    pub fn new(params: CuckooParams, machine: CodeChainMachine) -> Self {
        let verifier = CuckooVerifier::new(params.max_vertex, params.max_edge, params.cycle_length);
        Self {
            params,
            machine,
            verifier,
        }
    }

    fn calculate_score(&self, header: &Header, parent: &Header) -> U256 {
        if header.number() == 0 {
            panic!("Can't calculate genesis block score");
        }

        //score = parent_score + parent_score // 2048 * max(1 - (block_timestamp - parent_timestamp) // block_interval, -99)
        let diff = (header.timestamp() - parent.timestamp()) / self.params.block_interval;
        let target = if diff <= 1 {
            parent.score().saturating_add(*parent.score() / 2048.into() * U256::from(1 - diff))
        } else {
            parent.score().saturating_sub(*parent.score() / 2048.into() * U256::from(min(diff - 1, 99)))
        };
        max(self.params.min_score, target)
    }
}

impl ConsensusEngine<CodeChainMachine> for Cuckoo {
    fn name(&self) -> &str {
        "Cuckoo"
    }

    fn machine(&self) -> &CodeChainMachine {
        &self.machine
    }

    fn seal_fields(&self, _header: &Header) -> usize {
        2
    }

    fn engine_type(&self) -> EngineType {
        EngineType::PoW
    }

    fn verify_local_seal(&self, header: &Header) -> Result<(), Error> {
        self.verify_block_basic(header).and_then(|_| self.verify_block_unordered(header))
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        if *header.score() < self.params.min_score {
            return Err(From::from(BlockError::ScoreOutOfBounds(OutOfBounds {
                min: Some(self.params.min_score),
                max: None,
                found: *header.score(),
            })))
        }

        Ok(())
    }

    fn verify_block_unordered(&self, header: &Header) -> Result<(), Error> {
        let seal = Seal::parse_seal(header.seal())?;

        let mut message = header.bare_hash().0;
        LittleEndian::write_u64(&mut message, seal.nonce);

        if !self.verifier.verify(&message, &seal.proof) {
            return Err(From::from(BlockError::InvalidProofOfWork))
        }

        let target = self.score_to_target(header.score());
        let hash = blake256(::rlp::encode_list(&seal.proof));
        if U256::from(hash) > target {
            return Err(From::from(BlockError::PowOutOfBounds(OutOfBounds {
                min: None,
                max: Some(target),
                found: U256::from(hash),
            })))
        }
        Ok(())
    }

    fn verify_block_family(&self, header: &Header, parent: &Header) -> Result<(), Error> {
        if header.number() == 0 {
            return Err(From::from(BlockError::RidiculousNumber(OutOfBounds {
                min: Some(1),
                max: None,
                found: header.number(),
            })))
        }

        let expected_score = self.calculate_score(header, parent);
        if header.score() != &expected_score {
            return Err(From::from(BlockError::InvalidScore(Mismatch {
                expected: expected_score,
                found: U256::from(header.hash()),
            })))
        }

        Ok(())
    }

    fn populate_from_parent(&self, header: &mut Header, parent: &Header) {
        let score = self.calculate_score(header, parent);
        header.set_score(score);
    }

    fn on_close_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        let author = *block.header().author();
        let total_reward = block.parcels().iter().fold(self.params.block_reward, |sum, parcel| sum + parcel.fee);
        self.machine.add_balance(block, &author, total_reward)
    }

    fn score_to_target(&self, score: &U256) -> U256 {
        (U256::max_value() - *score) / *score
    }
}

#[cfg(test)]
mod tests {
    use crate::block::OpenBlock;
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    use super::*;

    #[test]
    fn has_valid_metadata() {
        let engine = Scheme::new_test_cuckoo().engine;

        assert_eq!(engine.name(), "Cuckoo");
        assert_eq!(engine.engine_type(), EngineType::PoW);
    }

    #[test]
    fn seal_fields() {
        let engine = Scheme::new_test_cuckoo().engine;
        let header = Header::default();

        assert_eq!(engine.seal_fields(&header), 2);
    }

    #[test]
    fn verify_block_basic_err() {
        let engine = Scheme::new_test_cuckoo().engine;
        let default_header = Header::default();

        assert!(engine.verify_block_basic(&default_header).is_err());
    }

    #[test]
    fn verify_block_basic_ok() {
        let scheme = Scheme::new_test_cuckoo();
        let engine = &*scheme.engine;
        let genesis_header = scheme.genesis_header();

        assert!(engine.verify_block_basic(&genesis_header).is_ok());
    }

    #[test]
    fn verify_block_unordered_err() {
        let engine = Scheme::new_test_cuckoo().engine;
        let default_header = Header::default();

        assert!(engine.verify_block_unordered(&default_header).is_err());
    }

    #[test]
    fn score_to_target() {
        let engine = Scheme::new_test_cuckoo().engine;

        assert_eq!(engine.score_to_target(&U256::max_value()), U256::from(0));
    }

    #[test]
    fn on_close_block() {
        let scheme = Scheme::new_test_cuckoo();
        let engine = &*scheme.engine;
        let db = scheme.ensure_genesis_state(get_temp_state_db()).unwrap();
        let header = Header::default();
        let block = OpenBlock::new(engine, db, &header, Default::default(), vec![], false).unwrap();
        let mut executed_block = block.block().clone();

        assert!(engine.on_close_block(&mut executed_block).is_ok());
        assert_eq!(0xd, engine.machine().balance(&executed_block, header.author()).unwrap());
    }

    #[test]
    fn populate_from_parent() {
        let scheme = Scheme::new_test_cuckoo();
        let engine = &*scheme.engine;
        let mut header = Header::default();
        let genesis_header = scheme.genesis_header();
        header.set_number(1);
        header.set_parent_hash(genesis_header.hash());

        engine.populate_from_parent(&mut header, &genesis_header);
        assert_eq!(*header.score(), U256::from(0x20040));
    }
}
