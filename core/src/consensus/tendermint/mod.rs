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

mod message;
mod params;
pub mod types;

use std::cmp;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};

use ccrypto::blake256;
use ckey::{public_to_address, recover, Address, Message, Password, Signature};
use cnetwork::{Api, NetworkExtension, NetworkService, NodeId, TimeoutHandler, TimerToken};
use ctypes::machine::WithBalances;
use ctypes::util::unexpected::{Mismatch, OutOfBounds};
use ctypes::BlockNumber;
use parking_lot::{Mutex, ReentrantMutex, RwLock};
use primitives::{u256_from_u128, Bytes, H256, U256};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use self::message::*;
pub use self::params::{TendermintParams, TendermintTimeouts};
use self::types::{Height, Step, View};
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
pub const ENGINE_TIMEOUT_TOKEN: TimerToken = 23;

pub type BlockHash = H256;

/// ConsensusEngine using `Tendermint` consensus algorithm
pub struct Tendermint {
    client: RwLock<Option<Weak<EngineClient>>>,
    /// Blockchain height.
    height: AtomicUsize,
    /// Consensus view.
    view: AtomicUsize,
    /// Consensus step.
    step: RwLock<Step>,
    /// Vote accumulator.
    votes: VoteCollector<ConsensusMessage>,
    /// Used to sign messages and proposals.
    signer: RwLock<EngineSigner>,
    /// Message for the last PoLC.
    lock_change: RwLock<Option<ConsensusMessage>>,
    /// Last lock view.
    last_lock: AtomicUsize,
    /// hash of the proposed block, used for seal submission.
    proposal: RwLock<Option<H256>>,
    /// Hash of the proposal parent block.
    proposal_parent: RwLock<H256>,
    /// Proposal block which parent is not imported yet
    future_proposal: RwLock<Option<Bytes>>,
    last_seal: RwLock<(H256, View, Vec<Signature>)>,
    /// Set used to determine the current validators.
    validators: Box<ValidatorSet>,
    /// Reward per block, in base units.
    block_reward: u64,
    /// Network extension,
    extension: Arc<TendermintExtension>,
    /// codechain machine descriptor
    machine: CodeChainMachine,
    /// Chain notify
    chain_notify: Arc<TendermintChainNotify>,
    /// Lock for changing step. We use ReentrantMutex because handle_valid_message called recursively.
    pub step_change_lock: ReentrantMutex<()>,
}

impl Tendermint {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    /// Create a new instance of Tendermint engine
    pub fn new(our_params: TendermintParams, machine: CodeChainMachine) -> Arc<Self> {
        let extension = TendermintExtension::new(our_params.timeouts);
        let chain_notify = TendermintChainNotify::new();
        let engine = Arc::new(Tendermint {
            client: RwLock::new(None),
            height: AtomicUsize::new(1),
            view: AtomicUsize::new(0),
            step: RwLock::new(Step::Propose),
            votes: Default::default(),
            signer: Default::default(),
            lock_change: RwLock::new(None),
            last_lock: AtomicUsize::new(0),
            proposal: RwLock::new(None),
            proposal_parent: Default::default(),
            future_proposal: RwLock::new(None),
            last_seal: RwLock::new((Default::default(), 0, Vec::new())),
            validators: our_params.validators,
            block_reward: our_params.block_reward,
            extension: Arc::new(extension),
            chain_notify: Arc::new(chain_notify),
            machine,
            step_change_lock: ReentrantMutex::new(()),
        });
        engine.extension.register_tendermint(Arc::downgrade(&engine));
        engine.chain_notify.register_tendermint(Arc::downgrade(&engine));

        engine
    }

    /// Find the designated for the given view.
    fn view_proposer(&self, bh: &H256, height: Height, view: View) -> Address {
        let proposer_nonce = height + view;
        ctrace!(ENGINE, "Proposer nonce: {}", proposer_nonce);
        self.validators.get(bh, proposer_nonce)
    }

    pub fn height(&self) -> Height {
        self.height.load(AtomicOrdering::SeqCst)
    }

    pub fn update_future_proposal(&self, future_proposal: Bytes) {
        let prev_score = self
            .future_proposal
            .read()
            .as_ref()
            .map(|prev| BlockView::new(prev).header_view().score())
            .unwrap_or_default();
        let new_score = BlockView::new(&future_proposal).header_view().score();
        if prev_score < new_score {
            ctrace!(ENGINE, "future proposal changed");
            *self.future_proposal.write() = Some(future_proposal);
        } else {
            ctrace!(ENGINE, "future proposal not changed");
        }
    }

