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

mod backup;
mod message;
mod params;
mod stake;
pub mod types;

use std::cmp;
use std::collections::{HashMap, HashSet};
use std::iter::Iterator;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};

use ccrypto::blake256;
use ckey::{public_to_address, recover_schnorr, verify_schnorr, Address, Message, SchnorrSignature};
use cnetwork::{Api, NetworkExtension, NetworkService, NodeId};
use cstate::ActionHandler;
use ctimer::{TimeoutHandler, TimerToken};
use ctypes::machine::WithBalances;
use ctypes::util::unexpected::{Mismatch, OutOfBounds};
use ctypes::BlockNumber;
use parking_lot::{Mutex, MutexGuard, RwLock};
use primitives::{u256_from_u128, Bytes, H256, U256};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rlp::{Encodable, UntrustedRlp};

use self::backup::{backup, restore, BackupView};
use self::message::*;
pub use self::params::{TendermintParams, TimeoutParams};
use self::types::{BitSet, Height, PeerState, Step, TendermintSealView, TendermintState, View};
use super::signer::EngineSigner;
use super::validator_set::validator_list::ValidatorList;
use super::validator_set::ValidatorSet;
use super::vote_collector::VoteCollector;
use super::{ConsensusEngine, ConstructedVerifier, EngineError, EpochChange, Seal};
use crate::account_provider::AccountProvider;
use crate::block::*;
use crate::client::{Client, EngineClient};
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::EngineType;
use crate::encoded;
use crate::error::{BlockError, Error};
use crate::header::Header;
use crate::views::{BlockView, HeaderView};
use crate::BlockId;
use ChainNotify;

/// Timer token representing the consensus step timeouts.
const ENGINE_TIMEOUT_TOKEN_NONCE_BASE: TimerToken = 23;
/// Timer token for empty proposal blocks.
const ENGINE_TIMEOUT_EMPTY_PROPOSAL: TimerToken = 22;

pub type BlockHash = H256;

/// ConsensusEngine using `Tendermint` consensus algorithm
pub struct Tendermint {
    inner: Mutex<TendermintInner>,
    machine: Arc<CodeChainMachine>,
    /// Action handlers for this consensus method
    action_handlers: Vec<Arc<ActionHandler>>,
}

struct TendermintInner {
    engine: Option<Weak<Tendermint>>,
    client: Option<Weak<EngineClient>>,
    /// Blockchain height.
    height: Height,
    /// Consensus view.
    view: View,
    /// Consensus step.
    step: TendermintState,
    /// Record current round's received votes as bit set
    votes_received: BitSet,
    /// Vote accumulator.
    votes: VoteCollector<ConsensusMessage>,
    /// Used to sign messages and proposals.
    signer: EngineSigner,
    /// Message for the last PoLC.
    lock_change: Option<ConsensusMessage>,
    /// Last lock view.
    last_lock: Option<View>,
    /// hash of the proposed block, used for seal submission.
    proposal: Option<H256>,
    /// The last confirmed view from the commit step.
    last_confirmed_view: View,
    /// Set used to determine the current validators.
    validators: Box<ValidatorSet>,
    /// Reward per block, in base units.
    block_reward: u64,
    /// TimeoutParams for delayed creation of the TendermintExtension.
    timeouts: TimeoutParams,
    /// Network extension, must be set later.
    extension: Option<Arc<TendermintExtension>>,
    /// codechain machine descriptor
    machine: Arc<CodeChainMachine>,
    /// Chain notify
    chain_notify: Arc<TendermintChainNotify>,
}

impl Tendermint {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    /// Create a new instance of Tendermint engine
    pub fn new(our_params: TendermintParams, machine: CodeChainMachine) -> Arc<Self> {
        let action_handlers: Vec<Arc<ActionHandler>> =
            vec![Arc::new(stake::Stake::new(our_params.genesis_stakes.clone()))];
        let machine = Arc::new(machine);
        let inner = TendermintInner::new(our_params, machine.clone());

        let engine = Arc::new(Tendermint {
            inner: Mutex::new(inner),
            machine,
            action_handlers,
        });

        {
            let mut guard = engine.inner.lock();
            let engine_weak = Arc::downgrade(&engine);
            guard.engine = Some(Weak::clone(&engine_weak));
            guard.chain_notify.register_tendermint(Weak::clone(&engine_weak));
        }

        engine
    }
}

impl TendermintInner {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    /// Create a new instance of Tendermint engine
    pub fn new(our_params: TendermintParams, machine: Arc<CodeChainMachine>) -> Self {
        let chain_notify = TendermintChainNotify::new();
        TendermintInner {
            engine: None,
            client: None,
            height: 1,
            view: 0,
            step: TendermintState::Propose,
            votes: Default::default(),
            signer: Default::default(),
            lock_change: None,
            last_lock: None,
            proposal: None,
            last_confirmed_view: 0,
            validators: our_params.validators,
            block_reward: our_params.block_reward,
            timeouts: our_params.timeouts,
            extension: None,
            chain_notify: Arc::new(chain_notify),
            machine,
            votes_received: BitSet::new(),
        }
    }
}

impl TendermintInner {
    fn engine(&self) -> Arc<Tendermint> {
        self.engine
            .as_ref()
            .expect("Only writes in initialize")
            .upgrade()
            .expect("Reference to itself should not be dropped")
    }

    /// The client is a thread-safe struct. Using it in multi-threads is safe.
    fn client(&self) -> Arc<EngineClient> {
        self.client.as_ref().expect("Only writes in initialize").upgrade().expect("Client lives longer than consensus")
    }

    /// Get previous block hash to determine validator set
    fn prev_block_hash(&self) -> H256 {
        let prev_height = (self.height - 1) as u64;
        self.client()
            .block_header(&BlockId::Number(prev_height))
            .expect("Height is increased when previous block is imported")
            .hash()
    }

    /// Check the committed block of the current height is imported to the canonical chain
    fn check_current_block_exists(&self) -> bool {
        self.client().block(&BlockId::Number(self.height as u64)).is_some()
    }

    /// Check Tendermint can move from the commit step to the propose step
    fn can_move_from_commit_to_propose(&self) -> bool {
        let vote_step = VoteStep::new(self.height, self.last_confirmed_view, Step::Precommit);
        self.step.is_commit() && self.has_all_votes(&vote_step) && self.check_current_block_exists()
    }

    /// Find the designated for the given view.
    fn view_proposer(&self, bh: &H256, height: Height, view: View) -> Address {
        let proposer_nonce = height + view;
        ctrace!(ENGINE, "Proposer nonce: {}", proposer_nonce);
        self.validators.get_address(bh, proposer_nonce)
    }

    pub fn proposal_at(&self, height: Height, view: View) -> Option<(SchnorrSignature, usize, Bytes)> {
        let vote_step = VoteStep {
            height,
            view,
            step: Step::Propose,
        };

        let all_votes = self.votes.get_all_votes_in_round(&vote_step);
        let proposal = all_votes.first();

        proposal.and_then(|proposal| {
            let block_hash = proposal.on.block_hash.expect("Proposal message always include block hash");
            let bytes = self.client().block(&BlockId::Hash(block_hash)).map(|block| block.into_inner());
            bytes.map(|bytes| (proposal.signature, proposal.signer_index, bytes))
        })
    }

    pub fn vote_step(&self) -> VoteStep {
        VoteStep {
            height: self.height,
            view: self.view,
            step: self.step.to_step(),
        }
    }

    pub fn need_proposal(&self) -> bool {
        self.proposal.is_none()
    }

    pub fn get_all_votes_and_authors(&self, vote_step: &VoteStep, requested: &BitSet) -> Vec<ConsensusMessage> {
        self.votes
            .get_all_votes_and_indices_in_round(vote_step)
            .into_iter()
            .filter(|(index, _)| requested.is_set(*index))
            .map(|(_, v)| v)
            .collect()
    }

    /// Check if address is a proposer for given view.
    fn check_view_proposer(&self, bh: &H256, height: Height, view: View, address: &Address) -> Result<(), EngineError> {
        let proposer = self.view_proposer(bh, height, view);
        if proposer == *address {
            Ok(())
        } else {
            Err(EngineError::NotProposer(Mismatch {
                expected: proposer,
                found: *address,
            }))
        }
    }

    /// Check if current signer is the current proposer.
    fn is_signer_proposer(&self, bh: &H256) -> bool {
        let proposer = self.view_proposer(bh, self.height, self.view);
        self.signer.is_address(&proposer)
    }

