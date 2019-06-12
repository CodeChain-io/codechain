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

mod params;

use ckey::Address;
use ctypes::{CommonParams, Header};

use self::params::NullEngineParams;
use super::ConsensusEngine;
use crate::block::ExecutedBlock;
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::{EngineError, EngineType};
use crate::error::Error;

/// An engine which does not provide any consensus mechanism and does not seal blocks.
pub struct NullEngine {
    params: NullEngineParams,
    machine: CodeChainMachine,
}

impl NullEngine {
    /// Returns new instance of NullEngine with default VM Factory
    pub fn new(params: NullEngineParams, machine: CodeChainMachine) -> Self {
        NullEngine {
            params,
            machine,
        }
    }
}

impl ConsensusEngine for NullEngine {
    fn name(&self) -> &str {
        "NullEngine"
    }

    fn machine(&self) -> &CodeChainMachine {
        &self.machine
    }

    fn engine_type(&self) -> EngineType {
        EngineType::Solo
    }

    fn on_close_block(
        &self,
        block: &mut ExecutedBlock,
        _parent_header: &Header,
        _parent_common_params: &CommonParams,
    ) -> Result<(), Error> {
        let (author, total_reward) = {
            let header = block.header();
            let author = *header.author();
            let total_reward = self.block_reward(header.number())
                + self.block_fee(Box::new(block.transactions().to_owned().into_iter().map(Into::into)));
            (author, total_reward)
        };
        self.machine.add_balance(block, &author, total_reward)
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        self.params.block_reward
    }

    fn recommended_confirmation(&self) -> u32 {
        1
    }

    fn possible_authors(&self, _block_number: Option<u64>) -> Result<Option<Vec<Address>>, EngineError> {
        Ok(None)
    }
}
