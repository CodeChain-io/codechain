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

use std::sync::{Arc, Weak};

use ckey::Address;
use cstate::{ActionHandler, HitHandler};
use ctypes::{CommonParams, Header};
use parking_lot::RwLock;

use self::params::SoloParams;
use super::stake;
use super::{ConsensusEngine, Seal};
use crate::block::{ExecutedBlock, IsBlock};
use crate::client::ConsensusClient;
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::{EngineError, EngineType};
use crate::error::Error;

/// A consensus engine which does not provide any consensus mechanism.
pub struct Solo {
    client: RwLock<Option<Weak<dyn ConsensusClient>>>,
    params: SoloParams,
    machine: CodeChainMachine,
    action_handlers: Vec<Arc<dyn ActionHandler>>,
}

impl Solo {
    /// Returns new instance of Solo over the given state machine.
    pub fn new(params: SoloParams, machine: CodeChainMachine) -> Self {
        let mut action_handlers: Vec<Arc<dyn ActionHandler>> = Vec::new();
        if params.enable_hit_handler {
            action_handlers.push(Arc::new(HitHandler::new()));
        }
        action_handlers.push(Arc::new(stake::Stake::new(params.genesis_stakes.clone())));

        Solo {
            client: Default::default(),
            params,
            machine,
            action_handlers,
        }
    }

    fn client(&self) -> Option<Arc<dyn ConsensusClient>> {
        self.client.read().as_ref()?.upgrade()
    }
}

impl ConsensusEngine for Solo {
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

    fn generate_seal(&self, _block: Option<&ExecutedBlock>, _parent: &Header) -> Seal {
        Seal::Solo
    }

    fn on_close_block(
        &self,
        block: &mut ExecutedBlock,
        _term_common_params: Option<&CommonParams>,
    ) -> Result<(), Error> {
        let client = self.client().ok_or(EngineError::CannotOpenBlock)?;

        let parent_hash = *block.header().parent_hash();
        let parent = client.block_header(&parent_hash.into()).expect("Parent header must exist");
        let parent_common_params = client.common_params(parent_hash.into()).expect("CommonParams of parent must exist");
        let author = *block.header().author();
        let (total_reward, total_min_fee) = {
            let transactions = block.transactions();
            let block_reward = self.block_reward(block.header().number());
            let total_min_fee: u64 = transactions.iter().map(|tx| tx.fee).sum();
            let min_fee: u64 =
                transactions.iter().map(|tx| CodeChainMachine::min_cost(&parent_common_params, &tx.action)).sum();
            (block_reward + total_min_fee, min_fee)
        };

        assert!(total_reward >= total_min_fee, "{} >= {}", total_reward, total_min_fee);
        let stakes = stake::get_stakes(block.state()).expect("Cannot get Stake status");

        let mut distributor = stake::fee_distribute(total_min_fee, &stakes);
        for (address, share) in &mut distributor {
            self.machine.add_balance(block, &address, share)?
        }

        let block_author_reward = total_reward - total_min_fee + distributor.remaining_fee();

        let term_seconds = parent_common_params.term_seconds();
        if term_seconds == 0 {
            self.machine.add_balance(block, &author, block_author_reward)?;
            return Ok(())
        }
        stake::v0::add_intermediate_rewards(block.state_mut(), author, block_author_reward)?;
        let last_term_finished_block_num = {
            let header = block.header();
            let current_term_period = header.timestamp() / term_seconds;
            let parent_term_period = parent.timestamp() / term_seconds;
            if current_term_period == parent_term_period {
                return Ok(())
            }
            header.number()
        };
        stake::v0::move_current_to_previous_intermediate_rewards(&mut block.state_mut())?;
        let rewards = stake::v0::drain_previous_rewards(&mut block.state_mut())?;
        for (address, reward) in rewards {
            self.machine.add_balance(block, &address, reward)?;
        }

        stake::on_term_close(block.state_mut(), last_term_finished_block_num, &[])?;
        Ok(())
    }

    fn register_client(&self, client: Weak<dyn ConsensusClient>) {
        *self.client.write() = Some(Weak::clone(&client));
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        self.params.block_reward
    }

    fn recommended_confirmation(&self) -> u32 {
        1
    }

    fn action_handlers(&self) -> &[Arc<dyn ActionHandler>] {
        &self.action_handlers
    }

    fn possible_authors(&self, _block_number: Option<u64>) -> Result<Option<Vec<Address>>, EngineError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ctypes::{CommonParams, Header};
    use primitives::H520;

    use crate::block::{IsBlock, OpenBlock};
    use crate::client::{ConsensusClient, TestBlockChainClient};
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    #[test]
    fn seal() {
        let scheme = Scheme::new_test_solo();
        let client = Arc::new(TestBlockChainClient::new_with_scheme(scheme));
        let engine = client.scheme.engine.clone();
        engine.register_client(Arc::downgrade(&(client.clone() as Arc<dyn ConsensusClient>)));
        let db = client.scheme.ensure_genesis_state(get_temp_state_db()).unwrap();
        let genesis_header = client.scheme.genesis_header();
        let b = OpenBlock::try_new(&*engine, db, &genesis_header, Default::default(), vec![]).unwrap();
        let term_common_params = CommonParams::default_for_test();
        let b = b.close_and_lock(&genesis_header, Some(&term_common_params)).unwrap();
        if let Some(seal) = engine.generate_seal(Some(b.block()), &genesis_header).seal_fields() {
            assert!(b.try_seal(&*engine, seal).is_ok());
        }
    }

    #[test]
    fn fail_to_verify() {
        let engine = Scheme::new_test_solo().engine;
        let mut header: Header = Header::default();

        assert!(engine.verify_header_basic(&header).is_ok());

        header.set_seal(vec![::rlp::encode(&H520::default())]);

        assert!(engine.verify_block_seal(&header).is_ok());
    }
}