    fn is_view(&self, message: &ConsensusMessage) -> bool {
        message.on.step.is_view(self.height, self.view)
    }

    fn is_step(&self, message: &ConsensusMessage) -> bool {
        message.on.step.is_step(self.height, self.view, self.step.to_step())
    }

    fn is_authority(&self, prev_hash: &H256, address: &Address) -> bool {
        self.validators.contains_address(&prev_hash, address)
    }

    fn check_above_threshold(&self, n: usize) -> Result<(), EngineError> {
        let threshold = self.validators.count(&self.prev_block_hash()) * 2 / 3;
        if n > threshold {
            Ok(())
        } else {
            Err(EngineError::BadSealFieldSize(OutOfBounds {
                min: Some(threshold),
                max: None,
                found: n,
            }))
        }
    }

    fn has_enough_any_votes(&self) -> bool {
        let step_votes = self.votes.count_round_votes(&VoteStep::new(self.height, self.view, self.step.to_step()));
        self.check_above_threshold(step_votes).is_ok()
    }

    fn has_all_votes(&self, vote_step: &VoteStep) -> bool {
        let step_votes = self.votes.count_round_votes(vote_step);
        self.validators.count(&self.prev_block_hash()) == step_votes
    }

    fn has_enough_aligned_votes(&self, message: &ConsensusMessage) -> bool {
        let aligned_count = self.votes.count_aligned_votes(&message);
        self.check_above_threshold(aligned_count).is_ok()
    }

    fn has_enough_precommit_votes(&self, block_hash: H256) -> bool {
        let vote_step = VoteStep::new(self.height, self.view, Step::Precommit);
        let count = self.votes.count_block_round_votes(&vote_step, &Some(block_hash));
        self.check_above_threshold(count).is_ok()
    }

    fn extension(&self) -> &Arc<TendermintExtension> {
        self.extension.as_ref().expect("TendermintExtension must be registered")
    }

    fn broadcast_message(&self, message: Bytes) {
        self.extension().broadcast_message(message);
    }

    fn broadcast_state(&self, vote_step: &VoteStep, proposal: Option<H256>, lock_view: Option<View>, votes: BitSet) {
        self.extension().broadcast_state(vote_step, proposal, lock_view, votes);
    }

    fn request_all_votes(&self, vote_step: &VoteStep) {
        self.extension().request_all_votes(vote_step);
    }

    fn request_proposal(&self, height: Height, view: View) {
        self.extension().request_proposal_to_any(height, view);
    }

    fn update_sealing(&self, parent_block_hash: H256) {
        self.client().update_sealing(BlockId::Hash(parent_block_hash), true);
    }

    fn save_last_confirmed_view(&mut self, view: View) {
        self.last_confirmed_view = view;
    }

    fn increment_view(&mut self, n: View) {
        cinfo!(ENGINE, "increment_view: New view.");
        self.view += n;
        self.proposal = None;
        self.votes_received = BitSet::new();
    }

    fn should_unlock(&self, lock_change_view: View) -> bool {
        self.last_lock.unwrap_or(0) < lock_change_view && lock_change_view < self.view
    }

    fn move_to_height(&mut self, height: Height) {
        assert!(height > self.height, "{} < {}", height, self.height);
        cinfo!(ENGINE, "Transitioning to height {}.", height);
        self.last_lock = None;
        self.height = height;
        self.view = 0;
        self.lock_change = None;
        self.proposal = None;
        self.votes_received = BitSet::new();
    }

    fn move_to_step(&mut self, step: Step, is_restoring: bool) {
        let prev_step = mem::replace(&mut self.step, step.into());
        if !is_restoring {
            self.backup();
        }
        self.extension().set_timer_step(step, self.view);
        let vote_step = VoteStep::new(self.height, self.view, step);

        // If there are not enough pre-votes or pre-commits,
        // move_to_step can be called with the same step
        // Also, when moving to the commit step,
        // keep `votes_received` for gossiping.
        if prev_step.to_step() != step && step != Step::Commit {
            self.votes_received = BitSet::new();
        }

        // need to reset vote
        self.broadcast_state(&vote_step, self.proposal, self.last_lock, self.votes_received);
        match step {
            Step::Propose => {
                cinfo!(ENGINE, "move_to_step: Propose.");
                if let Some(hash) = self.votes.get_block_hashes(&vote_step).first() {
                    if self.client().block_header(&BlockId::Hash(*hash)).is_some() {
                        self.proposal = Some(*hash);
                        self.move_to_step(Step::Prevote, is_restoring);
                    } else {
                        cwarn!(ENGINE, "Proposal is received but not imported");
                        // Proposal is received but is not verified yet.
                        // Wait for verification.
                        return
                    }
                } else {
                    let parent_block_hash = &self.prev_block_hash();
                    if self.is_signer_proposer(parent_block_hash) {
                        cinfo!(ENGINE, "I am a proposer, I'll create a block");
                        self.update_sealing(*parent_block_hash);
                        self.step = TendermintState::ProposeWaitBlockGeneration {
                            parent_hash: *parent_block_hash,
                        };
                    } else {
                        self.request_proposal(vote_step.height, vote_step.view);
                    }
                }
            }
            Step::Prevote => {
                cinfo!(ENGINE, "move_to_step: Prevote.");
                self.request_all_votes(&vote_step);
                if !self.already_generated_message() {
                    let block_hash = match self.lock_change {
                        Some(ref m) if !self.should_unlock(m.on.step.view) => m.on.block_hash,
                        _ => self.proposal,
                    };
                    self.generate_and_broadcast_message(block_hash, is_restoring);
                }
            }
            Step::Precommit => {
                cinfo!(ENGINE, "move_to_step: Precommit.");
                self.request_all_votes(&vote_step);
                if !self.already_generated_message() {
                    let block_hash = match self.lock_change {
                        Some(ref m) if self.is_view(m) && m.on.block_hash.is_some() => {
                            cinfo!(ENGINE, "Setting last lock: {}", m.on.step.view);
                            self.last_lock = Some(m.on.step.view);
                            m.on.block_hash
                        }
                        _ => None,
                    };
                    self.generate_and_broadcast_message(block_hash, is_restoring);
                }
            }
            Step::Commit => {
                cinfo!(ENGINE, "move_to_step: Commit.");
            }
        }
    }

    fn already_generated_message(&self) -> bool {
        match self.signer_index(&self.prev_block_hash()) {
            Some(signer_index) => self.votes_received.is_set(signer_index),
            _ => false,
        }
    }

    fn generate_and_broadcast_message(&mut self, block_hash: Option<BlockHash>, is_restoring: bool) {
        if let Some(message) = self.generate_message(block_hash, is_restoring) {
            if !is_restoring {
                self.backup();
            }
            self.broadcast_message(message);
        }
    }

    fn generate_message(&mut self, block_hash: Option<BlockHash>, is_restoring: bool) -> Option<Bytes> {
        let height = self.height;
        let r = self.view;
        let on = VoteOn {
            step: VoteStep::new(height, r, self.step.to_step()),
            block_hash,
        };
        let vote_info = on.rlp_bytes();
        match (self.signer_index(&self.prev_block_hash()), self.sign(blake256(&vote_info))) {
            (Some(signer_index), Ok(signature)) => {
                let message = ConsensusMessage {
                    signature,
                    signer_index,
                    on,
                };
                let message_rlp = message.rlp_bytes().into_vec();
                self.votes_received.set(signer_index);
                self.votes.vote(message.clone());
                cinfo!(ENGINE, "Generated {:?} as {}th validator.", message, signer_index);
                self.handle_valid_message(&message, is_restoring);

                Some(message_rlp)
            }
            (None, _) => {
                ctrace!(ENGINE, "No message, since there is no engine signer.");
                None
            }
            (Some(signer_index), Err(error)) => {
                ctrace!(ENGINE, "{}th validator could not sign the message {}", signer_index, error);
                None
            }
        }
    }

