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

use std::sync::Arc;

use cjson;
use cnetwork::NetworkExtension;
use ctypes::U256;

use super::super::machine::{Header, LiveBlock, Machine};
use super::ConsensusEngine;

/// Params for a null engine.
#[derive(Clone, Default)]
pub struct NullEngineParams {
    /// base reward for a block.
    pub block_reward: U256,
}

impl From<cjson::spec::NullEngineParams> for NullEngineParams {
    fn from(p: cjson::spec::NullEngineParams) -> Self {
        NullEngineParams {
            block_reward: p.block_reward.map_or_else(Default::default, Into::into),
        }
    }
}

/// An engine which does not provide any consensus mechanism and does not seal blocks.
pub struct NullEngine<M> {
    params: NullEngineParams,
    machine: M,
}

impl<M> NullEngine<M> {
    /// Returns new instance of NullEngine with default VM Factory
    pub fn new(params: NullEngineParams, machine: M) -> Self {
        NullEngine {
            params,
            machine,
        }
    }
}

impl<M: Default> Default for NullEngine<M> {
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}

impl<M: Machine> ConsensusEngine<M> for NullEngine<M> {
    fn name(&self) -> &str {
        "NullEngine"
    }

    fn machine(&self) -> &M {
        &self.machine
    }

    fn on_close_block(&self, block: &mut M::LiveBlock) -> Result<(), M::Error> {
        let author = *LiveBlock::header(&*block).author();
        self.machine.add_balance(block, &author, &self.params.block_reward)
    }

    fn verify_local_seal(&self, _header: &M::Header) -> Result<(), M::Error> {
        Ok(())
    }

    fn network_extension(&self) -> Option<Arc<NetworkExtension>> {
        None
    }
}
