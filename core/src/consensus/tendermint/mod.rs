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

use std::cmp;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};

use cbytes::Bytes;
use ccrypto::blake256;
use ckeys::{public_to_address, recover_ecdsa};
use ckeys::{ECDSASignature, Message};
use cnetwork::{Api, NetworkExtension, NodeId, TimerToken};
use ctypes::{Address, H256, H520, U128, U256};
use parking_lot::{Mutex, RwLock};
use rand::{thread_rng, Rng};
use rlp::{self, Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};
use time::Duration;
use unexpected::{Mismatch, OutOfBounds};

use self::message::*;
pub use self::params::{TendermintParams, TendermintTimeouts};
use super::super::account_provider::AccountProvider;
use super::super::block::*;
use super::super::client::EngineClient;
use super::super::codechain_machine::CodeChainMachine;
use super::super::error::{BlockError, Error};
use super::super::header::Header;
use super::super::machine::Machine;
use super::super::types::BlockNumber;
use super::signer::EngineSigner;
use super::validator_set::validator_list::ValidatorList;
use super::validator_set::ValidatorSet;
use super::vote_collector::VoteCollector;
use super::{ConsensusEngine, ConstructedVerifier, EngineError, EpochChange, Seal};

const EXTENSION_NAME: &'static str = "tendermint";

/// Timer token representing the consensus step timeouts.
pub const ENGINE_TIMEOUT_TOKEN: TimerToken = 23;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Step {
    Propose,
    Prevote,
    Precommit,
    Commit,
}

impl Step {
    pub fn is_pre(self) -> bool {
        match self {
            Step::Prevote | Step::Precommit => true,
            _ => false,
        }
    }

    fn number(&self) -> u8 {
        match self {
            Step::Propose => 0,
            Step::Prevote => 1,
            Step::Precommit => 2,
            Step::Commit => 3,
        }
    }
}

impl Decodable for Step {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.as_val()? {
            0u8 => Ok(Step::Propose),
            1 => Ok(Step::Prevote),
            2 => Ok(Step::Precommit),
            // FIXME: Step::Commit case is not necessary if Api::send_local_message does not serialize message.
            3 => Ok(Step::Commit),
            _ => Err(DecoderError::Custom("Invalid step.")),
        }
    }
}

impl Encodable for Step {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_internal(&self.number());
    }
}

pub type Height = usize;
pub type View = usize;
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
    /// Bare hash of the proposed block, used for seal submission.
    proposal: RwLock<Option<H256>>,
    /// Hash of the proposal parent block.
    proposal_parent: RwLock<H256>,
    /// Last block proposed by this validator.
    last_proposed: RwLock<H256>,
    /// Set used to determine the current validators.
    validators: Box<ValidatorSet>,
    /// Reward per block, in base units.
    block_reward: U256,
    /// Network extension,
    extension: Arc<TendermintExtension>,
    /// codechain machine descriptor
    machine: CodeChainMachine,
}