    fn handle_valid_message(&mut self, message: &ConsensusMessage, is_restoring: bool) {
        let vote_step = &message.on.step;
        let is_newer_than_lock = match &self.lock_change {
            Some(lock) => *vote_step > lock.on.step,
            None => true,
        };
        let has_enough_aligned_votes = self.has_enough_aligned_votes(message);
        let lock_change = is_newer_than_lock
            && vote_step.height == self.height
            && vote_step.step == Step::Prevote
            && message.on.block_hash.is_some()
            && has_enough_aligned_votes;
        if lock_change {
            cinfo!(
                ENGINE,
                "handle_valid_message: Lock change to {}-{} at {}-{}",
                vote_step.height,
                vote_step.view,
                self.height,
                self.view
            );
            self.lock_change = Some(message.clone());
        }
        // Check if it can affect the step transition.
        if self.is_step(message) {
            let next_step = match self.step {
                TendermintState::Precommit if message.on.block_hash.is_none() && has_enough_aligned_votes => {
                    self.increment_view(1);
                    Some(Step::Propose)
                }
                TendermintState::Precommit if has_enough_aligned_votes => {
                    let bh = message.on.block_hash.expect("previous guard ensures is_some; qed");
                    if self.client().block(&BlockId::Hash(bh)).is_some() {
                        // Commit the block, and update the last confirmed view
                        self.save_last_confirmed_view(message.on.step.view);

                        // Update the best block hash as the hash of the committed block
                        self.client().update_best_as_committed(bh);
                        Some(Step::Commit)
                    } else {
                        cwarn!(ENGINE, "Cannot find a proposal which committed");
                        self.increment_view(1);
                        Some(Step::Propose)
                    }
                }
                // Avoid counting votes twice.
                TendermintState::Prevote if lock_change => Some(Step::Precommit),
                TendermintState::Prevote if has_enough_aligned_votes => Some(Step::Precommit),
                _ => None,
            };

            if let Some(step) = next_step {
                ctrace!(ENGINE, "Transition to {:?} triggered.", step);
                self.move_to_step(step, is_restoring);
                return
            }
        } else if vote_step.step == Step::Precommit
            && self.height == vote_step.height
            && self.can_move_from_commit_to_propose()
        {
            cinfo!(
                ENGINE,
                "Transition to Propose because all pre-commits are received and the canonical chain is appended"
            );
            let height = self.height;
            self.move_to_height(height + 1);
            self.move_to_step(Step::Propose, is_restoring);
            return
        }

        // self.move_to_step() calls self.broadcast_state()
        // If self.move_to_step() is not called, call self.broadcast_state() in here.
        self.broadcast_state(&self.vote_step(), self.proposal, self.last_lock, self.votes_received);
    }

    pub fn on_imported_proposal(&mut self, proposal: &Header) {
        if proposal.number() < 1 {
            return
        }

        let height = proposal.number() as Height;
        let prev_block_view = previous_block_view(proposal).expect("The proposal is verified");
        let on = VoteOn {
            step: VoteStep::new(height - 1, prev_block_view, Step::Precommit),
            block_hash: Some(*proposal.parent_hash()),
        };
        let seal_view = TendermintSealView::new(proposal.seal());
        for (index, signature) in seal_view.signatures().expect("The proposal is verified") {
            let message = ConsensusMessage {
                signature,
                signer_index: index,
                on: on.clone(),
            };
            if !self.votes.is_old_or_known(&message) {
                self.votes.vote(message);
            }
        }

        // Since the votes needs at least one vote to check the old votes,
        // we should remove old votes after inserting current votes.
        self.votes.throw_out_old(&VoteStep {
            height: (proposal.number() - 1) as usize,
            view: 0,
            step: Step::Propose,
        });

        let proposal_view = consensus_view(proposal).unwrap();
        let current_height = self.height;
        if current_height == height && self.view == proposal_view {
            self.proposal = Some(proposal.hash());
            let current_step = self.step.clone();
            match current_step {
                TendermintState::Propose => {
                    self.move_to_step(Step::Prevote, false);
                }
                TendermintState::ProposeWaitImported {
                    block,
                } => {
                    if !block.transactions().is_empty() {
                        self.submit_proposal_block(&block);
                    } else {
                        ctrace!(ENGINE, "Empty proposal is generated, set timer");
                        self.step = TendermintState::ProposeWaitEmptyBlockTimer {
                            block,
                        };
                        self.extension().set_timer_empty_proposal(self.view);
                    }
                }
                TendermintState::ProposeWaitEmptyBlockTimer {
                    ..
                } => unreachable!(),
                _ => {}
            };
        } else if current_height < height {
            self.move_to_height(height);
            self.save_last_confirmed_view(proposal_view);
            self.proposal = Some(proposal.hash());
            self.move_to_step(Step::Prevote, false);
        }
    }

    fn submit_proposal_block(&mut self, sealed_block: &SealedBlock) {
        cinfo!(ENGINE, "Submitting proposal block {}", sealed_block.header().hash());
        self.move_to_step(Step::Prevote, false);
        self.broadcast_proposal_block(encoded::Block::new(sealed_block.rlp_bytes()));
    }

    fn backup(&self) {
        backup(
            self.client().get_kvdb().as_ref(),
            BackupView {
                height: &self.height,
                view: &self.view,
                step: &self.step.to_step(),
                votes: &self.votes.get_all(),
                last_confirmed_view: &self.last_confirmed_view,
            },
        );
    }

    fn restore(&mut self) {
        let client = self.client();
        let backup = restore(client.get_kvdb().as_ref());
        if let Some(backup) = backup {
            let backup_step = if backup.step == Step::Commit {
                // If the backuped step is `Commit`, we should start at `Precommit` to update the
                // chain's best block safely.
                Step::Precommit
            } else {
                backup.step
            };
            self.step = backup_step.into();
            self.height = backup.height;
            self.view = backup.view;
            self.last_confirmed_view = backup.last_confirmed_view;
            if let Some(proposal) = backup.proposal {
                if client.block_header(&BlockId::Hash(proposal)).is_some() {
                    self.proposal = Some(proposal);
                }
            }

            for vote in backup.votes {
                let bytes = rlp::encode(&vote);
                if let Err(err) = self.handle_message(&bytes, true) {
                    cinfo!(ENGINE, "Fail to load backuped message {:?}", err);
                }
            }
        }
    }

    fn seal_fields(&self, _header: &Header) -> usize {
        4
    }

    fn generate_seal(&self, block: &ExecutedBlock, parent: &Header) -> Seal {
        let header = block.header();
        let height = header.number() as Height;

        // Block is received from other nodes while creating a block
        if height < self.height {
            return Seal::None
        }

        assert_eq!(true, self.is_signer_proposer(&parent.hash()));
        assert_eq!(true, self.proposal.is_none());
        assert_eq!(true, height == self.height);

        let view = self.view;

        let last_block_hash = &self.prev_block_hash();
        let last_block_view = &self.last_confirmed_view;
        assert_eq!(last_block_hash, &parent.hash());

        let (precommits, precommit_indices) = self.votes.round_signatures_and_indices(
            &VoteStep::new(height - 1, *last_block_view, Step::Precommit),
            &last_block_hash,
        );
        ctrace!(ENGINE, "Collected seal: {:?}({:?})", precommits, precommit_indices);
        let precommit_bitset = BitSet::new_with_indices(&precommit_indices);
        Seal::Tendermint {
            prev_view: *last_block_view,
            cur_view: view,
            precommits: precommits.clone(),
            precommit_bitset,
        }
    }