    pub fn future_proposal_prev_hash(&self) -> Option<H256> {
        self.future_proposal.read().as_ref().map(|b| BlockView::new(b).header_view().parent_hash())
    }

    pub fn take_future_proposal(&self) -> Option<Bytes> {
        self.future_proposal.write().take()
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
        let proposer =
            self.view_proposer(bh, self.height.load(AtomicOrdering::SeqCst), self.view.load(AtomicOrdering::SeqCst));
        self.signer.read().is_address(&proposer)
    }

    fn is_view(&self, message: &ConsensusMessage) -> bool {
        message.vote_step.is_view(self.height.load(AtomicOrdering::SeqCst), self.view.load(AtomicOrdering::SeqCst))
    }

    fn is_step(&self, message: &ConsensusMessage) -> bool {
        message.vote_step.is_step(
            self.height.load(AtomicOrdering::SeqCst),
            self.view.load(AtomicOrdering::SeqCst),
            *self.step.read(),
        )
    }

    fn is_authority(&self, address: &Address) -> bool {
        self.validators.contains(&*self.proposal_parent.read(), address)
    }

    fn check_above_threshold(&self, n: usize) -> Result<(), EngineError> {
        let threshold = self.validators.count(&*self.proposal_parent.read()) * 2 / 3;
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
        let step_votes = self.votes.count_round_votes(&VoteStep::new(
            self.height.load(AtomicOrdering::SeqCst),
            self.view.load(AtomicOrdering::SeqCst),
            *self.step.read(),
        ));
        self.check_above_threshold(step_votes).is_ok()
    }

    fn has_enough_aligned_votes(&self, message: &ConsensusMessage) -> bool {
        let aligned_count = self.votes.count_aligned_votes(&message);
        self.check_above_threshold(aligned_count).is_ok()
    }

    fn has_enough_precommit_votes(&self, block_hash: H256) -> bool {
        let vote_step = VoteStep::new(self.height(), self.view.load(AtomicOrdering::SeqCst), Step::Precommit);
        let count = self.votes.count_block_round_votes(&vote_step, &Some(block_hash));
        self.check_above_threshold(count).is_ok()
    }

    /// Broadcast all messages since last issued block to get the peers up to speed.
    fn broadcast_old_messages(&self) {
        for m in self
            .votes
            .get_up_to(&VoteStep::new(
                self.height.load(AtomicOrdering::SeqCst),
                self.view.load(AtomicOrdering::SeqCst),
                Step::Precommit,
            ))
            .into_iter()
        {
            self.broadcast_message(m);
        }
    }

    fn broadcast_message(&self, message: Bytes) {
        self.extension.broadcast_message(message);
    }

    fn update_sealing(&self) {
        if let Some(ref weak) = *self.client.read() {
            if let Some(c) = weak.upgrade() {
                c.update_sealing();
            }
        }
    }

    fn save_last_seal(&self, block_hash: H256, view: View, seal: Vec<Signature>) {
        *self.last_seal.write() = (block_hash, view, seal);
    }

    fn increment_view(&self, n: View) {
        ctrace!(ENGINE, "increment_view: New view.");
        self.view.fetch_add(n, AtomicOrdering::SeqCst);
    }

    fn should_unlock(&self, lock_change_view: View) -> bool {
        self.last_lock.load(AtomicOrdering::SeqCst) < lock_change_view
            && lock_change_view < self.view.load(AtomicOrdering::SeqCst)
    }

    fn to_next_height(&self, height: Height) {
        assert!(
            height >= self.height.load(AtomicOrdering::SeqCst),
            "{} < {}",
            height,
            self.height.load(AtomicOrdering::SeqCst)
        );
        let new_height = height + 1;
        cdebug!(ENGINE, "Received a Commit, transitioning to height {}.", new_height);
        self.last_lock.store(0, AtomicOrdering::SeqCst);
        self.height.store(new_height, AtomicOrdering::SeqCst);
        self.view.store(0, AtomicOrdering::SeqCst);
        *self.lock_change.write() = None;
        *self.proposal.write() = None;
        *self.proposal_parent.write() = Default::default();
    }

