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