    fn proposal_generated(&mut self, sealed_block: &SealedBlock) {
        let header = sealed_block.header();
        let hash = header.hash();

        if let TendermintState::ProposeWaitBlockGeneration {
            parent_hash: expected_parent_hash,
        } = self.step
        {
            assert_eq!(
                *header.parent_hash(),
                expected_parent_hash,
                "Generated hash({:?}) is different from expected({:?})",
                *header.parent_hash(),
                expected_parent_hash
            );
        } else {
            panic!("Block is generated at unexpected step {:?}", self.step);
        }

        let vote_step =
            VoteStep::new(header.number() as Height, consensus_view(&header).expect("I am proposer"), Step::Propose);
        let vote_info = message_info_rlp(vote_step, Some(hash));
        let num_validators = self.validators.count(&self.prev_block_hash());
        let signature = self.sign(blake256(&vote_info)).expect("I am proposer");
        self.votes.vote(ConsensusMessage::new_proposal(signature, num_validators, header).expect("I am proposer"));

        self.step = TendermintState::ProposeWaitImported {
            block: Box::new(sealed_block.clone()),
        };
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        let seal_length = header.seal().len();
        let expected_seal_fields = self.seal_fields(header);
        if seal_length != expected_seal_fields {
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: expected_seal_fields,
                found: seal_length,
            })
            .into())
        }

        let height = header.number() as usize;
        let view = consensus_view(header).unwrap();
        let score = Self::calculate_score(height, view);

        if *header.score() != score {
            return Err(BlockError::InvalidScore(Mismatch {
                expected: score,
                found: *header.score(),
            })
            .into())
        }

        Ok(())
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        let height = header.number() as usize;
        let view = consensus_view(header).unwrap();
        ctrace!(ENGINE, "Verify external at {}-{}, {:?}", height, view, header);
        let proposer = header.author();
        if !self.is_authority(header.parent_hash(), proposer) {
            return Err(EngineError::BlockNotAuthorized(*proposer).into())
        }
        self.check_view_proposer(header.parent_hash(), header.number() as usize, consensus_view(header)?, &proposer)
            .map_err(Error::from)?;
        let seal_view = TendermintSealView::new(header.seal());
        let bitset_count = seal_view.bitset()?.count() as usize;
        let precommits_count = seal_view.precommits().item_count()?;

        if bitset_count < precommits_count {
            cwarn!(
                ENGINE,
                "verify_block_external: The header({})'s bitset count is less than the precommits count",
                header.hash()
            );
            return Err(BlockError::InvalidSeal.into())
        }

        if bitset_count < precommits_count {
            cwarn!(
                ENGINE,
                "verify_block_external: The header({})'s bitset count is greater than the precommits count",
                header.hash()
            );
            return Err(BlockError::InvalidSeal.into())
        }

        let previous_block_view = previous_block_view(header)?;
        let step = VoteStep::new((header.number() - 1) as usize, previous_block_view, Step::Precommit);
        let precommit_hash = message_hash(step, *header.parent_hash());
        let mut counter = 0;

        for (bitset_index, signature) in seal_view.signatures()? {
            let public = self.validators.get(header.parent_hash(), bitset_index);
            if !verify_schnorr(&public, &signature, &precommit_hash)? {
                let address = public_to_address(&public);
                return Err(EngineError::BlockNotAuthorized(address.to_owned()).into())
            }
            counter += 1;
        }

        // Genesisblock does not have signatures
        if header.number() == 1 {
            return Ok(())
        }
        self.check_above_threshold(counter).map_err(Into::into)
    }

    fn signals_epoch_end(&self, header: &Header) -> EpochChange {
        let first = header.number() == 0;
        self.validators.signals_epoch_end(first, header)
    }

    fn is_epoch_end(
        &self,
        chain_head: &Header,
        _chain: &super::Headers<Header>,
        transition_store: &super::PendingTransitionStore,
    ) -> Option<Vec<u8>> {
        let first = chain_head.number() == 0;

        if let Some(change) = self.validators.is_epoch_end(first, chain_head) {
            let change = combine_proofs(chain_head.number(), &change, &[]);
            return Some(change)
        } else if let Some(pending) = transition_store(chain_head.hash()) {
            let signal_number = chain_head.number();
            let finality_proof = ::rlp::encode(chain_head);
            return Some(combine_proofs(signal_number, &pending.proof, &finality_proof))
        }

        None
    }

    fn epoch_verifier<'a>(&self, _header: &Header, proof: &'a [u8]) -> ConstructedVerifier<'a, CodeChainMachine> {
        let (signal_number, set_proof, finality_proof) = match destructure_proofs(proof) {
            Ok(x) => x,
            Err(e) => return ConstructedVerifier::Err(e),
        };

        let first = signal_number == 0;
        match self.validators.epoch_set(first, &self.machine, signal_number, set_proof) {
            Ok((list, finalize)) => {
                let verifier = Box::new(EpochVerifier {
                    subchain_validators: list,
                    recover: |signature: &SchnorrSignature, message: &Message| {
                        Ok(public_to_address(&recover_schnorr(&signature, &message)?))
                    },
                });

                match finalize {
                    Some(finalize) => ConstructedVerifier::Unconfirmed(verifier, finality_proof, finalize),
                    None => ConstructedVerifier::Trusted(verifier),
                }
            }
            Err(e) => ConstructedVerifier::Err(e),
        }
    }

    fn populate_from_parent(&self, header: &mut Header, _parent: &Header) {
        let new_score = Self::calculate_score(header.number() as usize, self.view);
        header.set_score(new_score);
    }

    fn calculate_score(height: Height, view: View) -> U256 {
        let height = U256::from(height);
        u256_from_u128(std::u128::MAX) * height - view
    }

    fn on_timeout(&mut self, token: usize) {
        // Timeout from empty block generation
        if token == ENGINE_TIMEOUT_EMPTY_PROPOSAL {
            let prev_step = mem::replace(&mut self.step, TendermintState::Propose);
            match prev_step {
                TendermintState::ProposeWaitEmptyBlockTimer {
                    block,
                } => {
                    cdebug!(ENGINE, "Empty proposal timer is finished, go to the prevote step and broadcast the block");
                    self.submit_proposal_block(block.as_ref());
                }
                _ => {
                    cwarn!(ENGINE, "Empty proposal timer was not cleared.");
                }
            }
            return
        }

        // Timeout from Tendermint step
        if self.extension().is_expired_timeout_token(token) {
            return
        }

        let next_step = match self.step {
            TendermintState::Propose => {
                cinfo!(ENGINE, "Propose timeout.");
                if self.proposal.is_none() {
                    // Report the proposer if no proposal was received.
                    let height = self.height;
                    let current_proposer = self.view_proposer(&self.prev_block_hash(), height, self.view);
                    self.validators.report_benign(&current_proposer, height as BlockNumber, height as BlockNumber);
                }
                Some(Step::Prevote)
            }
            TendermintState::ProposeWaitBlockGeneration {
                ..
            } => {
                cwarn!(ENGINE, "Propose timed out but block is not generated yet");
                None
            }
            TendermintState::ProposeWaitImported {
                ..
            } => {
                cwarn!(ENGINE, "Propose timed out but still waiting for the block imported");
                None
            }
            TendermintState::ProposeWaitEmptyBlockTimer {
                ..
            } => {
                cwarn!(ENGINE, "Propose timed out but still waiting for the empty block");
                None
            }
            TendermintState::Prevote if self.has_enough_any_votes() => {
                cinfo!(ENGINE, "Prevote timeout.");
                Some(Step::Precommit)
            }
            TendermintState::Prevote => {
                cinfo!(ENGINE, "Prevote timeout without enough votes.");
                Some(Step::Prevote)
            }
            TendermintState::Precommit if self.has_enough_any_votes() => {
                cinfo!(ENGINE, "Precommit timeout.");
                self.increment_view(1);
                Some(Step::Propose)
            }
            TendermintState::Precommit => {
                cinfo!(ENGINE, "Precommit timeout without enough votes.");
                Some(Step::Precommit)
            }
            TendermintState::Commit => {
                cinfo!(ENGINE, "Commit timeout.");
                assert!(
                    self.check_current_block_exists(),
                    "The canonical chain must have the block of the previous height"
                );
                let height = self.height;
                self.move_to_height(height + 1);
                Some(Step::Propose)
            }
        };

        if let Some(next_step) = next_step {
            self.move_to_step(next_step, false);
        }
    }

    fn on_new_block(&self, block: &mut ExecutedBlock, epoch_begin: bool) -> Result<(), Error> {
        if !epoch_begin {
            return Ok(())
        }

        // genesis is never a new block, but might as well check.
        let header = block.header().clone();
        let first = header.number() == 0;

        self.validators.on_epoch_begin(first, &header)
    }

    fn on_close_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        let author = *block.header().author();
        let transactions = block.transactions().to_owned().into_iter();
        let fee = transactions.map(|tx| tx.fee).sum();
        let stakes = stake::get_stakes(block.state()).expect("Cannot get Stake status");
        for (address, share) in stake::fee_distribute(&author, fee, &stakes) {
            self.machine.add_balance(block, &address, share)?
        }
        Ok(())
    }

    fn register_client(&mut self, client: Weak<EngineClient>) {
        self.last_confirmed_view = 0;
        self.client = Some(Weak::clone(&client));
        self.validators.register_client(Weak::clone(&client));
        self.chain_notify.register_client(client);
    }

    fn handle_message(&mut self, rlp: &[u8], is_restoring: bool) -> Result<(), EngineError> {
        fn fmt_err<T: ::std::fmt::Debug>(x: T) -> EngineError {
            EngineError::MalformedMessage(format!("{:?}", x))
        }

        let rlp = UntrustedRlp::new(rlp);
        let message: ConsensusMessage = rlp.as_val().map_err(fmt_err)?;
        if !self.votes.is_old_or_known(&message) {
            let signer_index = message.signer_index;
            let prev_height = (message.on.step.height - 1) as u64;
            if message.on.step.height > self.height {
                // Because the members of the committee could change in future height, we could not verify future height's message.
                return Err(EngineError::FutureMessage {
                    future_height: message.on.step.height as u64,
                    current_height: self.height as u64,
                })
            }

            let prev_block_hash = self
                .client()
                .block_header(&BlockId::Number((message.on.step.height as u64) - 1))
                .expect("self.height - 1 == the best block number")
                .hash();

            if signer_index >= self.validators.count(&prev_block_hash) {
                return Err(EngineError::ValidatorNotExist {
                    height: prev_height,
                    index: signer_index,
                })
            }

            let sender_public = self.validators.get(&prev_block_hash, signer_index);

            if !message.verify(&sender_public).map_err(fmt_err)? {
                return Err(EngineError::MessageWithInvalidSignature {
                    height: prev_height,
                    signer_index,
                    address: public_to_address(&sender_public),
                })
            }

            let sender = public_to_address(&sender_public);

            if message.on.step > self.vote_step() {
                ctrace!(ENGINE, "Ignore future message {:?} from {}.", message, sender);
                return Ok(())
            }

            let current_vote_step = if self.step.is_commit() {
                // Even in the commit step, it must be possible to get pre-commits from
                // the previous step. So, act as the last precommit step.
                VoteStep {
                    height: self.height,
                    view: self.last_confirmed_view,
                    step: Step::Precommit,
                }
            } else {
                self.vote_step()
            };

            if message.on.step == current_vote_step {
                let vote_index = self
                    .validators
                    .get_index(&self.prev_block_hash(), &sender_public)
                    .expect("is_authority already checked the existence");
                self.votes_received.set(vote_index);
            }

            if let Some(double) = self.votes.vote(message.clone()) {
                let height = message.on.step.height as BlockNumber;
                cwarn!(ENGINE, "Double vote found {:?}", double);
                self.validators.report_malicious(&sender, height, height, ::rlp::encode(&double).into_vec());
                return Err(EngineError::DoubleVote(sender))
            }
            ctrace!(ENGINE, "Handling a valid {:?} from {}.", message, sender);
            self.handle_valid_message(&message, is_restoring);
        }
        Ok(())
    }

    fn is_proposal(&self, header: &Header) -> bool {
        let number = header.number();
        if self.height > number as usize {
            return false
        }

        // if next header is imported, current header is not a proposal
        if self
            .client()
            .block_header(&BlockId::Number(number + 1))
            .map_or(false, |next| next.parent_hash() == header.hash())
        {
            return false
        }

        !self.has_enough_precommit_votes(header.hash())
    }

    fn broadcast_proposal_block(&self, block: encoded::Block) {
        let header = block.decode_header();
        let hash = header.hash();
        let parent_hash = header.parent_hash();
        let vote_step =
            VoteStep::new(header.number() as Height, consensus_view(&header).expect("Already verified"), Step::Propose);
        cdebug!(ENGINE, "Send proposal {:?}", vote_step);

        if self.is_signer_proposer(&parent_hash) {
            let vote_info = message_info_rlp(vote_step, Some(hash));
            let signature = self.sign(blake256(&vote_info)).expect("I am proposer");
            self.extension().broadcast_proposal_block(signature, block.into_inner());
        } else if let Some(signature) = self.votes.round_signature(&vote_step, &hash) {
            self.extension().broadcast_proposal_block(signature, block.into_inner());
        } else {
            cwarn!(ENGINE, "There is a proposal but does not have signature {:?}", vote_step);
        }
    }

    fn set_signer(&mut self, ap: Arc<AccountProvider>, address: Address) {
        self.signer.set_to_keep_decrypted_account(ap, address);
    }

    fn sign(&self, hash: H256) -> Result<SchnorrSignature, Error> {
        self.signer.sign(hash).map_err(Into::into)
    }

    fn signer_index(&self, bh: &H256) -> Option<usize> {
        // FIXME: More effecient way to find index
        self.signer.public().and_then(|public| self.validators.get_index(bh, public))
    }

    fn register_network_extension_to_service(&mut self, service: &NetworkService) {
        let extension = {
            let timeouts = self.timeouts.clone();
            let tendermint = Arc::downgrade(&self.engine());
            let client = Arc::downgrade(&self.client());
            service.new_extension(|api| TendermintExtension::new(tendermint, client, timeouts, api))
        };

        self.extension = Some(Arc::clone(&extension));
        self.restore();
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        self.block_reward
    }

    fn register_chain_notify(&self, client: &Client) {
        client.add_notify(Arc::downgrade(&self.chain_notify) as Weak<ChainNotify>);
    }
}