    fn to_step(&self, step: Step) {
        self.extension.set_timer_step(step);
        *self.step.write() = step;
        match step {
            Step::Propose => {
                let c = match self.client.read().as_ref().and_then(|weak| weak.upgrade()) {
                    Some(c) => c,
                    None => return,
                };

                let vote_step = VoteStep::new(
                    self.height.load(AtomicOrdering::SeqCst),
                    self.view.load(AtomicOrdering::SeqCst),
                    Step::Propose,
                );

                if let Some(hash) = self.votes.get_block_hashes(&vote_step).first() {
                    if let Some(header) = c.block_header(&BlockId::Hash(*hash)) {
                        *self.proposal.write() = Some(*hash);
                        *self.proposal_parent.write() = header.parent_hash();
                    } else {
                        ctrace!(ENGINE, "Proposal is received but not imported");
                        // Proposal is received but is not verified yet.
                        // Wait for verification.
                        return
                    }
                } else {
                    self.update_sealing()
                }
            }
            Step::Prevote => {
                let block_hash = match *self.lock_change.read() {
                    Some(ref m) if !self.should_unlock(m.vote_step.view) => m.block_hash,
                    _ => *self.proposal.read(),
                };
                self.generate_and_broadcast_message(block_hash);
            }
            Step::Precommit => {
                ctrace!(ENGINE, "to_step: Precommit.");
                let block_hash = match *self.lock_change.read() {
                    Some(ref m) if self.is_view(m) && m.block_hash.is_some() => {
                        ctrace!(ENGINE, "Setting last lock: {}", m.vote_step.view);
                        self.last_lock.store(m.vote_step.view, AtomicOrdering::SeqCst);
                        m.block_hash
                    }
                    _ => None,
                };
                self.generate_and_broadcast_message(block_hash);
            }
            Step::Commit => {
                ctrace!(ENGINE, "to_step: Commit.");
            }
        }
    }

    fn generate_and_broadcast_message(&self, block_hash: Option<BlockHash>) {
        if let Some(message) = self.generate_message(block_hash) {
            self.broadcast_message(message);
        }
    }

    fn generate_message(&self, block_hash: Option<BlockHash>) -> Option<Bytes> {
        let height = self.height.load(AtomicOrdering::SeqCst);
        let r = self.view.load(AtomicOrdering::SeqCst);
        let step = *self.step.read();
        let vote_info = message_info_rlp(&VoteStep::new(height, r, step), block_hash);
        match (self.signer.read().address(), self.sign(blake256(&vote_info))) {
            (Some(validator), Ok(signature)) => {
                let message_rlp = message_full_rlp(&signature, &vote_info);
                let message = ConsensusMessage::new(signature, height, r, step, block_hash);
                self.votes.vote(message.clone(), validator);
                cdebug!(ENGINE, "Generated {:?} as {}.", message, validator);
                self.handle_valid_message(&message);

                Some(message_rlp)
            }
            (None, _) => {
                ctrace!(ENGINE, "No message, since there is no engine signer.");
                None
            }
            (Some(validator), Err(error)) => {
                ctrace!(ENGINE, "{} could not sign the message {}", validator, error);
                None
            }
        }
    }

    fn handle_valid_message(&self, message: &ConsensusMessage) {
        let _guard = self.step_change_lock.lock();
        let vote_step = &message.vote_step;
        let is_newer_than_lock = match &*self.lock_change.read() {
            Some(lock) => *vote_step > lock.vote_step,
            None => true,
        };
        let lock_change = is_newer_than_lock
            && vote_step.height == self.height.load(AtomicOrdering::SeqCst)
            && vote_step.step == Step::Prevote
            && message.block_hash.is_some()
            && self.has_enough_aligned_votes(message);
        if lock_change {
            ctrace!(ENGINE, "handle_valid_message: Lock change.");
            *self.lock_change.write() = Some(message.clone());
        }
        // Check if it can affect the step transition.
        if self.is_step(message) {
            let next_step = match *self.step.read() {
                Step::Precommit if message.block_hash.is_none() && self.has_enough_aligned_votes(message) => {
                    self.increment_view(1);
                    Some(Step::Propose)
                }
                Step::Precommit if self.has_enough_aligned_votes(message) => {
                    let bh = message.block_hash.expect("previous guard ensures is_some; qed");
                    let c = match self.client.read().as_ref().and_then(|weak| weak.upgrade()) {
                        Some(c) => c,
                        None => return,
                    };
                    if c.block(&BlockId::Hash(bh)).is_some() {
                        // Commit the block using a complete signature set.
                        // Generate seal and remove old votes.
                        let precommits = self.votes.round_signatures(vote_step, &bh);
                        ctrace!(ENGINE, "Collected seal: {:?}", precommits);
                        self.save_last_seal(bh, message.vote_step.view, precommits);
                        self.votes.throw_out_old(&vote_step);
                        self.to_next_height(self.height());
                        Some(Step::Commit)
                    } else {
                        cwarn!(ENGINE, "Cannot find a proposal which committed");
                        self.increment_view(1);
                        Some(Step::Propose)
                    }
                }
                // Avoid counting votes twice.
                Step::Prevote if lock_change => Some(Step::Precommit),
                Step::Prevote if self.has_enough_aligned_votes(message) => Some(Step::Precommit),
                _ => None,
            };

            if let Some(step) = next_step {
                ctrace!(ENGINE, "Transition to {:?} triggered.", step);
                self.to_step(step);
            }
        }
    }

