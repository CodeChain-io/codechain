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

use engine::{ConsensusEngine, Seal};
use machine::{Machine, Transactions};

/// A consensus engine which does not provide any consensus mechanism.
pub struct Solo<M> {
    machine: M,
}

impl<M> Solo<M> {
    /// Returns new instance of Solo over the given state machine.
    pub fn new(machine: M) -> Self {
        Solo {
            machine,
        }
    }
}

impl<M: Machine> ConsensusEngine<M> for Solo<M>
    where M::LiveBlock: Transactions
{
    fn name(&self) -> &str {
        "Solo"
    }

    fn machine(&self) -> &M { &self.machine }

    fn seals_internally(&self) -> Option<bool> { Some(true) }

    fn generate_seal(&self, block: &M::LiveBlock, _parent: &M::Header) -> Seal {
        if block.transactions().is_empty() { Seal::None } else { Seal::Regular(Vec::new()) }
    }

    fn verify_local_seal(&self, _header: &M::Header) -> Result<(), M::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use codechain_types::{Address, H520};
    use codechain_machine::CodeChainMachine;

    use super::super::block::{OpenBlock, IsBlock};
    use super::super::header::Header;
    use super::super::engine::{Seal, ConsensusEngine};
    use super::Solo;

    fn genesis_header() -> Header {
        Header::default()
    }

    #[test]
    fn solo_can_seal() {
        let machine = CodeChainMachine::new();
        let engine = Solo::new(machine);
        let genesis_header = genesis_header();
        let b = OpenBlock::new(&engine, &genesis_header, Address::default()).unwrap();
        let b = b.close_and_lock();
        if let Seal::Regular(seal) = engine.generate_seal(b.block(), &genesis_header) {
            assert!(b.try_seal(&engine, seal).is_ok());
        }
    }

    #[test]
    fn solo_cant_verify() {
        let machine = CodeChainMachine::new();
        let engine = Solo::new(machine);
        let mut header: Header = Header::default();

        assert!(engine.verify_block_basic(&header).is_ok());

        header.set_seal(vec![::rlp::encode(&H520::default()).into_vec()]);

        assert!(engine.verify_block_unordered(&header).is_ok());
    }
}
