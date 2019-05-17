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

use std::iter::Iterator;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Weak};

use ckey::Address;
use cnetwork::NetworkService;
use crossbeam_channel as crossbeam;
use cstate::ActionHandler;
use primitives::H256;

use super::super::stake;
use super::super::{ConsensusEngine, EngineError, Seal};
use super::network::TendermintExtension;
pub use super::params::{TendermintParams, TimeoutParams};
use super::worker;
use super::{ChainNotify, Tendermint, SEAL_FIELDS};
use crate::account_provider::AccountProvider;
use crate::block::*;
use crate::client::{Client, EngineClient};
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::EngineType;
use crate::error::Error;
use crate::header::Header;
use crate::views::HeaderView;
use consensus::tendermint::params::TimeGapParams;

impl ConsensusEngine for Tendermint {
    fn name(&self) -> &str {
        "Tendermint"
    }

    fn machine(&self) -> &CodeChainMachine {
        &self.machine.as_ref()
    }

    /// (consensus view, proposal signature, authority signatures)
    fn seal_fields(&self, _header: &Header) -> usize {
        SEAL_FIELDS
    }

    /// Should this node participate.
    fn seals_internally(&self) -> Option<bool> {
        Some(self.has_signer.load(AtomicOrdering::SeqCst))
    }

    fn engine_type(&self) -> EngineType {
        EngineType::PBFT
    }

    /// Attempt to seal generate a proposal seal.
    ///
    /// This operation is synchronous and may (quite reasonably) not be available, in which case
    /// `Seal::None` will be returned.
    fn generate_seal(&self, block: &ExecutedBlock, parent: &Header) -> Seal {
        let (result, receiver) = crossbeam::bounded(1);
        let block_number = block.header().number();
        let parent_hash = parent.hash();
        self.inner
            .send(worker::Event::GenerateSeal {
                block_number,
                parent_hash,
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    /// Called when the node is the leader and a proposal block is generated from the miner.
    /// This writes the proposal information and go to the prevote step.
    fn proposal_generated(&self, sealed_block: &SealedBlock) {
        self.inner.send(worker::Event::ProposalGenerated(Box::from(sealed_block.clone()))).unwrap();
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::VerifyBlockBasic {
                header: Box::from(header.clone()),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::VerifyBlockExternal {
                header: Box::from(header.clone()),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn populate_from_parent(&self, header: &mut Header, _parent: &Header) {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::CalculateScore {
                block_number: header.number(),
                result,
            })
            .unwrap();
        let score = receiver.recv().unwrap();
        header.set_score(score);
    }

    /// Equivalent to a timeout: to be used for tests.
    fn on_timeout(&self, token: usize) {
        self.inner.send(worker::Event::OnTimeout(token)).unwrap();
    }

    fn stop(&self) {}

    fn on_close_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        let author = *block.header().author();
        let (total_fee, min_fee) = {
            let transactions = block.transactions();
            let total_fee: u64 = transactions.iter().map(|tx| tx.fee).sum();
            let block_number = block.header().number();
            let min_fee = transactions.iter().map(|tx| self.machine.min_cost(&tx.action, Some(block_number))).sum();
            (total_fee, min_fee)
        };
        assert!(total_fee >= min_fee, "{} >= {}", total_fee, min_fee);
        let stakes = stake::get_stakes(block.state()).expect("Cannot get Stake status");

        for (address, share) in stake::fee_distribute(&author, min_fee, &stakes) {
            self.machine.add_balance(block, &address, share)?
        }
        if total_fee != min_fee {
            self.machine.add_balance(block, &author, total_fee - min_fee)?
        }
        Ok(())
    }

    fn register_client(&self, client: Weak<EngineClient>) {
        *self.client.write() = Some(Weak::clone(&client));
    }

    fn handle_message(&self, rlp: &[u8]) -> Result<(), EngineError> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::HandleMessages {
                messages: vec![rlp.to_owned()],
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn is_proposal(&self, header: &Header) -> bool {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::IsProposal {
                block_number: header.number(),
                block_hash: header.hash(),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn set_signer(&self, ap: Arc<AccountProvider>, address: Address) {
        self.has_signer.store(true, AtomicOrdering::SeqCst);
        self.inner
            .send(worker::Event::SetSigner {
                ap,
                address,
            })
            .unwrap();
    }

    fn register_network_extension_to_service(&self, service: &NetworkService) {
        let timeouts = self.timeouts;

        let inner = self.inner.clone();
        let extension = service.register_extension(move |api| TendermintExtension::new(inner, timeouts, api));
        let client = Weak::clone(self.client.read().as_ref().unwrap());
        self.extension_initializer.send((extension, client)).unwrap();

        let (result, receiver) = crossbeam::bounded(1);
        self.inner.send(worker::Event::Restore(result)).unwrap();
        receiver.recv().unwrap();
    }

    fn register_time_gap_config_to_worker(&self, time_gap_params: TimeGapParams) {
        self.external_params_initializer.send(time_gap_params).unwrap();
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        self.block_reward
    }

    fn recommended_confirmation(&self) -> u32 {
        1
    }

    fn register_chain_notify(&self, client: &Client) {
        client.add_notify(Arc::downgrade(&self.chain_notify) as Weak<ChainNotify>);
    }

    fn get_best_block_from_best_proposal_header(&self, header: &HeaderView) -> H256 {
        header.parent_hash()
    }

    fn can_change_canon_chain(&self, header: &HeaderView) -> bool {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(worker::Event::AllowedHeight {
                result,
            })
            .unwrap();
        let allowed_height = receiver.recv().unwrap();
        header.number() >= allowed_height
    }

    fn action_handlers(&self) -> &[Arc<ActionHandler>] {
        &self.action_handlers
    }
}