    pub fn on_imported_proposal(&self, proposal: &Header) {
        let _guard = self.step_change_lock.lock();

        let current_height = self.height.load(AtomicOrdering::SeqCst);
        let height = proposal.number() as Height;
        let view = consensus_view(proposal).expect("Imported block is already verified");
        if current_height == height && self.view.load(AtomicOrdering::SeqCst) == view {
            *self.proposal.write() = Some(proposal.hash());
            *self.proposal_parent.write() = *proposal.parent_hash();
        } else if current_height < height {
            self.to_next_height(height - 1);
            *self.proposal.write() = Some(proposal.hash());
            *self.proposal_parent.write() = *proposal.parent_hash();
            self.to_step(Step::Prevote);
        }
    }
}

impl ConsensusEngine<CodeChainMachine> for Tendermint {
    fn name(&self) -> &str {
        "Tendermint"
    }

    fn machine(&self) -> &CodeChainMachine {
        &self.machine
    }

    /// (consensus view, proposal signature, authority signatures)
    fn seal_fields(&self, _header: &Header) -> usize {
        3
    }

    /// Should this node participate.
    fn seals_internally(&self) -> Option<bool> {
        Some(self.signer.read().is_some())
    }

    fn engine_type(&self) -> EngineType {
        EngineType::InternalSealing
    }

    /// Attempt to seal generate a proposal seal.
    ///
    /// This operation is synchronous and may (quite reasonably) not be available, in which case
    /// `Seal::None` will be returned.
    fn generate_seal(&self, block: &ExecutedBlock, _parent: &Header) -> Seal {
        let header = block.header();
        let height = header.number() as Height;
        // Only proposer can generate seal if None was generated.
        if !self.is_signer_proposer(header.parent_hash())
            || self.proposal.read().is_some()
            || height < self.height.load(AtomicOrdering::SeqCst)
        {
            return Seal::None
        }

        let view = self.view.load(AtomicOrdering::SeqCst);

        let (_, last_block_view, seal) = &*self.last_seal.read();
        Seal::Tendermint {
            prev_view: *last_block_view,
            cur_view: view,
            precommits: seal.clone(),
        }
    }

    fn proposal_generated(&self, sealed_block: &SealedBlock) {
        let header = sealed_block.header();
        let hash = header.hash();

        *self.proposal.write() = Some(hash);
        *self.proposal_parent.write() = *header.parent_hash();
        cdebug!(
            ENGINE,
            "Submitting proposal {} at height {}-{} view {}-{}.\n{:?}",
            hash,
            self.height(),
            header.number(),
            self.view.load(AtomicOrdering::SeqCst),
            consensus_view(header).expect("This block is generated by this machine."),
            header
        );
    }

