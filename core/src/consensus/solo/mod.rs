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
use ctypes::machine::{Header, LiveBlock, Transactions, WithBalances};

use self::params::SoloParams;
use super::{ConsensusEngine, Seal};
use crate::consensus::EngineType;
use crate::SignedTransaction;

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

        Solo {
            params,
            machine,
            action_handlers,
        }
    }
}

impl<M: WithBalances> ConsensusEngine<M> for Solo<M>
where
    M::LiveBlock: Transactions<Transaction = SignedTransaction>,
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

    fn engine_type(&self) -> EngineType {
        EngineType::Solo
    }

    fn generate_seal(&self, _block: &M::LiveBlock, _parent: &M::Header) -> Seal {
        Seal::Solo
    }

    fn verify_local_seal(&self, _header: &M::Header) -> Result<(), M::Error> {
        Ok(())
    }

    fn on_close_block(&self, block: &mut M::LiveBlock) -> Result<(), M::Error> {
        let author = *LiveBlock::header(&*block).author();
        let total_reward = self.block_reward(block.header().number())
            + self.block_fee(Box::new(block.transactions().to_owned().into_iter().map(Into::into)));
        self.machine.add_balance(block, &author, total_reward)
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
        let b = OpenBlock::try_new(engine, db, &genesis_header, Default::default(), vec![], false).unwrap();
        let parent_transactions_root = *genesis_header.transactions_root();
        let parent_invoices_root = *genesis_header.invoices_root();
        let b = b.close_and_lock(parent_transactions_root, parent_invoices_root).unwrap();
        if let Some(seal) = engine.generate_seal(b.block(), &genesis_header).seal_fields() {
            assert!(b.try_seal(engine, seal).is_ok());
        }
    }

    #[test]
    fn fail_to_verify() {
        let engine = Scheme::new_test_solo().engine;
        let mut header: Header = Header::default();

        assert!(engine.verify_block_basic(&header).is_ok());

        header.set_seal(vec![::rlp::encode(&H520::default()).into_vec()]);

        assert!(engine.verify_block_unordered(&header).is_ok());
    }
}