impl ConsensusEngine<CodeChainMachine> for Tendermint {
    fn name(&self) -> &str {
        "Tendermint"
    }

    fn machine(&self) -> &CodeChainMachine {
        &self.machine.as_ref()
    }

    /// (consensus view, proposal signature, authority signatures)
    fn seal_fields(&self, header: &Header) -> usize {
        let guard = self.inner.lock();
        guard.seal_fields(header)
    }

    /// Should this node participate.
    fn seals_internally(&self) -> Option<bool> {
        let guard = self.inner.lock();
        let has_signer = guard.signer.is_some();
        Some(has_signer)
    }

    fn engine_type(&self) -> EngineType {
        EngineType::PBFT
    }

    /// Attempt to seal generate a proposal seal.
    ///
    /// This operation is synchronous and may (quite reasonably) not be available, in which case
    /// `Seal::None` will be returned.
    fn generate_seal(&self, block: &ExecutedBlock, parent: &Header) -> Seal {
        let guard = self.inner.lock();
        guard.generate_seal(block, parent)
    }

    /// Called when the node is the leader and a proposal block is generated from the miner.
    /// This writes the proposal information and go to the prevote step.
    fn proposal_generated(&self, sealed_block: &SealedBlock) {
        let mut guard = self.inner.lock();
        guard.proposal_generated(sealed_block);
    }

    fn verify_local_seal(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        let guard = self.inner.lock();
        guard.verify_block_basic(header)
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        let guard = self.inner.lock();
        guard.verify_block_external(header)
    }

    fn signals_epoch_end(&self, header: &Header) -> EpochChange {
        let guard = self.inner.lock();
        guard.signals_epoch_end(header)
    }

    fn is_epoch_end(
        &self,
        chain_head: &Header,
        chain: &super::Headers<Header>,
        transition_store: &super::PendingTransitionStore,
    ) -> Option<Vec<u8>> {
        let guard = self.inner.lock();
        guard.is_epoch_end(chain_head, chain, transition_store)
    }

    fn epoch_verifier<'a>(&self, header: &Header, proof: &'a [u8]) -> ConstructedVerifier<'a, CodeChainMachine> {
        let guard = self.inner.lock();
        guard.epoch_verifier(header, proof)
    }

    fn populate_from_parent(&self, header: &mut Header, parent: &Header) {
        let guard = self.inner.lock();
        guard.populate_from_parent(header, parent);
    }

    /// Equivalent to a timeout: to be used for tests.
    fn on_timeout(&self, token: usize) {
        let mut guard = self.inner.lock();
        guard.on_timeout(token)
    }

    fn stop(&self) {}

    fn on_new_block(&self, block: &mut ExecutedBlock, epoch_begin: bool) -> Result<(), Error> {
        let guard = self.inner.lock();
        guard.on_new_block(block, epoch_begin)
    }

    fn on_close_block(&self, block: &mut ExecutedBlock) -> Result<(), Error> {
        let guard = self.inner.lock();
        guard.on_close_block(block)
    }

    fn register_client(&self, client: Weak<EngineClient>) {
        let mut guard = self.inner.lock();
        guard.register_client(client);
    }

    fn handle_message(&self, rlp: &[u8]) -> Result<(), EngineError> {
        let mut guard = self.inner.lock();
        guard.handle_message(rlp, false)
    }

    fn is_proposal(&self, header: &Header) -> bool {
        let guard = self.inner.lock();
        guard.is_proposal(header)
    }

    fn set_signer(&self, ap: Arc<AccountProvider>, address: Address) {
        let mut guard = self.inner.lock();
        guard.set_signer(ap, address)
    }

    fn register_network_extension_to_service(&self, service: &NetworkService) {
        let mut guard = self.inner.lock();
        guard.register_network_extension_to_service(service)
    }

    fn block_reward(&self, _block_number: u64) -> u64 {
        let guard = self.inner.lock();
        guard.block_reward(_block_number)
    }

    fn recommended_confirmation(&self) -> u32 {
        1
    }

    fn register_chain_notify(&self, client: &Client) {
        let guard = self.inner.lock();
        guard.register_chain_notify(client);
    }

    fn get_best_block_from_best_proposal_header(&self, header: &HeaderView) -> H256 {
        header.parent_hash()
    }

    fn can_change_canon_chain(&self, header: &HeaderView) -> bool {
        let guard = self.inner.lock();
        let allowed_height = if guard.step.is_commit() {
            guard.height + 1
        } else {
            guard.height
        };
        header.number() >= allowed_height as u64
    }

    fn action_handlers(&self) -> &[Arc<ActionHandler>] {
        &self.action_handlers
    }
}