    fn verify_local_seal(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        let seal_length = header.seal().len();
        let expected_seal_fields = self.seal_fields(header);
        if seal_length == expected_seal_fields {
            Ok(())
        } else {
            Err(BlockError::InvalidSealArity(Mismatch {
                expected: expected_seal_fields,
                found: seal_length,
            })
            .into())
        }
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        let height = header.number() as usize;
        let view = consensus_view(header).unwrap();
        ctrace!(ENGINE, "Verify external at {}-{}, {:?}", height, view, header);
        let proposer = header.author();
        if !self.is_authority(proposer) {
            return Err(EngineError::NotAuthorized(*proposer).into())
        }
        self.check_view_proposer(header.parent_hash(), header.number() as usize, consensus_view(header)?, &proposer)
            .map_err(Error::from)?;

        let previous_block_view = previous_block_view(header)?;
        let vote_step = VoteStep::new((header.number() - 1) as usize, previous_block_view, Step::Precommit);
        let precommit_hash = message_hash(vote_step, *header.parent_hash());
        let precommits_field =
            &header.seal().get(2).expect("block went through verify_block_basic; block has .seal_fields() fields; qed");
        let mut origins = HashSet::new();
        for rlp in UntrustedRlp::new(precommits_field).iter() {
            let precommit = ConsensusMessage {
                signature: rlp.as_val()?,
                block_hash: Some(header.hash()),
                vote_step,
            };
            let address = match self.votes.get(&precommit) {
                Some(a) => a,
                None => public_to_address(&recover(&precommit.signature, &precommit_hash)?),
            };
            if !self.validators.contains(header.parent_hash(), &address) {
                return Err(EngineError::NotAuthorized(address.to_owned()).into())
            }

            if !origins.insert(address) {
                cwarn!(ENGINE, "verify_block_unordered: Duplicate signature from {} on the seal.", address);
                return Err(BlockError::InvalidSeal.into())
            }
        }

        // Genesisblock does not have signatures
        if header.number() == 1 {
            return Ok(())
        }
        self.check_above_threshold(origins.len()).map_err(Into::into)
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
                    recover: |signature: &Signature, message: &Message| {
                        Ok(public_to_address(&recover(&signature, &message)?))
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
        let height = U256::from(header.number());
        let new_score = u256_from_u128(std::u128::MAX) * height - self.view.load(AtomicOrdering::SeqCst).into();

        header.set_score(new_score);
    }

    /// Equivalent to a timeout: to be used for tests.
    fn step(&self) {
        let _guard = self.step_change_lock.lock();
        let next_step = match *self.step.read() {
            Step::Propose => {
                ctrace!(ENGINE, "Propose timeout.");
                if self.proposal.read().is_none() {
                    // Report the proposer if no proposal was received.
                    let height = self.height.load(AtomicOrdering::SeqCst);
                    let current_proposer = self.view_proposer(
                        &*self.proposal_parent.read(),
                        height,
                        self.view.load(AtomicOrdering::SeqCst),
                    );
                    self.validators.report_benign(&current_proposer, height as BlockNumber, height as BlockNumber);
                }
                Step::Prevote
            }
            Step::Prevote if self.has_enough_any_votes() => {
                ctrace!(ENGINE, "Prevote timeout.");
                Step::Precommit
            }
            Step::Prevote => {
                ctrace!(ENGINE, "Prevote timeout without enough votes.");
                self.broadcast_old_messages();
                Step::Prevote
            }
            Step::Precommit if self.has_enough_any_votes() => {
                ctrace!(ENGINE, "Precommit timeout.");
                self.increment_view(1);
                Step::Propose
            }
            Step::Precommit => {
                ctrace!(ENGINE, "Precommit timeout without enough votes.");
                self.broadcast_old_messages();
                Step::Precommit
            }
            Step::Commit => {
                ctrace!(ENGINE, "Commit timeout.");
                Step::Propose
            }
        };
        self.to_step(next_step);
    }

    fn stop(&self) {}

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
        let total_reward = self.block_reward(block.header().number())
            + self.block_fee(Box::new(block.parcels().to_owned().into_iter().map(Into::into)));
        self.machine.add_balance(block, &author, total_reward)
    }

    fn register_client(&self, client: Weak<EngineClient>) {
        if let Some(c) = client.upgrade() {
            self.height.store(c.chain_info().best_block_number as usize + 1, AtomicOrdering::SeqCst);
            *self.last_seal.write() = (c.best_block_header().hash(), 0, Vec::new());
        }
        *self.client.write() = Some(client.clone());
        self.extension.register_client(client.clone());
        self.validators.register_client(client.clone());
        self.chain_notify.register_client(client);
    }

