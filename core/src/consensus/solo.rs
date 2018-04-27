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
use cnetwork::{Api, NetworkExtension};
use ctypes::U256;

use super::super::machine::{Header, LiveBlock, Machine, Transactions};
use super::{ConsensusEngine, Seal};

/// Params for a null engine.
#[derive(Clone, Default)]
pub struct SoloParams {
    /// base reward for a block.
    pub block_reward: U256,
}

impl From<cjson::spec::SoloParams> for SoloParams {
    fn from(p: cjson::spec::SoloParams) -> Self {
        SoloParams {
            block_reward: p.block_reward.map_or_else(Default::default, Into::into),
        }
    }
}

/// A consensus engine which does not provide any consensus mechanism.
pub struct Solo<M> {
    params: SoloParams,
    machine: M,
}

impl<M> Solo<M> {
    /// Returns new instance of Solo over the given state machine.
    pub fn new(params: SoloParams, machine: M) -> Self {
        Solo {
            params,
            machine,
        }
    }
}

impl<M: Machine> ConsensusEngine<M> for Solo<M>
where
    M::LiveBlock: Transactions,
{
    fn name(&self) -> &str {
        "Solo"
    }

    fn machine(&self) -> &M {
        &self.machine
    }

    fn seals_internally(&self) -> Option<bool> {
        Some(true)
    }

    fn generate_seal(&self, block: &M::LiveBlock, _parent: &M::Header) -> Seal {
        if block.transactions().is_empty() {
            Seal::None
        } else {
            Seal::Regular(Vec::new())
        }
    }

    fn verify_local_seal(&self, _header: &M::Header) -> Result<(), M::Error> {
        Ok(())
    }

    fn on_close_block(&self, block: &mut M::LiveBlock) -> Result<(), M::Error> {
        let author = *LiveBlock::header(&*block).author();
        let reward = self.params.block_reward;
        if reward == U256::zero() {
            return Ok(())
        }

        self.machine.add_balance(block, &author, &reward)
    }

    fn network_extension(&self) -> Option<Arc<NetworkExtension>> {
        None
    }
}

impl<M: Machine> NetworkExtension for Solo<M> {
    fn name(&self) -> String {
        "Solo".to_string()
    }

    fn need_encryption(&self) -> bool {
        false
    }

    fn on_initialize(&self, _api: Arc<Api>) {}
}

#[cfg(test)]
mod tests {
    use ctypes::H520;

    use super::super::super::block::{IsBlock, OpenBlock};
    use super::super::super::header::Header;
    use super::super::super::spec::Spec;
    use super::super::super::tests::helpers::get_temp_state_db;
    use super::super::Seal;

    #[test]
    fn solo_can_seal() {
        let spec = Spec::new_solo();
        let engine = &*spec.engine;
        let db = spec.ensure_db_good(get_temp_state_db(), &Default::default()).unwrap();
        let genesis_header = spec.genesis_header();
        let b =
            OpenBlock::new(engine, Default::default(), db, &genesis_header, Default::default(), vec![], false).unwrap();
        let b = b.close_and_lock();
        if let Seal::Regular(seal) = engine.generate_seal(b.block(), &genesis_header) {
            assert!(b.try_seal(engine, seal).is_ok());
        }
    }

    #[test]
    fn solo_cant_verify() {
        let engine = Spec::new_solo().engine;
        let mut header: Header = Header::default();

        assert!(engine.verify_block_basic(&header).is_ok());

        header.set_seal(vec![::rlp::encode(&H520::default()).into_vec()]);

        assert!(engine.verify_block_unordered(&header).is_ok());
    }
}
