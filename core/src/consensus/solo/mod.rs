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

use std::sync::Arc;

use cstate::{ActionHandler, HitHandler};
use ctypes::CommonParams;

use self::params::SoloParams;
use super::stake;
use super::validator_set;
use super::{ConsensusEngine, Seal};
use crate::block::{ExecutedBlock, IsBlock};
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::EngineType;
use crate::error::Error;
use crate::header::Header;

/// A consensus engine which does not provide any consensus mechanism.
pub struct Solo<M> {
    params: SoloParams,
    machine: M,
    action_handlers: Vec<Arc<ActionHandler>>,
}

impl<M> Solo<M> {
    /// Returns new instance of Solo over the given state machine.
    pub fn new(params: SoloParams, machine: M) -> Self {
        let mut action_handlers: Vec<Arc<ActionHandler>> = Vec::new();
        if params.enable_hit_handler {
            action_handlers.push(Arc::new(HitHandler::new()));
        }
        action_handlers.push(Arc::new(stake::Stake::new(
            params.genesis_stakes.clone(),
            Arc::new(validator_set::null_validator::NullValidator {}),
        )));

        Solo {
            params,
            machine,
            action_handlers,
        }
    }
}

impl ConsensusEngine for Solo<CodeChainMachine> {
    fn name(&self) -> &str {
        "Solo"
    }

    fn machine(&self) -> &CodeChainMachine {
        &self.machine
    }

    fn seals_internally(&self) -> Option<bool> {
        Some(true)
    }

    fn engine_type(&self) -> EngineType {
        EngineType::Solo
    }

    fn generate_seal(&self, _block: &ExecutedBlock, _parent: &Header) -> Seal {
        Seal::Solo
    }

    fn on_close_block(&self, block: &mut ExecutedBlock, parent_common_params: &CommonParams) -> Result<(), Error> {
        let author = *block.header().author();
        let (total_reward, min_fee) = {
            let transactions = block.transactions();
            let block_reward = self.block_reward(block.header().number());
            let total_fee: u64 = transactions.iter().map(|tx| tx.fee).sum();
            let min_fee: u64 =
                transactions.iter().map(|tx| CodeChainMachine::min_cost(&parent_common_params, &tx.action)).sum();
            (block_reward + total_fee, min_fee)
        };

        assert!(total_reward >= min_fee, "{} >= {}", total_reward, min_fee);
        let stakes = stake::get_stakes(block.state()).expect("Cannot get Stake status");
        for (address, share) in stake::fee_distribute(&author, min_fee, &stakes) {
            self.machine.add_balance(block, &address, share)?
        }
        if total_reward != min_fee {
            self.machine.add_balance(block, &author, total_reward - min_fee)?
        }
        Ok(())
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        self.params.block_reward
    }

    fn recommended_confirmation(&self) -> u32 {
        1
    }

    fn action_handlers(&self) -> &[Arc<ActionHandler>] {
        &self.action_handlers
    }
}

#[cfg(test)]
mod tests {
    use ctypes::CommonParams;
    use primitives::H520;

    use crate::block::{IsBlock, OpenBlock};
    use crate::header::Header;
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    #[test]
    fn seal() {
        let scheme = Scheme::new_test_solo();
        let engine = &*scheme.engine;
        let db = scheme.ensure_genesis_state(get_temp_state_db()).unwrap();
        let genesis_header = scheme.genesis_header();
        let b = OpenBlock::try_new(engine, db, &genesis_header, Default::default(), vec![]).unwrap();
        let parent_transactions_root = *genesis_header.transactions_root();
        let parent_common_params = CommonParams::default_for_test();
        let b = b.close_and_lock(parent_transactions_root, &parent_common_params).unwrap();
        if let Some(seal) = engine.generate_seal(b.block(), &genesis_header).seal_fields() {
            assert!(b.try_seal(engine, seal).is_ok());
        }
    }

    #[test]
    fn fail_to_verify() {
        let engine = Scheme::new_test_solo().engine;
        let mut header: Header = Header::default();

        assert!(engine.verify_header_basic(&header).is_ok());

        header.set_seal(vec![::rlp::encode(&H520::default()).into_vec()]);

        assert!(engine.verify_block_seal(&header).is_ok());
    }
}