struct TendermintChainNotify {
    tendermint: RwLock<Option<Weak<Tendermint>>>,
    client: RwLock<Option<Weak<EngineClient>>>,
}

impl TendermintChainNotify {
    fn new() -> Self {
        Self {
            tendermint: RwLock::new(None),
            client: RwLock::new(None),
        }
    }

    fn register_client(&self, client: Weak<EngineClient>) {
        *self.client.write() = Some(client);
    }

    fn register_tendermint(&self, tendermint: Weak<Tendermint>) {
        *self.tendermint.write() = Some(tendermint);
    }
}

impl ChainNotify for TendermintChainNotify {
    /// fires when chain has new blocks.
    fn new_blocks(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
        let c = match self.client.read().as_ref().and_then(|weak| weak.upgrade()) {
            Some(c) => c,
            None => return,
        };

        let t = match self.tendermint.read().as_ref().and_then(|weak| weak.upgrade()) {
            Some(t) => t,
            None => return,
        };

        let mut t = t.inner.lock();
        if !imported.is_empty() {
            let mut height_changed = false;
            for hash in imported {
                // New Commit received, skip to next height.
                let header = c.block_header(&hash.into()).expect("ChainNotify is called after the block is imported");

                let full_header = header.decode();
                if t.is_proposal(&full_header) {
                    t.on_imported_proposal(&full_header);
                } else if t.height < header.number() as usize {
                    height_changed = true;
                    cinfo!(ENGINE, "Received a commit: {:?}.", header.number());
                    let view = consensus_view(&full_header).expect("Imported block already checked");
                    t.move_to_height(header.number() as usize);
                    t.save_last_confirmed_view(view);
                }
            }
            if height_changed {
                t.move_to_step(Step::Commit, false);
                return
            }
        }
        if !enacted.is_empty() && t.can_move_from_commit_to_propose() {
            cinfo!(
                ENGINE,
                "Transition to Propose because all pre-commits are received and the canonical chain is appended"
            );
            let new_height = t.height + 1;
            t.move_to_height(new_height);
            t.move_to_step(Step::Propose, false)
        }
    }
}

struct EpochVerifier<F>
where
    F: Fn(&SchnorrSignature, &Message) -> Result<Address, Error> + Send + Sync, {
    subchain_validators: ValidatorList,
    recover: F,
}

impl<F> super::EpochVerifier<CodeChainMachine> for EpochVerifier<F>
where
    F: Fn(&SchnorrSignature, &Message) -> Result<Address, Error> + Send + Sync,
{
    fn verify_light(&self, header: &Header) -> Result<(), Error> {
        let message = header.hash();

        let mut addresses = HashSet::new();
        let header_precommits_field = &header.seal().get(2).ok_or(BlockError::InvalidSeal)?;
        for rlp in UntrustedRlp::new(header_precommits_field).iter() {
            let signature: SchnorrSignature = rlp.as_val()?;
            let address = (self.recover)(&signature, &message)?;

            if !self.subchain_validators.contains_address(header.parent_hash(), &address) {
                return Err(EngineError::BlockNotAuthorized(address.to_owned()).into())
            }
            addresses.insert(address);
        }

        let n = addresses.len();
        let threshold = self.subchain_validators.len() * 2 / 3;
        if n > threshold {
            Ok(())
        } else {
            Err(EngineError::BadSealFieldSize(OutOfBounds {
                min: Some(threshold),
                max: None,
                found: n,
            })
            .into())
        }
    }

    fn check_finality_proof(&self, proof: &[u8]) -> Option<Vec<H256>> {
        let header: Header = ::rlp::decode(proof);
        self.verify_light(&header).ok().map(|_| vec![header.hash()])
    }
}

fn combine_proofs(signal_number: BlockNumber, set_proof: &[u8], finality_proof: &[u8]) -> Vec<u8> {
    let mut stream = ::rlp::RlpStream::new_list(3);
    stream.append(&signal_number).append(&set_proof).append(&finality_proof);
    stream.out()
}

fn destructure_proofs(combined: &[u8]) -> Result<(BlockNumber, &[u8], &[u8]), Error> {
    let rlp = UntrustedRlp::new(combined);
    Ok((rlp.at(0)?.as_val()?, rlp.at(1)?.data()?, rlp.at(2)?.data()?))
}

struct TendermintExtension {
    tendermint: Weak<Tendermint>,
    client: Weak<EngineClient>,
    peers: RwLock<HashMap<NodeId, PeerState>>,
    api: Arc<Api>,
    timeouts: TimeoutParams,
    timeout_token_nonce: AtomicUsize,
}

const MIN_PEERS_PROPAGATION: usize = 4;
const MAX_PEERS_PROPAGATION: usize = 128;

impl TendermintExtension {
    fn new(tendermint: Weak<Tendermint>, client: Weak<EngineClient>, timeouts: TimeoutParams, api: Arc<Api>) -> Self {
        Self {
            tendermint,
            client,
            peers: RwLock::new(HashMap::new()),
            api,
            timeouts,
            timeout_token_nonce: AtomicUsize::new(ENGINE_TIMEOUT_TOKEN_NONCE_BASE),
        }
    }

    fn update_peer_state(&self, token: &NodeId, vote_step: VoteStep, proposal: Option<H256>, messages: BitSet) {
        let mut peers_guard = self.peers.write();
        let peer_state = match peers_guard.get_mut(token) {
            Some(peer_state) => peer_state,
            // update_peer_state could be called after the peer is disconnected
            None => return,
        };
        peer_state.vote_step = vote_step;
        peer_state.proposal = proposal;
        peer_state.messages = messages;
    }

    fn select_random_peers(&self) -> Vec<NodeId> {
        let mut peers: Vec<NodeId> = self.peers.write().keys().cloned().collect();
        let mut count = (peers.len() as f64).powf(0.5).round() as usize;
        count = cmp::min(count, MAX_PEERS_PROPAGATION);
        count = cmp::max(count, MIN_PEERS_PROPAGATION);
        peers.shuffle(&mut thread_rng());
        peers.truncate(count);
        peers
    }

    fn broadcast_message(&self, message: Bytes) {
        let tokens = self.select_random_peers();
        let message = TendermintMessage::ConsensusMessage(message).rlp_bytes().into_vec();
        for token in tokens {
            self.api.send(&token, &message);
        }
    }

    fn send_message(&self, token: &NodeId, message: Bytes) {
        ctrace!(ENGINE, "Send message to {}", token);
        let message = TendermintMessage::ConsensusMessage(message).rlp_bytes().into_vec();
        self.api.send(token, &message);
    }

    fn broadcast_state(&self, vote_step: &VoteStep, proposal: Option<H256>, lock_view: Option<View>, votes: BitSet) {
        ctrace!(ENGINE, "Broadcast state {:?} {:?} {:?}", vote_step, proposal, votes);
        let tokens = self.select_random_peers();
        let message = TendermintMessage::StepState {
            vote_step: *vote_step,
            proposal,
            lock_view,
            known_votes: votes,
        }
        .rlp_bytes()
        .into_vec();

        for token in tokens {
            self.api.send(&token, &message);
        }
    }

    fn send_proposal_block(&self, token: &NodeId, signature: SchnorrSignature, message: Bytes) {
        let message = TendermintMessage::ProposalBlock {
            signature,
            message,
        }
        .rlp_bytes()
        .into_vec();
        self.api.send(token, &message);
    }

    fn broadcast_proposal_block(&self, signature: SchnorrSignature, message: Bytes) {
        let message = TendermintMessage::ProposalBlock {
            signature,
            message,
        }
        .rlp_bytes()
        .into_vec();
        for token in self.peers.read().keys() {
            self.api.send(&token, &message);
        }
    }