    fn handle_message(&self, rlp: &[u8]) -> Result<(), EngineError> {
        fn fmt_err<T: ::std::fmt::Debug>(x: T) -> EngineError {
            EngineError::MalformedMessage(format!("{:?}", x))
        }

        let rlp = UntrustedRlp::new(rlp);
        let message: ConsensusMessage = rlp.as_val().map_err(fmt_err)?;
        if !self.votes.is_old_or_known(&message) {
            let msg_hash = blake256(rlp.at(1).map_err(fmt_err)?.as_raw());
            let sender = public_to_address(&recover(&message.signature, &msg_hash).map_err(fmt_err)?);

            if !self.is_authority(&sender) {
                return Err(EngineError::NotAuthorized(sender))
            }
            self.broadcast_message(rlp.as_raw().to_vec());
            if let Some(double) = self.votes.vote(message.clone(), sender) {
                let height = message.vote_step.height as BlockNumber;
                self.validators.report_malicious(&sender, height, height, ::rlp::encode(&double).into_vec());
                return Err(EngineError::DoubleVote(sender))
            }
            ctrace!(ENGINE, "Handling a valid {:?} from {}.", message, sender);
            self.handle_valid_message(&message);
        }
        Ok(())
    }

    fn is_proposal(&self, header: &Header) -> bool {
        let number = header.number();
        if self.height() > number as usize {
            return false
        }

        let c = match self.client.read().as_ref().and_then(|weak| weak.upgrade()) {
            Some(c) => c,
            None => return false,
        };

        // if next header is imported, current header is not a proposal
        if c.block_header(&BlockId::Number(number + 1)).map_or(false, |next| next.parent_hash() == header.hash()) {
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
            let vote_info = message_info_rlp(&vote_step, Some(hash));
            let signature = self.sign(blake256(&vote_info)).expect("I am proposer");
            self.extension.broadcast_proposal_block(signature, block.into_inner());
        } else if let Some(signature) = self.votes.round_signatures(&vote_step, &hash).first() {
            self.extension.broadcast_proposal_block(*signature, block.into_inner());
        } else {
            cwarn!(ENGINE, "There is a proposal but does not have signature {:?}", vote_step);
        }
    }

    fn set_signer(&self, ap: Arc<AccountProvider>, address: Address, password: Option<Password>) {
        let _guard = self.step_change_lock.lock();
        {
            self.signer.write().set(ap, address, password);
        }
        self.to_step(Step::Propose);
    }

    fn sign(&self, hash: H256) -> Result<Signature, Error> {
        self.signer.read().sign(hash).map_err(Into::into)
    }