impl Tendermint {
    /// Create a new instance of Tendermint engine
    pub fn new(our_params: TendermintParams, machine: CodeChainMachine) -> Result<Arc<Self>, Error> {
        let extension = TendermintExtension::new(our_params.timeouts);
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
            last_proposed: Default::default(),
            validators: our_params.validators,
            block_reward: our_params.block_reward,
            extension: Arc::new(extension),
            machine,
        });
        engine.extension.register_tendermint(Arc::downgrade(&engine));

        Ok(engine)
    }

    /// Find the designated for the given view.
    fn view_proposer(&self, bh: &H256, height: Height, view: View) -> Address {
        let proposer_nonce = height + view;
        trace!(target: "engine", "Proposer nonce: {}", proposer_nonce);
        self.validators.get(bh, proposer_nonce)
    }

    /// Check if address is a proposer for given view.
    fn check_view_proposer(&self, bh: &H256, height: Height, view: View, address: &Address) -> Result<(), EngineError> {
        let proposer = self.view_proposer(bh, height, view);
        if proposer == *address {
            Ok(())
        } else {
            Err(EngineError::NotProposer(Mismatch {
                expected: proposer,
                found: address.clone(),
            }))
        }
    }

    /// Check if current signer is the current proposer.
    fn is_signer_proposer(&self, bh: &H256) -> bool {
        let proposer =
            self.view_proposer(bh, self.height.load(AtomicOrdering::SeqCst), self.view.load(AtomicOrdering::SeqCst));
        self.signer.read().is_address(&proposer)
    }

    fn is_height(&self, message: &ConsensusMessage) -> bool {
        message.vote_step.is_height(self.height.load(AtomicOrdering::SeqCst))
    }

    fn is_view(&self, message: &ConsensusMessage) -> bool {
        message.vote_step.is_view(self.height.load(AtomicOrdering::SeqCst), self.view.load(AtomicOrdering::SeqCst))
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

    fn has_enough_future_step_votes(&self, vote_step: &VoteStep) -> bool {
        if vote_step.view > self.view.load(AtomicOrdering::SeqCst) {
            let step_votes = self.votes.count_round_votes(vote_step);
            self.check_above_threshold(step_votes).is_ok()
        } else {
            false
        }
    }

    fn has_enough_aligned_votes(&self, message: &ConsensusMessage) -> bool {
        let aligned_count = self.votes.count_aligned_votes(&message);
        self.check_above_threshold(aligned_count).is_ok()
    }

    /// Broadcast all messages since last issued block to get the peers up to speed.
    fn broadcast_old_messages(&self) {
        for m in self.votes
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

    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>) {
        if let Some(ref weak) = *self.client.read() {
            if let Some(c) = weak.upgrade() {
                c.submit_seal(block_hash, seal);
            }
        }
    }

    fn increment_view(&self, n: View) {
        trace!(target: "engine", "increment_view: New view.");
        self.view.fetch_add(n, AtomicOrdering::SeqCst);
    }

    fn should_unlock(&self, lock_change_view: View) -> bool {
        self.last_lock.load(AtomicOrdering::SeqCst) < lock_change_view
            && lock_change_view < self.view.load(AtomicOrdering::SeqCst)
    }

    fn to_next_height(&self, height: Height) {
        let new_height = height + 1;
        debug!(target: "engine", "Received a Commit, transitioning to height {}.", new_height);
        self.last_lock.store(0, AtomicOrdering::SeqCst);
        self.height.store(new_height, AtomicOrdering::SeqCst);
        self.view.store(0, AtomicOrdering::SeqCst);
        *self.lock_change.write() = None;
        *self.proposal.write() = None;
    }

    fn to_step(&self, step: Step) {
        self.extension.send_local_message(step);
        *self.step.write() = step;
        match step {
            Step::Propose => self.update_sealing(),
            Step::Prevote => {
                let block_hash = match *self.lock_change.read() {
                    Some(ref m) if !self.should_unlock(m.vote_step.view) => m.block_hash,
                    _ => self.proposal.read().clone(),
                };
                self.generate_and_broadcast_message(block_hash);
            }
            Step::Precommit => {
                trace!(target: "engine", "to_step: Precommit.");
                let block_hash = match *self.lock_change.read() {
                    Some(ref m) if self.is_view(m) && m.block_hash.is_some() => {
                        trace!(target: "engine", "Setting last lock: {}", m.vote_step.view);
                        self.last_lock.store(m.vote_step.view, AtomicOrdering::SeqCst);
                        m.block_hash
                    }
                    _ => None,
                };
                self.generate_and_broadcast_message(block_hash);
            }
            Step::Commit => {
                trace!(target: "engine", "to_step: Commit.");
            }
        }
    }

    fn generate_and_broadcast_message(&self, block_hash: Option<BlockHash>) {
        if let Some(message) = self.generate_message(block_hash) {
            self.broadcast_message(message);
        }
    }

    fn generate_message(&self, block_hash: Option<BlockHash>) -> Option<Bytes> {
        let h = self.height.load(AtomicOrdering::SeqCst);
        let r = self.view.load(AtomicOrdering::SeqCst);
        let s = *self.step.read();
        let vote_info = message_info_rlp(&VoteStep::new(h, r, s), block_hash);
        match (self.signer.read().address(), self.sign(blake256(&vote_info)).map(Into::into)) {
            (Some(validator), Ok(signature)) => {
                let message_rlp = message_full_rlp(&signature, &vote_info);
                let message = ConsensusMessage::new(signature, h, r, s, block_hash);
                self.votes.vote(message.clone(), validator);
                debug!(target: "engine", "Generated {:?} as {}.", message, validator);
                self.handle_valid_message(&message);

                Some(message_rlp)
            }
            (None, _) => {
                trace!(target: "engine", "No message, since there is no engine signer.");
                None
            }
            (Some(v), Err(e)) => {
                trace!(target: "engine", "{} could not sign the message {}", v, e);
                None
            }
        }
    }

    fn handle_valid_message(&self, message: &ConsensusMessage) {
        let ref vote_step = message.vote_step;
        let is_newer_than_lock = match &*self.lock_change.read() {
            Some(lock) => vote_step > &lock.vote_step,
            None => true,
        };
        let lock_change = is_newer_than_lock && vote_step.step == Step::Prevote && message.block_hash.is_some()
            && self.has_enough_aligned_votes(message);
        if lock_change {
            trace!(target: "engine", "handle_valid_message: Lock change.");
            *self.lock_change.write() = Some(message.clone());
        }
        // Check if it can affect the step transition.
        if self.is_height(message) {
            let next_step = match *self.step.read() {
                Step::Precommit if message.block_hash.is_none() && self.has_enough_aligned_votes(message) => {
                    self.increment_view(1);
                    Some(Step::Propose)
                }
                Step::Precommit if self.has_enough_aligned_votes(message) => {
                    let bh = message.block_hash.expect("previous guard ensures is_some; qed");
                    if *self.last_proposed.read() == bh {
                        // Commit the block using a complete signature set.
                        // Generate seal and remove old votes.
                        let precommits = self.votes.round_signatures(vote_step, &bh);
                        trace!(target: "engine", "Collected seal: {:?}", precommits);
                        let seal = vec![
                            ::rlp::encode(&vote_step.view).into_vec(),
                            ::rlp::NULL_RLP.to_vec(),
                            ::rlp::encode_list(&precommits).into_vec(),
                        ];
                        self.submit_seal(bh, seal);
                        self.votes.throw_out_old(&vote_step);
                    }
                    self.to_next_height(self.height.load(AtomicOrdering::SeqCst));
                    Some(Step::Commit)
                }
                Step::Precommit if self.has_enough_future_step_votes(&vote_step) => {
                    self.increment_view(vote_step.view - self.view.load(AtomicOrdering::SeqCst));
                    Some(Step::Precommit)
                }
                // Avoid counting votes twice.
                Step::Prevote if lock_change => Some(Step::Precommit),
                Step::Prevote if self.has_enough_aligned_votes(message) => Some(Step::Precommit),
                Step::Prevote if self.has_enough_future_step_votes(&vote_step) => {
                    self.increment_view(vote_step.view - self.view.load(AtomicOrdering::SeqCst));
                    Some(Step::Prevote)
                }
                _ => None,
            };

            if let Some(step) = next_step {
                trace!(target: "engine", "Transition to {:?} triggered.", step);
                self.to_step(step);
            }
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

    /// Attempt to seal generate a proposal seal.
    ///
    /// This operation is synchronous and may (quite reasonably) not be available, in which case
    /// `Seal::None` will be returned.
    fn generate_seal(&self, block: &ExecutedBlock, _parent: &Header) -> Seal {
        let header = block.header();
        // Only proposer can generate seal if None was generated.
        if !self.is_signer_proposer(header.parent_hash()) || self.proposal.read().is_some() {
            return Seal::None
        }

        let height = header.number() as Height;
        let view = self.view.load(AtomicOrdering::SeqCst);
        let bh = Some(header.bare_hash());
        let vote_info = message_info_rlp(&VoteStep::new(height, view, Step::Propose), bh.clone());
        if let Ok(signature) = self.sign(blake256(&vote_info)).map(Into::into) {
            // Insert Propose vote.
            debug!(target: "engine", "Submitting proposal {} at height {} view {}.", header.bare_hash(), height, view);
            let sender = self.signer.read().address().expect("seals_internally already returned true");
            self.votes.vote(ConsensusMessage::new(signature, height, view, Step::Propose, bh), sender);
            // Remember the owned block.
            *self.last_proposed.write() = header.bare_hash();
            // Remember proposal for later seal submission.
            *self.proposal.write() = bh;
            *self.proposal_parent.write() = header.parent_hash().clone();
            Seal::Proposal(vec![
                ::rlp::encode(&view).into_vec(),
                ::rlp::encode(&signature).into_vec(),
                ::rlp::EMPTY_LIST_RLP.to_vec(),
            ])
        } else {
            warn!(target: "engine", "generate_seal: FAIL: accounts secret key unavailable");
            Seal::None
        }
    }

    fn verify_local_seal(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        let seal_length = header.seal().len();
        let expected_seal_fields = self.seal_fields(header);
        if seal_length == expected_seal_fields {
            // Either proposal or commit.
            if (header.seal()[1] == ::rlp::NULL_RLP) != (header.seal()[2] == ::rlp::EMPTY_LIST_RLP) {
                Ok(())
            } else {
                warn!(target: "engine", "verify_block_basic: Block is neither a Commit nor Proposal.");
                Err(BlockError::InvalidSeal.into())
            }
        } else {
            Err(BlockError::InvalidSealArity(Mismatch {
                expected: expected_seal_fields,
                found: seal_length,
            }).into())
        }
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        if let Ok(proposal) = ConsensusMessage::new_proposal(header) {
            let proposer = proposal.verify()?;
            if !self.is_authority(&proposer) {
                return Err(EngineError::NotAuthorized(proposer).into())
            }
            self.check_view_proposer(
                header.parent_hash(),
                proposal.vote_step.height,
                proposal.vote_step.view,
                &proposer,
            ).map_err(Into::into)
        } else {
            let vote_step = VoteStep::new(header.number() as usize, consensus_view(header)?, Step::Precommit);
            let precommit_hash = message_hash(vote_step.clone(), header.bare_hash());
            let ref signatures_field = header
                .seal()
                .get(2)
                .expect("block went through verify_block_basic; block has .seal_fields() fields; qed");
            let mut origins = HashSet::new();
            for rlp in UntrustedRlp::new(signatures_field).iter() {
                let precommit = ConsensusMessage {
                    signature: rlp.as_val()?,
                    block_hash: Some(header.bare_hash()),
                    vote_step: vote_step.clone(),
                };
                let address = match self.votes.get(&precommit) {
                    Some(a) => a,
                    None => public_to_address(&recover_ecdsa(&precommit.signature.into(), &precommit_hash)?),
                };
                if !self.validators.contains(header.parent_hash(), &address) {
                    return Err(EngineError::NotAuthorized(address.to_owned()).into())
                }

                if !origins.insert(address) {
                    warn!(target: "engine", "verify_block_unordered: Duplicate signature from {} on the seal.", address);
                    return Err(BlockError::InvalidSeal.into())
                }
            }

            self.check_above_threshold(origins.len()).map_err(Into::into)
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
        self.machine.add_balance(block, &author, &self.block_reward)
    }

    fn handle_message(&self, rlp: &[u8]) -> Result<(), EngineError> {
        fn fmt_err<T: ::std::fmt::Debug>(x: T) -> EngineError {
            EngineError::MalformedMessage(format!("{:?}", x))
        }

        let rlp = UntrustedRlp::new(rlp);
        let message: ConsensusMessage = rlp.as_val().map_err(fmt_err)?;
        if !self.votes.is_old_or_known(&message) {
            let msg_hash = blake256(rlp.at(1).map_err(fmt_err)?.as_raw());
            let sender = public_to_address(&recover_ecdsa(&message.signature.into(), &msg_hash).map_err(fmt_err)?);

            if !self.is_authority(&sender) {
                return Err(EngineError::NotAuthorized(sender))
            }
            self.broadcast_message(rlp.as_raw().to_vec());
            if let Some(double) = self.votes.vote(message.clone(), sender) {
                let height = message.vote_step.height as BlockNumber;
                self.validators.report_malicious(&sender, height, height, ::rlp::encode(&double).into_vec());
                return Err(EngineError::DoubleVote(sender))
            }
            trace!(target: "engine", "Handling a valid {:?} from {}.", message, sender);
            self.handle_valid_message(&message);
        }
        Ok(())
    }

    /// Equivalent to a timeout: to be used for tests.
    fn step(&self) {
        let next_step = match *self.step.read() {
            Step::Propose => {
                trace!(target: "engine", "Propose timeout.");
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
                trace!(target: "engine", "Prevote timeout.");
                Step::Precommit
            }
            Step::Prevote => {
                trace!(target: "engine", "Prevote timeout without enough votes.");
                self.broadcast_old_messages();
                Step::Prevote
            }
            Step::Precommit if self.has_enough_any_votes() => {
                trace!(target: "engine", "Precommit timeout.");
                self.increment_view(1);
                Step::Propose
            }
            Step::Precommit => {
                trace!(target: "engine", "Precommit timeout without enough votes.");
                self.broadcast_old_messages();
                Step::Precommit
            }
            Step::Commit => {
                trace!(target: "engine", "Commit timeout.");
                Step::Propose
            }
        };
        self.to_step(next_step);
    }

    fn register_client(&self, client: Weak<EngineClient>) {
        if let Some(c) = client.upgrade() {
            self.height.store(c.chain_info().best_block_number as usize + 1, AtomicOrdering::SeqCst);
        }
        *self.client.write() = Some(client.clone());
        self.extension.register_client(client.clone());
        self.validators.register_client(client);
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
                    recover: |signature: &ECDSASignature, message: &Message| {
                        Ok(public_to_address(&recover_ecdsa(&signature, &message)?))
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

    fn populate_from_parent(&self, header: &mut Header, parent: &Header) {
        // Chain scoring: total weight is sqrt(U256::max_value())*height - view
        let new_score = U256::from(U128::max_value())
            + consensus_view(parent).expect("Header has been verified; qed").into()
            - self.view.load(AtomicOrdering::SeqCst).into();

        header.set_score(new_score);
    }

    fn set_signer(&self, ap: Arc<AccountProvider>, address: Address) {
        {
            self.signer.write().set(ap, address);
        }
        self.to_step(Step::Propose);
    }

    fn sign(&self, hash: H256) -> Result<ECDSASignature, Error> {
        self.signer.read().sign(hash).map_err(Into::into)
    }

    fn stop(&self) {}

    fn is_proposal(&self, header: &Header) -> bool {
        let signatures_len = header.seal()[2].len();
        // Signatures have to be an empty list rlp.
        if signatures_len != 1 {
            // New Commit received, skip to next height.
            trace!(target: "engine", "Received a commit: {:?}.", header.number());
            self.to_next_height(header.number() as usize);
            self.to_step(Step::Commit);
            return false
        }
        let proposal = ConsensusMessage::new_proposal(header)
            .expect("block went through full verification; this Engine verifies new_proposal creation; qed");
        let proposer = proposal.verify().expect("block went through full verification; this Engine tries verify; qed");
        debug!(target: "engine", "Received a new proposal {:?} from {}.", proposal.vote_step, proposer);
        if self.is_view(&proposal) {
            *self.proposal.write() = proposal.block_hash.clone();
            *self.proposal_parent.write() = header.parent_hash().clone();
        }
        self.votes.vote(proposal, proposer);
        true
    }

    fn broadcast_proposal_block(&self, block: SealedBlock) {
        self.extension.broadcast_proposal_block(block.rlp_bytes());
    }

    fn network_extension(&self) -> Option<Arc<NetworkExtension>> {
        Some(Arc::clone(&self.extension) as Arc<NetworkExtension>)
    }
}

struct EpochVerifier<F>
where
    F: Fn(&ECDSASignature, &Message) -> Result<Address, Error> + Send + Sync, {
    subchain_validators: ValidatorList,
    recover: F,
}

impl<F> super::EpochVerifier<CodeChainMachine> for EpochVerifier<F>
where
    F: Fn(&ECDSASignature, &Message) -> Result<Address, Error> + Send + Sync,
{
    fn verify_light(&self, header: &Header) -> Result<(), Error> {
        let message = header.bare_hash();

        let mut addresses = HashSet::new();
        let ref header_signatures_field = header.seal().get(2).ok_or(BlockError::InvalidSeal)?;
        for rlp in UntrustedRlp::new(header_signatures_field).iter() {
            let signature: H520 = rlp.as_val()?;
            let address = (self.recover)(&signature.into(), &message)?;

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
            }).into())
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
        *self.client.write() = Some(client.clone());
    }

    fn select_random_peers(&self) -> Vec<NodeId> {
        let mut peers: Vec<NodeId> = self.peers.write().iter().cloned().collect();
        let mut count = (peers.len() as f64).powf(0.5).round() as usize;
        count = cmp::min(count, MAX_PEERS_PROPAGATION);
        count = cmp::max(count, MIN_PEERS_PROPAGATION);
        thread_rng().shuffle(&mut peers);
        peers.truncate(count);
        peers
    }

    fn broadcast_message(&self, message: Bytes) {
        let tokens = self.select_random_peers();
        let message = TendermintMessage::ConsensusMessage(message).rlp_bytes().into_vec();
        self.api.lock().as_ref().map(|api| {
            for token in tokens {
                api.send(&token, &message);
            }
        });
    }

    fn broadcast_proposal_block(&self, message: Bytes) {
        let message = TendermintMessage::ProposalBlock(message).rlp_bytes().into_vec();
        self.api.lock().as_ref().map(|api| {
            for token in self.peers.read().iter() {
                api.send(&token, &message);
            }
        });
    }

    fn send_local_message(&self, message: Step) {
        self.api.lock().as_ref().map(|api| {
            api.send_local_message(&message);
        });
    }

    fn register_tendermint(&self, tendermint: Weak<Tendermint>) {
        *self.tendermint.write() = Some(tendermint.clone());
    }
}

impl NetworkExtension for TendermintExtension {
    fn name(&self) -> String {
        String::from(EXTENSION_NAME)
    }

    fn need_encryption(&self) -> bool {
        false
    }

    fn on_initialize(&self, api: Arc<Api>) {
        let initial = self.timeouts.initial();
        trace!(target: "engine", "Setting the initial timeout to {}.", initial);
        api.set_timer_once(ENGINE_TIMEOUT_TOKEN, initial).expect("Timer set succeeds");
        *self.api.lock() = Some(api);
    }

    fn on_node_added(&self, token: &NodeId) {
        self.peers.write().insert(*token);
    }

    fn on_node_removed(&self, token: &NodeId) {
        self.peers.write().remove(token);
    }

    fn on_negotiated(&self, _token: &NodeId) {}

    fn on_negotiation_allowed(&self, token: &NodeId) {
        self.on_negotiated(token);
    }

    fn on_message(&self, token: &NodeId, data: &[u8]) {
        let m = UntrustedRlp::new(data);
        match m.as_val() {
            Ok(TendermintMessage::ConsensusMessage(ref bytes)) => {
                if let Some(ref weak) = *self.tendermint.read() {
                    if let Some(c) = weak.upgrade() {
                        if let Err(e) = c.handle_message(bytes) {
                            info!(target: "engine", "Failed to handle message {:?}", e);
                        }
                    }
                }
            }
            Ok(TendermintMessage::ProposalBlock(bytes)) => {
                if let Some(ref weak) = *self.client.read() {
                    if let Some(c) = weak.upgrade() {
                        if let Err(e) = c.import_block(bytes) {
                            info!(target: "engine", "Failed to import proposal block {:?}", e);
                        }
                    }
                }
            }
            _ => info!(target: "engine", "invalid message from peer {}", token),
        }
    }

    fn on_local_message(&self, data: &[u8]) {
        let next: Step = rlp::decode(data);
        self.api.lock().as_ref().map(|api| {
            api.clear_timer(ENGINE_TIMEOUT_TOKEN).expect("Timer clear succeeds");
            api.set_timer_once(ENGINE_TIMEOUT_TOKEN, self.timeouts.timeout(&next)).expect("Timer set succeeds");
        });
    }

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
    use super::super::super::error::{BlockError, Error};
    use super::super::super::header::Header;
    use super::super::super::spec::Spec;

    #[test]
    fn has_valid_metadata() {
        let engine = Spec::new_test_tendermint().engine;
        assert!(!engine.name().is_empty());
    }

    #[test]
    fn verification_fails_on_short_seal() {
        let engine = Spec::new_test_tendermint().engine;
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
}