    fn request_proposal_to_any(&self, height: Height, view: View) {
        let peers_guard = self.peers.read();
        for (token, peer) in peers_guard.iter() {
            let is_future_height_and_view = {
                let higher_height = peer.vote_step.height > height;
                let same_height_and_higher_view = peer.vote_step.height == height && peer.vote_step.view > view;
                higher_height || same_height_and_higher_view
            };

            if is_future_height_and_view {
                self.request_proposal(token, height, view);
                continue
            }

            let is_same_height_and_view = peer.vote_step.height == height && peer.vote_step.view == view;

            if is_same_height_and_view && peer.proposal.is_some() {
                self.request_proposal(token, height, view);
            }
        }
    }

    fn request_proposal(&self, token: &NodeId, height: Height, view: View) {
        ctrace!(ENGINE, "Request proposal {} {} to {:?}", height, view, token);
        let message = TendermintMessage::RequestProposal {
            height,
            view,
        }
        .rlp_bytes()
        .into_vec();
        self.api.send(&token, &message);
    }

    fn request_all_votes(&self, vote_step: &VoteStep) {
        let peers_guard = self.peers.read();
        for (token, peer) in peers_guard.iter() {
            if *vote_step <= peer.vote_step && !peer.messages.is_empty() {
                // FIXME: Do not need to request already known votes
                self.request_messages(token, *vote_step, BitSet::all_set());
            }
        }
    }

    fn request_messages(&self, token: &NodeId, vote_step: VoteStep, requested_votes: BitSet) {
        ctrace!(ENGINE, "Request messages {:?} {:?} to {:?}", vote_step, requested_votes, token);
        let message = TendermintMessage::RequestMessage {
            vote_step,
            requested_votes,
        }
        .rlp_bytes()
        .into_vec();
        self.api.send(&token, &message);
    }

    fn is_expired_timeout_token(&self, nonce: usize) -> bool {
        nonce < self.timeout_token_nonce.load(AtomicOrdering::SeqCst)
    }

    fn set_timer_step(&self, step: Step, view: View) {
        let expired_token_nonce = self.timeout_token_nonce.fetch_add(1, AtomicOrdering::SeqCst);

        self.api.clear_timer(ENGINE_TIMEOUT_EMPTY_PROPOSAL).expect("Timer clear succeeds");
        self.api.clear_timer(expired_token_nonce).expect("Timer clear succeeds");
        self.api
            .set_timer_once(expired_token_nonce + 1, self.timeouts.timeout(step, view))
            .expect("Timer set succeeds");
    }

    fn set_timer_empty_proposal(&self, view: View) {
        self.api.clear_timer(ENGINE_TIMEOUT_EMPTY_PROPOSAL).expect("Timer clear succeeds");
        self.api
            .set_timer_once(ENGINE_TIMEOUT_EMPTY_PROPOSAL, self.timeouts.timeout(Step::Propose, view) / 2)
            .expect("Timer set succeeds");
    }

    fn on_proposal_message(&self, tendermint: MutexGuard<TendermintInner>, signature: SchnorrSignature, bytes: Bytes) {
        let c = match self.client.upgrade() {
            Some(c) => c,
            None => return,
        };

        // This block borrows bytes
        {
            let block_view = BlockView::new(&bytes);
            let header_view = block_view.header();
            let number = header_view.number();
            cinfo!(ENGINE, "Proposal received for {}-{:?}", number, header_view.hash());

            let parent_hash = header_view.parent_hash();
            if c.block(&BlockId::Hash(*parent_hash)).is_none() {
                let best_block_number = c.best_block_header().number();
                ctrace!(
                    ENGINE,
                    "Received future proposal {}-{}, current best block number is {}. ignore it",
                    number,
                    parent_hash,
                    best_block_number
                );
                return
            }

            let num_validators = tendermint.validators.count(&parent_hash);
            let message = match ConsensusMessage::new_proposal(signature, num_validators, &header_view) {
                Ok(message) => message,
                Err(err) => {
                    cdebug!(ENGINE, "Invalid proposal received: {:?}", err);
                    return
                }
            };

            // If the proposal's height is current height + 1 and the proposal has valid precommits,
            // we should import it and increase height
            if number > (tendermint.height + 1) as u64 {
                ctrace!(ENGINE, "Received future proposal, ignore it");
                return
            }

            if number == tendermint.height as u64 && message.on.step.view > tendermint.view {
                ctrace!(ENGINE, "Received future proposal, ignore it");
                return
            }

            let signer_public = tendermint.validators.get(&parent_hash, message.signer_index);
            let signer = public_to_address(&signer_public);

            match message.verify(&signer_public) {
                Ok(false) => {
                    cwarn!(ENGINE, "Proposal verification failed: signer is different");
                    return
                }
                Err(err) => {
                    cwarn!(ENGINE, "Proposal verification failed: {:?}", err);
                    return
                }
                _ => {}
            }

            if *header_view.author() != signer {
                cwarn!(ENGINE, "Proposal author({}) not matched with header({})", signer, header_view.author());
                return
            }

            if tendermint.votes.is_old_or_known(&message) {
                cdebug!(ENGINE, "Proposal is already known");
                return
            }

            tendermint.votes.vote(message);
        }

        drop(tendermint);
        if let Err(e) = c.import_block(bytes) {
            cinfo!(ENGINE, "Failed to import proposal block {:?}", e);
        }
    }

    fn on_step_state_message(
        &self,
        tendermint: &TendermintInner,
        token: &NodeId,
        peer_vote_step: VoteStep,
        peer_proposal: Option<H256>,
        peer_lock_view: Option<View>,
        peer_known_votes: BitSet,
    ) {
        ctrace!(
            ENGINE,
            "Peer state update step: {:?} proposal {:?} peer_lock_view {:?} known_votes {:?}",
            peer_vote_step,
            peer_proposal,
            peer_lock_view,
            peer_known_votes,
        );
        self.update_peer_state(token, peer_vote_step, peer_proposal, peer_known_votes);

        let current_vote_step = if tendermint.step.is_commit() {
            // Even in the commit step, it must be possible to get pre-commits from
            // the previous step. So, act as the last precommit step.
            VoteStep {
                height: tendermint.height,
                view: tendermint.last_confirmed_view,
                step: Step::Precommit,
            }
        } else {
            tendermint.vote_step()
        };

        if tendermint.height > peer_vote_step.height {
            // no messages to receive
            return
        }

        // Since the peer may not have a proposal in its height,
        // we may need to receive votes instead of block.
        if tendermint.height + 2 < peer_vote_step.height {
            // need to get blocks from block sync
            return
        }

        let peer_has_proposal = (tendermint.view == peer_vote_step.view && peer_proposal.is_some())
            || tendermint.view < peer_vote_step.view
            || tendermint.height < peer_vote_step.height;

        let need_proposal = tendermint.need_proposal();
        if need_proposal && peer_has_proposal {
            self.request_proposal(token, tendermint.height, tendermint.view);
        }

        let current_step = current_vote_step.step;
        if current_step == Step::Prevote || current_step == Step::Precommit {
            let peer_known_votes = if current_vote_step == peer_vote_step {
                peer_known_votes
            } else if current_vote_step < peer_vote_step {
                // We don't know which votes peer has.
                // However the peer knows more than 2/3 of votes.
                // So request all votes.
                BitSet::all_set()
            } else {
                // If peer's state is less than my state,
                // the peer does not know any useful votes.
                BitSet::new()
            };

            let current_votes = tendermint.votes_received;
            let difference = &peer_known_votes - &current_votes;
            if !difference.is_empty() {
                self.request_messages(token, current_vote_step, difference);
            }
        }

        if peer_vote_step.height == tendermint.height {
            match (tendermint.last_lock, peer_lock_view) {
                (None, Some(peer_lock_view)) if peer_lock_view < tendermint.view => {
                    ctrace!(
                        ENGINE,
                        "Peer has a lock on {}-{} but I don't have it",
                        peer_vote_step.height,
                        peer_lock_view
                    );
                    self.request_messages(
                        token,
                        VoteStep {
                            height: tendermint.height,
                            view: peer_lock_view,
                            step: Step::Prevote,
                        },
                        BitSet::all_set(),
                    );
                }
                (Some(my_lock_view), Some(peer_lock_view))
                    if my_lock_view < peer_lock_view && peer_lock_view < tendermint.view =>
                {
                    ctrace!(
                        ENGINE,
                        "Peer has a lock on {}-{} which is newer than mine {}",
                        peer_vote_step.height,
                        peer_lock_view,
                        my_lock_view
                    );
                    self.request_messages(
                        token,
                        VoteStep {
                            height: tendermint.height,
                            view: peer_lock_view,
                            step: Step::Prevote,
                        },
                        BitSet::all_set(),
                    );
                }
                _ => {
                    // Do nothing
                }
            }
        }
    }