    fn register_network_extension_to_service(&self, service: &NetworkService) {
        service.register_extension(Arc::clone(&self.extension));
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

    fn get_block_hash_to_mine_on(&self, _best_block_hash: H256) -> H256 {
        let c = self.client.read().as_ref().and_then(|weak| weak.upgrade()).expect("Client should be exist");

        let prev_height = self.height() - 1;
        c.block_header(&BlockId::Number(prev_height as BlockNumber))
            .expect("Previous height's block should be imported")
            .hash()
    }

    fn get_best_block_from_highest_score_header(&self, header: &HeaderView) -> H256 {
        header.parent_hash()
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
        _enacted: Vec<H256>,
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

        let imported_is_empty = imported.is_empty();
        if imported_is_empty {
            return
        }
        let _guard = t.step_change_lock.lock();
        let mut height_changed = false;
        for hash in imported {
            // New Commit received, skip to next height.
            let header = c.block_header(&hash.into()).expect("ChainNotify is called after the block is imported");

            let full_header = header.decode();
            if t.is_proposal(&full_header) {
                t.on_imported_proposal(&full_header);
            } else if t.height() < header.number() as usize {
                height_changed = true;
                ctrace!(ENGINE, "Received a commit: {:?}.", header.number());
                t.to_next_height((header.number() - 1) as usize);
            }
        }
        if height_changed {
            t.to_step(Step::Commit)
        }

        if t.future_proposal_prev_hash().map(|hash| c.block(&BlockId::Hash(hash)).is_some()).unwrap_or(false) {
            if let Err(err) = c
                .import_block(t.take_future_proposal().expect("future_proposal_prev_hash is already checked existance"))
            {
                cwarn!(ENGINE, "Future proposal import failed {:?}", err);
            }
        }
    }
}

struct EpochVerifier<F>
where
    F: Fn(&Signature, &Message) -> Result<Address, Error> + Send + Sync, {
    subchain_validators: ValidatorList,
    recover: F,
}

impl<F> super::EpochVerifier<CodeChainMachine> for EpochVerifier<F>
where
    F: Fn(&Signature, &Message) -> Result<Address, Error> + Send + Sync,
{
    fn verify_light(&self, header: &Header) -> Result<(), Error> {
        let message = header.hash();

        let mut addresses = HashSet::new();
        let header_precommits_field = &header.seal().get(2).ok_or(BlockError::InvalidSeal)?;
        for rlp in UntrustedRlp::new(header_precommits_field).iter() {
            let signature: Signature = rlp.as_val()?;
            let address = (self.recover)(&signature, &message)?;

            if !self.subchain_validators.contains(header.parent_hash(), &address) {
                return Err(EngineError::NotAuthorized(address.to_owned()).into())
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

/// Timeouts lookup
pub trait Timeouts<S: Sync + Send + Clone>: Send + Sync {
    /// Return the first timeout.
    fn initial(&self) -> Duration;

    /// Get a timeout based on step.
    fn timeout(&self, step: &S) -> Duration;
}

struct TendermintExtension {
    tendermint: RwLock<Option<Weak<Tendermint>>>,
    client: RwLock<Option<Weak<EngineClient>>>,
    peers: RwLock<HashSet<NodeId>>,
    api: Mutex<Option<Arc<Api>>>,
    timeouts: TendermintTimeouts,
}

const MIN_PEERS_PROPAGATION: usize = 4;
const MAX_PEERS_PROPAGATION: usize = 128;

impl TendermintExtension {
    fn new(timeouts: TendermintTimeouts) -> Self {
        Self {
            tendermint: RwLock::new(None),
            client: RwLock::new(None),
            peers: RwLock::new(HashSet::new()),
            api: Mutex::new(None),
            timeouts,
        }
    }

    fn register_client(&self, client: Weak<EngineClient>) {
        *self.client.write() = Some(client);
    }

    fn select_random_peers(&self) -> Vec<NodeId> {
        let mut peers: Vec<NodeId> = self.peers.write().iter().cloned().collect();
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
        if let Some(api) = self.api.lock().as_ref() {
            for token in tokens {
                api.send(&token, &message);
            }
        }
    }

    fn broadcast_proposal_block(&self, signature: Signature, message: Bytes) {
        let message = TendermintMessage::ProposalBlock(signature, message).rlp_bytes().into_vec();
        if let Some(api) = self.api.lock().as_ref() {
            for token in self.peers.read().iter() {
                api.send(&token, &message);
            }
        };
    }

    fn set_timer_step(&self, step: Step) {
        if let Some(api) = self.api.lock().as_ref() {
            let t = match self.tendermint.read().as_ref().and_then(|weak| weak.upgrade()) {
                Some(c) => c,
                None => return,
            };
            let view = t.view.load(AtomicOrdering::SeqCst);
            let du = Duration::seconds(view as i64);

            api.clear_timer(ENGINE_TIMEOUT_TOKEN).expect("Timer clear succeeds");
            api.set_timer_once(ENGINE_TIMEOUT_TOKEN, self.timeouts.timeout(&step) + du).expect("Timer set succeeds");
        };
    }

    fn register_tendermint(&self, tendermint: Weak<Tendermint>) {
        *self.tendermint.write() = Some(tendermint);
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

    fn on_initialize(&self, api: Arc<Api>) {
        let initial = self.timeouts.initial();
        ctrace!(ENGINE, "Setting the initial timeout to {}.", initial);
        api.set_timer_once(ENGINE_TIMEOUT_TOKEN, initial).expect("Timer set succeeds");
        *self.api.lock() = Some(api);
    }

    fn on_node_added(&self, token: &NodeId, _version: u64) {
        self.peers.write().insert(*token);
    }

    fn on_node_removed(&self, token: &NodeId) {
        self.peers.write().remove(token);
    }

    fn on_message(&self, token: &NodeId, data: &[u8]) {
        let m = UntrustedRlp::new(data);
        match m.as_val() {
            Ok(TendermintMessage::ConsensusMessage(ref bytes)) => {
                if let Some(ref weak) = *self.tendermint.read() {
                    if let Some(c) = weak.upgrade() {
                        if let Err(e) = c.handle_message(bytes) {
                            cinfo!(ENGINE, "Failed to handle message {:?}", e);
                        }
                    }
                }
            }
            Ok(TendermintMessage::ProposalBlock(signature, bytes)) => {
                let c = match self.client.read().as_ref().and_then(|weak| weak.upgrade()) {
                    Some(c) => c,
                    None => return,
                };
                let t = match self.tendermint.read().as_ref().and_then(|weak| weak.upgrade()) {
                    Some(c) => c,
                    None => return,
                };

                // This block borrows bytes
                {
                    let block_view = BlockView::new(&bytes);
                    let header_view = block_view.header();
                    let number = header_view.number();
                    ctrace!(ENGINE, "Proposal received for {}-{:?}", number, header_view.hash());

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
                        t.update_future_proposal(bytes.clone());
                        return
                    }

                    let message = match ConsensusMessage::new_proposal(signature, &header_view) {
                        Ok(message) => message,
                        Err(err) => {
                            cdebug!(ENGINE, "Invalid proposal received: {:?}", err);
                            return
                        }
                    };

                    let author_from_signature = match message.signer() {
                        Ok(address) => address,
                        Err(err) => {
                            cdebug!(ENGINE, "Proposal verification failed: {:?}", err);
                            return
                        }
                    };

                    if *header_view.author() != author_from_signature {
                        cdebug!(
                            ENGINE,
                            "Proposal author({}) not matched with header({})",
                            author_from_signature,
                            header_view.author()
                        );
                        return
                    }

                    if t.votes.is_old_or_known(&message) {
                        cdebug!(ENGINE, "Proposal is already known");
                        return
                    }

                    t.votes.vote(message, author_from_signature);
                }

                if let Err(e) = c.import_block(bytes) {
                    cinfo!(ENGINE, "Failed to import proposal block {:?}", e);
                }
            }
            _ => cinfo!(ENGINE, "Invalid message from peer {}", token),
        }
    }
}

impl TimeoutHandler for TendermintExtension {
    fn on_timeout(&self, timer: TimerToken) {
        match timer {
            ENGINE_TIMEOUT_TOKEN => {
                if let Some(ref weak) = *self.tendermint.read() {
                    if let Some(c) = weak.upgrade() {
                        c.step();
                    }
                }
            }
            _ => debug_assert!(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::block::{ClosedBlock, IsBlock, OpenBlock};
    use crate::consensus::CodeChainEngine;
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    use super::*;

    /// Accounts inserted with "0" and "1" are validators. First proposer is "0".
    fn setup() -> (Scheme, Arc<AccountProvider>) {
        let tap = AccountProvider::transient_provider();
        let scheme = Scheme::new_test_tendermint();
        (scheme, tap)
    }

    fn propose_default(scheme: &Scheme, proposer: Address) -> (ClosedBlock, Vec<Bytes>) {
        let db = get_temp_state_db();
        let db = scheme.ensure_genesis_state(db).unwrap();
        let genesis_header = scheme.genesis_header();
        let b = OpenBlock::try_new(scheme.engine.as_ref(), db, &genesis_header, proposer, vec![], false).unwrap();
        let b = b.close(*genesis_header.parcels_root(), *genesis_header.invoices_root()).unwrap();
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
        engine.set_signer(tap.clone(), addr, Some(acc.into()));
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
            Err(_) => {
                panic!("should be block seal-arity mismatch error (got {:?})", verify_result);
            }
            _ => {
                panic!("Should be error, got Ok");
            }
        }
    }

    #[test]
    fn generate_seal() {
        let (scheme, tap) = setup();

        let proposer = insert_and_register(&tap, scheme.engine.as_ref(), "1");

        let (b, seal) = propose_default(&scheme, proposer);
        assert!(b.lock().try_seal(scheme.engine.as_ref(), seal).is_ok());
    }

    #[test]
    fn seal_signatures_checking() {
        let (spec, tap) = setup();
        let engine = spec.engine;

        let mut header = Header::default();
        header.set_number(4);
        let proposer = insert_and_unlock(&tap, "0");
        header.set_author(proposer);
        header.set_parent_hash(Default::default());

        let vote_info = message_info_rlp(&VoteStep::new(3, 0, Step::Precommit), Some(header.parent_hash().clone()));
        let signature0 = tap.sign(proposer, None, blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0],
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
        let signature1 = tap.sign(voter, None, blake256(&vote_info)).unwrap();
        let voter = insert_and_unlock(&tap, "2");
        let signature2 = tap.sign(voter, None, blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0, signature1, signature2],
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        assert!(engine.verify_block_external(&header).is_ok());

        let bad_voter = insert_and_unlock(&tap, "101");
        let bad_signature = tap.sign(bad_voter, None, blake256(vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0, signature1, bad_signature],
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        // Two good and one bad signature.
        match engine.verify_block_external(&header) {
            Err(Error::Engine(EngineError::NotAuthorized(_))) => {}
            _ => panic!(),
        };
        engine.stop();
    }
}