    fn on_request_proposal_message(
        &self,
        tendermint: &TendermintInner,
        token: &NodeId,
        request_height: Height,
        request_view: View,
    ) {
        ctrace!(ENGINE, "Received RequestProposal for {}-{} from {:?}", request_height, request_view, token);
        if request_height > tendermint.height {
            return
        }

        if request_height == tendermint.height && request_view > tendermint.view {
            return
        }

        if let Some((signature, _signer_index, block)) = tendermint.proposal_at(request_height, request_view) {
            ctrace!(ENGINE, "Send proposal {}-{} to {:?}", request_height, request_view, token);
            self.send_proposal_block(token, signature, block);
        }
    }
}

impl NetworkExtension for TendermintExtension {
    fn name(&self) -> &'static str {
        "tendermint"
    }

    fn need_encryption(&self) -> bool {
        false
    }

    fn versions(&self) -> &[u64] {
        const VERSIONS: &[u64] = &[0];
        &VERSIONS
    }

    fn on_initialize(&self) {
        let initial = self.timeouts.initial();
        ctrace!(ENGINE, "Setting the initial timeout to {}.", initial);
        self.api.set_timer_once(ENGINE_TIMEOUT_TOKEN_NONCE_BASE, initial).expect("Timer set succeeds");
    }

    fn on_node_added(&self, token: &NodeId, _version: u64) {
        self.peers.write().insert(*token, PeerState::new());
    }

    fn on_node_removed(&self, token: &NodeId) {
        self.peers.write().remove(token);
    }

    fn on_message(&self, token: &NodeId, data: &[u8]) {
        let t = match self.tendermint.upgrade() {
            Some(t) => t,
            None => return,
        };
        let mut t = t.inner.lock();

        let m = UntrustedRlp::new(data);
        match m.as_val() {
            Ok(TendermintMessage::ConsensusMessage(ref bytes)) => {
                match t.handle_message(bytes, false) {
                    Err(EngineError::FutureMessage {
                        future_height,
                        current_height,
                    }) => {
                        cdebug!(
                            ENGINE,
                            "Could not handle future message from {}, in height {}",
                            future_height,
                            current_height
                        );
                    }
                    Err(e) => {
                        cinfo!(ENGINE, "Failed to handle message {:?}", e);
                    }
                    Ok(_) => {}
                }

                if let Err(e) = t.handle_message(bytes, false) {
                    cinfo!(ENGINE, "Failed to handle message {:?}", e);
                }
            }
            Ok(TendermintMessage::ProposalBlock {
                signature,
                message,
            }) => {
                self.on_proposal_message(t, signature, message);
            }
            Ok(TendermintMessage::StepState {
                vote_step,
                proposal,
                lock_view,
                known_votes,
            }) => {
                self.on_step_state_message(&t, token, vote_step, proposal, lock_view, known_votes);
            }
            Ok(TendermintMessage::RequestProposal {
                height,
                view,
            }) => {
                self.on_request_proposal_message(&t, token, height, view);
            }
            Ok(TendermintMessage::RequestMessage {
                vote_step: request_vote_step,
                requested_votes,
            }) => {
                ctrace!(ENGINE, "Received RequestMessage for {:?} from {:?}", request_vote_step, requested_votes);

                let all_votes = t.get_all_votes_and_authors(&request_vote_step, &requested_votes);
                for vote in all_votes {
                    self.send_message(token, vote.rlp_bytes().into_vec());
                }
            }
            _ => cinfo!(ENGINE, "Invalid message from peer {}", token),
        }
    }
}

impl TimeoutHandler for TendermintExtension {
    fn on_timeout(&self, token: TimerToken) {
        debug_assert!(token >= ENGINE_TIMEOUT_TOKEN_NONCE_BASE || token == ENGINE_TIMEOUT_EMPTY_PROPOSAL);
        if let Some(c) = self.tendermint.upgrade() {
            c.on_timeout(token);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::block::{ClosedBlock, IsBlock, OpenBlock};
    use crate::client::TestBlockChainClient;
    use crate::consensus::CodeChainEngine;
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    use super::*;

    /// Accounts inserted with "0" and "1" are validators. First proposer is "0".
    fn setup() -> (Scheme, Arc<AccountProvider>, Arc<EngineClient>) {
        let tap = AccountProvider::transient_provider();
        let scheme = Scheme::new_test_tendermint();
        let test_client: Arc<EngineClient> =
            Arc::new(TestBlockChainClient::new_with_scheme(Scheme::new_test_tendermint()));
        scheme.engine.register_client(Arc::downgrade(&test_client));
        (scheme, tap, test_client)
    }

    fn propose_default(scheme: &Scheme, proposer: Address) -> (ClosedBlock, Vec<Bytes>) {
        let db = get_temp_state_db();
        let db = scheme.ensure_genesis_state(db).unwrap();
        let genesis_header = scheme.genesis_header();
        let b = OpenBlock::try_new(scheme.engine.as_ref(), db, &genesis_header, proposer, vec![], false).unwrap();
        let b = b.close(*genesis_header.transactions_root(), *genesis_header.results_root()).unwrap();
        if let Some(seal) = scheme.engine.generate_seal(b.block(), &genesis_header).seal_fields() {
            (b, seal)
        } else {
            panic!()
        }
    }

    fn insert_and_unlock(tap: &Arc<AccountProvider>, acc: &str) -> Address {
        let addr = tap.insert_account(blake256(acc).into(), &acc.into()).unwrap();
        tap.unlock_account_permanently(addr, acc.into()).unwrap();
        addr
    }

    fn insert_and_register(tap: &Arc<AccountProvider>, engine: &CodeChainEngine, acc: &str) -> Address {
        let addr = insert_and_unlock(tap, acc);
        engine.set_signer(tap.clone(), addr);
        addr
    }

    #[test]
    fn has_valid_metadata() {
        let engine = Scheme::new_test_tendermint().engine;
        assert!(!engine.name().is_empty());
    }

    #[test]
    fn verification_fails_on_short_seal() {
        let engine = Scheme::new_test_tendermint().engine;
        let header = Header::default();

        let verify_result = engine.verify_block_basic(&header);

        match verify_result {
            Err(Error::Block(BlockError::InvalidSealArity(_))) => {}
            Err(err) => {
                panic!("should be block seal-arity mismatch error (got {:?})", err);
            }
            _ => {
                panic!("Should be error, got Ok");
            }
        }
    }

    #[test]
    fn generate_seal() {
        let (scheme, tap, _c) = setup();

        let proposer = insert_and_register(&tap, scheme.engine.as_ref(), "1");

        let (b, seal) = propose_default(&scheme, proposer);
        assert!(b.lock().try_seal(scheme.engine.as_ref(), seal).is_ok());
    }

    #[test]
    fn seal_signatures_checking() {
        let (spec, tap, _c) = setup();
        let engine = spec.engine;

        let mut header = Header::default();
        header.set_number(4);
        let proposer = insert_and_unlock(&tap, "0");
        header.set_author(proposer);
        header.set_parent_hash(Default::default());

        let vote_info = message_info_rlp(VoteStep::new(3, 0, Step::Precommit), Some(*header.parent_hash()));
        let signature0 = tap.get_account(&proposer, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0],
            precommit_bitset: BitSet::new_with_indices(&[0]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        // One good signature is not enough.
        match engine.verify_block_external(&header) {
            Err(Error::Engine(EngineError::BadSealFieldSize(_))) => {}
            _ => panic!(),
        }

        let voter = insert_and_unlock(&tap, "1");
        let signature1 = tap.get_account(&voter, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();
        let voter = insert_and_unlock(&tap, "2");
        let signature2 = tap.get_account(&voter, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0, signature1, signature2],
            precommit_bitset: BitSet::new_with_indices(&[0, 1, 2]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        assert!(engine.verify_block_external(&header).is_ok());

        let bad_voter = insert_and_unlock(&tap, "101");
        let bad_signature = tap.get_account(&bad_voter, None).unwrap().sign_schnorr(&blake256(vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0, signature1, bad_signature],
            precommit_bitset: BitSet::new_with_indices(&[0, 1, 2]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        // Two good and one bad signature.
        match engine.verify_block_external(&header) {
            Err(Error::Engine(EngineError::BlockNotAuthorized(_))) => {}
            _ => panic!(),
        };
        engine.stop();
    }
}
