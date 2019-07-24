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
use std::mem;
use std::sync::{Arc, Weak};
use std::thread::{Builder, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ccrypto::blake256;
use ckey::{public_to_address, verify_schnorr, Address, SchnorrSignature};
use cnetwork::{EventSender, NodeId};
use crossbeam_channel as crossbeam;
use ctypes::transaction::{Action, Transaction};
use ctypes::util::unexpected::Mismatch;
use ctypes::{BlockNumber, Header};
use primitives::{u256_from_u128, Bytes, H256, U256};
use rlp::{Encodable, UntrustedRlp};

use super::super::vote_collector::DoubleVote;
use super::super::BitSet;
use super::backup::{backup, restore, BackupView};
use super::message::*;
use super::network;
use super::params::TimeGapParams;
use super::stake::CUSTOM_ACTION_HANDLER_ID;
use super::types::{Height, Proposal, Step, TendermintSealView, TendermintState, TwoThirdsMajority, View};
use super::{
    BlockHash, ENGINE_TIMEOUT_BROADCAST_STEP_STATE, ENGINE_TIMEOUT_EMPTY_PROPOSAL, ENGINE_TIMEOUT_TOKEN_NONCE_BASE,
    SEAL_FIELDS,
};
use crate::account_provider::AccountProvider;
use crate::block::*;
use crate::client::ConsensusClient;
use crate::consensus::signer::EngineSigner;
use crate::consensus::validator_set::{DynamicValidator, ValidatorSet};
use crate::consensus::vote_collector::{Message, VoteCollector};
use crate::consensus::{EngineError, Seal};
use crate::encoded;
use crate::error::{BlockError, Error};
use crate::transaction::{SignedTransaction, UnverifiedTransaction};
use crate::views::BlockView;
use crate::BlockId;

type SpawnResult = (
    JoinHandle<()>,
    crossbeam::Sender<TimeGapParams>,
    crossbeam::Sender<(crossbeam::Sender<network::Event>, Weak<ConsensusClient>)>,
    crossbeam::Sender<Event>,
    crossbeam::Sender<()>,
);

pub fn spawn(validators: Arc<DynamicValidator>) -> SpawnResult {
    Worker::spawn(validators)
}

struct Worker {
    client: Weak<ConsensusClient>,
    /// Blockchain height.
    height: Height,
    /// Consensus view.
    view: View,
    /// Consensus step.
    step: TendermintState,
    /// Record current round's received votes as bit set
    votes_received: BitSet,
    /// The votes_received field is changed after last state broadcast.
    votes_received_changed: bool,
    /// Vote accumulator.
    votes: VoteCollector<ConsensusMessage>,
    /// Used to sign messages and proposals.
    signer: EngineSigner,
    /// Last majority
    last_two_thirds_majority: TwoThirdsMajority,
    /// hash of the proposed block, used for seal submission.
    proposal: Proposal,
    /// The last confirmed view from the commit step.
    last_confirmed_view: View,
    /// Set used to determine the current validators.
    validators: Arc<DynamicValidator>,
    /// Channel to the network extension, must be set later.
    extension: EventSender<network::Event>,
    time_gap_params: TimeGapParams,
    timeout_token_nonce: usize,
}

pub enum Event {
    NewBlocks {
        imported: Vec<H256>,
        enacted: Vec<H256>,
    },
    GenerateSeal {
        block_number: Height,
        parent_hash: H256,
        result: crossbeam::Sender<Seal>,
    },
    ProposalGenerated(Box<SealedBlock>),
    VerifyHeaderBasic {
        header: Box<Header>,
        result: crossbeam::Sender<Result<(), Error>>,
    },
    VerifyBlockExternal {
        header: Box<Header>,
        result: crossbeam::Sender<Result<(), Error>>,
    },
    CalculateScore {
        block_number: Height,
        result: crossbeam::Sender<U256>,
    },
    OnTimeout(usize),
    HandleMessages {
        messages: Vec<Vec<u8>>,
        result: crossbeam::Sender<Result<(), EngineError>>,
    },
    IsProposal {
        block_number: BlockNumber,
        block_hash: H256,
        result: crossbeam::Sender<bool>,
    },
    SetSigner {
        ap: Arc<AccountProvider>,
        address: Address,
    },
    AllowedHeight {
        result: crossbeam::Sender<Height>,
    },
    Restore(crossbeam::Sender<()>),
    ProposalBlock {
        signature: SchnorrSignature,
        view: View,
        message: Bytes,
        result: crossbeam::Sender<Option<Arc<ConsensusClient>>>,
    },
    StepState {
        token: NodeId,
        vote_step: VoteStep,
        proposal: Option<H256>,
        lock_view: Option<View>,
        known_votes: Box<BitSet>,
        result: crossbeam::Sender<Bytes>,
    },
    RequestProposal {
        token: NodeId,
        height: Height,
        view: View,
        result: crossbeam::Sender<Bytes>,
    },
    GetAllVotesAndAuthors {
        vote_step: VoteStep,
        requested: BitSet,
        result: crossbeam::Sender<ConsensusMessage>,
    },
}

impl Worker {
    /// Create a new instance of Tendermint engine
    fn new(
        validators: Arc<DynamicValidator>,
        extension: EventSender<network::Event>,
        client: Weak<ConsensusClient>,
        time_gap_params: TimeGapParams,
    ) -> Self {
        Worker {
            client,
            height: 1,
            view: 0,
            step: TendermintState::Propose,
            votes: Default::default(),
            signer: Default::default(),
            last_two_thirds_majority: TwoThirdsMajority::Empty,
            proposal: Proposal::None,
            last_confirmed_view: 0,
            validators,
            extension,
            votes_received: BitSet::new(),
            votes_received_changed: false,
            time_gap_params,
            timeout_token_nonce: ENGINE_TIMEOUT_TOKEN_NONCE_BASE,
        }
    }

    fn spawn(validators: Arc<DynamicValidator>) -> SpawnResult {
        let (sender, receiver) = crossbeam::unbounded();
        let (quit, quit_receiver) = crossbeam::bounded(1);
        let (external_params_initializer, external_params_receiver) = crossbeam::bounded(1);
        let (extension_initializer, extension_receiver) = crossbeam::bounded(1);
        let join = Builder::new()
            .name("tendermint".to_string())
            .spawn(move || {
                let time_gap_params = crossbeam::select! {
                recv(external_params_receiver) -> msg => {
                match msg {
                    Ok(time_gap_params) => time_gap_params,
                    Err(crossbeam::RecvError) => {
                        cerror!(ENGINE, "The tendermint external parameters are not initialized");
                        return
                    }
                }
                }
                recv(quit_receiver) -> msg => {
                    match msg {
                        Ok(()) => {},
                        Err(crossbeam::RecvError) => {
                            cerror!(ENGINE, "The quit channel for tendermint thread had been closed.");
                        }
                    }
                    return
                }
                };
                let (extension, client) = crossbeam::select! {
                recv(extension_receiver) -> msg => {
                    match msg {
                        Ok((extension, client)) => (extension, client),
                        Err(crossbeam::RecvError) => {
                            cerror!(ENGINE, "The tendermint extension is not initalized.");
                            return
                        }
                    }
                }
                recv(quit_receiver) -> msg => {
                    match msg {
                        Ok(()) => {},
                        Err(crossbeam::RecvError) => {
                            cerror!(ENGINE, "The quit channel for tendermint thread had been closed.");
                        }
                    }
                    return
                }
                };
                validators.register_client(Weak::clone(&client));
                let mut inner = Self::new(validators, extension, client, time_gap_params);
                loop {
                    crossbeam::select! {
                    recv(receiver) -> msg => {
                        match msg {
                            Ok(Event::NewBlocks {
                                imported,
                                enacted,
                            }) => {
                                inner.new_blocks(imported, enacted);
                            }
                            Ok(Event::GenerateSeal {
                                block_number,
                                parent_hash,
                                result,
                            }) => {
                                let seal = inner.generate_seal(block_number, parent_hash);
                                result.send(seal).unwrap();
                            }
                            Ok(Event::ProposalGenerated(sealed)) => {
                                inner.proposal_generated(&*sealed);
                            }
                            Ok(Event::VerifyHeaderBasic{header, result}) => {
                                result.send(inner.verify_header_basic(&*header)).unwrap();
                            }
                            Ok(Event::VerifyBlockExternal{header, result, }) => {
                                result.send(inner.verify_block_external(&*header)).unwrap();
                            }
                            Ok(Event::CalculateScore {
                                block_number,
                                result,
                            }) => {
                                result.send(inner.calculate_score(block_number)).unwrap();
                            }
                            Ok(Event::OnTimeout(token)) => {
                                inner.on_timeout(token);
                            }
                            Ok(Event::HandleMessages {
                                messages,
                                result,
                            }) => {
                                for message in messages {
                                    result.send(inner.handle_message(&message, false)).unwrap();
                                }
                            }
                            Ok(Event::IsProposal {
                                block_number,
                                block_hash,
                                result,
                            }) => {
                                result.send(inner.is_proposal(block_number, block_hash)).unwrap();
                            }
                            Ok(Event::SetSigner {
                                ap,
                                address,
                            }) => {
                                inner.set_signer(ap, address);
                            }
                            Ok(Event::AllowedHeight {
                                result,
                            }) => {
                                let allowed_height = if inner.step.is_commit() {
                                    inner.height + 1
                                } else {
                                    inner.height
                                };
                                result.send(allowed_height).unwrap();
                            }
                            Ok(Event::Restore(result)) => {
                                inner.restore();
                                result.send(()).unwrap();
                            }
                            Ok(Event::ProposalBlock {
                                signature,
                                view,
                                message,
                                result,
                            }) => {
                                let client = inner.on_proposal_message(signature, view, message);
                                result.send(client).unwrap();
                            }
                            Ok(Event::StepState {
                                token, vote_step, proposal, lock_view, known_votes, result
                            }) => {
                                inner.on_step_state_message(&token, vote_step, proposal, lock_view, *known_votes, result);
                            }
                            Ok(Event::RequestProposal {
                                token,
                                height,
                                view,
                                result,
                            }) => {
                                inner.on_request_proposal_message(&token, height, view, result);
                            }
                            Ok(Event::GetAllVotesAndAuthors {
                                vote_step,
                                requested,
                                result,
                            }) => {
                                inner.get_all_votes_and_authors(&vote_step, &requested, result);
                            }
                            Err(crossbeam::RecvError) => {
                                cerror!(ENGINE, "The event channel for tendermint thread had been closed.");
                                break
                            }
                        }
                    }
                    recv(quit_receiver) -> msg => {
                        match msg {
                            Ok(()) => {},
                            Err(crossbeam::RecvError) => {
                                cerror!(ENGINE, "The quit channel for tendermint thread had been closed.");
                            }
                        }
                        break
                    }
                    }
                }
            })
            .unwrap();
        (join, external_params_initializer, extension_initializer, sender, quit)
    }

    /// The client is a thread-safe struct. Using it in multi-threads is safe.
    fn client(&self) -> Arc<ConsensusClient> {
        self.client.upgrade().expect("Client lives longer than consensus")
    }

    /// Get previous block hash to determine validator set
    fn prev_block_hash(&self) -> H256 {
        self.prev_block_header_of_height(self.height)
            .expect("Height is increased when previous block is imported")
            .hash()
    }

    /// Get the index of the proposer of a block to check the new proposer is valid.
    fn block_proposer_idx(&self, block_hash: H256) -> Option<usize> {
        self.client().block_header(&BlockId::Hash(block_hash)).map(|header| {
            let proposer = header.author();
            let parent = if header.number() == 0 {
                // Genesis block's parent is not exist
                // FIXME: The DynamicValidator should handle the Genesis block correctly.
                block_hash
            } else {
                header.parent_hash()
            };

            self.validators.get_index_by_address(&parent, &proposer).expect("The proposer must be in the validator set")
        })
    }

    /// Get previous block header of given height
    fn prev_block_header_of_height(&self, height: Height) -> Option<encoded::Header> {
        let prev_height = (height - 1) as u64;
        self.client().block_header(&BlockId::Number(prev_height))
    }

    /// Check the committed block of the current height is imported to the canonical chain
    fn check_current_block_exists(&self) -> bool {
        self.client().block(&BlockId::Number(self.height as u64)).is_some()
    }

    /// Check Tendermint can move from the commit step to the propose step
    fn can_move_from_commit_to_propose(&self) -> bool {
        let vote_step = VoteStep::new(self.height, self.last_confirmed_view, Step::Precommit);
        if self.step.is_commit_timedout() && self.check_current_block_exists() {
            cinfo!(ENGINE, "Transition to Propose because best block is changed after commit timeout");
            return true
        }

        if self.step.is_commit() && self.has_all_votes(&vote_step) && self.check_current_block_exists() {
            cinfo!(
                ENGINE,
                "Transition to Propose because all pre-commits are received and the canonical chain is appended"
            );
            return true
        }

        false
    }

    /// Find the designated for the given view.
    fn view_proposer(&self, prev_block_hash: &H256, view: View) -> Option<Address> {
        self.validators.next_block_proposer(prev_block_hash, view)
    }

    pub fn proposal_at(&self, height: Height, view: View) -> Option<(SchnorrSignature, usize, Bytes)> {
        let vote_step = VoteStep {
            height,
            view,
            step: Step::Propose,
        };

        let all_votes = self.votes.get_all_votes_in_round(&vote_step);
        let proposal = all_votes.first()?;

        let block_hash = proposal.on.block_hash.expect("Proposal message always include block hash");
        let bytes = self.client().block(&BlockId::Hash(block_hash))?.into_inner();
        Some((proposal.signature, proposal.signer_index, bytes))
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

    pub fn get_all_votes_and_authors(
        &self,
        vote_step: &VoteStep,
        requested: &BitSet,
        result: crossbeam::Sender<ConsensusMessage>,
    ) {
        for (_, vote) in self
            .votes
            .get_all_votes_and_indices_in_round(vote_step)
            .into_iter()
            .filter(|(index, _)| requested.is_set(*index))
        {
            result.send(vote).unwrap();
        }
    }

    /// Check if address is a proposer for given view.
    fn check_view_proposer(
        &self,
        parent: &H256,
        height: Height,
        view: View,
        address: &Address,
    ) -> Result<(), EngineError> {
        let proposer = self.view_proposer(parent, view).ok_or_else(|| EngineError::PrevBlockNotExist {
            height: height as u64,
        })?;
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
        self.view_proposer(bh, self.view).map_or(false, |proposer| self.signer.is_address(&proposer))
    }

    fn is_step(&self, message: &ConsensusMessage) -> bool {
        message.on.step.is_step(self.height, self.view, self.step.to_step())
    }

    fn is_authority(&self, prev_hash: &H256, address: &Address) -> bool {
        self.validators.contains_address(&prev_hash, address)
    }

    fn has_enough_any_votes(&self) -> bool {
        let step_votes = self.votes.round_votes(&VoteStep::new(self.height, self.view, self.step.to_step()));
        self.validators.check_enough_votes(&self.prev_block_hash(), &step_votes).is_ok()
    }

    fn has_all_votes(&self, vote_step: &VoteStep) -> bool {
        let step_votes = self.votes.round_votes(vote_step);
        self.validators.count(&self.prev_block_hash()) == step_votes.count()
    }

    fn has_enough_aligned_votes(&self, message: &ConsensusMessage) -> bool {
        let parent_hash = self
            .client()
            .block_header(&(message.on.step.height - 1).into())
            .expect("The parent of the vote must exist")
            .hash();
        let aligned_votes = self.votes.aligned_votes(&message);
        self.validators.check_enough_votes(&parent_hash, &aligned_votes).is_ok()
    }

    fn has_enough_precommit_votes(&self, block_hash: H256) -> bool {
        let vote_step = VoteStep::new(self.height, self.view, Step::Precommit);
        let votes = self.votes.block_round_votes(&vote_step, &Some(block_hash));
        self.validators.check_enough_votes(&self.prev_block_hash(), &votes).is_ok()
    }

    fn broadcast_message(&self, message: Bytes) {
        self.extension
            .send(network::Event::BroadcastMessage {
                message,
            })
            .unwrap();
    }

    fn broadcast_state(&self, vote_step: VoteStep, proposal: Option<H256>, lock_view: Option<View>, votes: BitSet) {
        self.extension
            .send(network::Event::BroadcastState {
                vote_step,
                proposal,
                lock_view,
                votes,
            })
            .unwrap();
    }

    fn request_messages_to_all(&self, vote_step: VoteStep, requested_votes: BitSet) {
        self.extension
            .send(network::Event::RequestMessagesToAll {
                vote_step,
                requested_votes,
            })
            .unwrap();
    }

    fn request_proposal_to_any(&self, height: Height, view: View) {
        self.extension
            .send(network::Event::RequestProposalToAny {
                height,
                view,
            })
            .unwrap();
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
        self.proposal = Proposal::None;
        self.votes_received = BitSet::new();
    }

    fn move_to_height(&mut self, height: Height) {
        assert!(height > self.height, "{} < {}", height, self.height);
        cinfo!(ENGINE, "Transitioning to height {}.", height);
        self.last_two_thirds_majority = TwoThirdsMajority::Empty;
        self.height = height;
        self.view = 0;
        self.proposal = Proposal::None;
        self.votes_received = BitSet::new();
    }

    fn move_to_step(&mut self, step: Step, is_restoring: bool) {
        let prev_step = mem::replace(&mut self.step, step.into());
        if !is_restoring {
            self.backup();
        }

        let expired_token_nonce = self.timeout_token_nonce;
        self.timeout_token_nonce += 1;
        self.extension
            .send(network::Event::SetTimerStep {
                step,
                view: self.view,
                expired_token_nonce,
            })
            .unwrap();
        let vote_step = VoteStep::new(self.height, self.view, step);

        // If there are not enough pre-votes or pre-commits,
        // move_to_step can be called with the same step
        // Also, when moving to the commit step,
        // keep `votes_received` for gossiping.
        if prev_step.to_step() != step && step != Step::Commit {
            self.votes_received = BitSet::new();
        }

        // need to reset vote
        self.broadcast_state(
            vote_step,
            self.proposal.block_hash(),
            self.last_two_thirds_majority.view(),
            self.votes_received,
        );
        match step {
            Step::Propose => {
                cinfo!(ENGINE, "move_to_step: Propose.");
                if let Some(hash) = self.votes.get_block_hashes(&vote_step).first() {
                    if self.client().block(&BlockId::Hash(*hash)).is_some() {
                        self.proposal = Proposal::new_imported(*hash);
                        self.move_to_step(Step::Prevote, is_restoring);
                    } else {
                        cwarn!(ENGINE, "Proposal is received but not imported");
                        // Proposal is received but is not verified yet.
                        // Wait for verification.
                        return
                    }
                } else {
                    let parent_block_hash = self.prev_block_hash();
                    if self.is_signer_proposer(&parent_block_hash) {
                        if let TwoThirdsMajority::Lock(lock_view, _) = self.last_two_thirds_majority {
                            cinfo!(ENGINE, "I am a proposer, I'll re-propose a locked block");
                            match self.locked_proposal_block(lock_view) {
                                Ok(block) => self.repropose_block(block),
                                Err(error_msg) => cwarn!(ENGINE, "{}", error_msg),
                            }
                        } else {
                            cinfo!(ENGINE, "I am a proposer, I'll create a block");
                            self.update_sealing(parent_block_hash);
                            self.step = TendermintState::ProposeWaitBlockGeneration {
                                parent_hash: parent_block_hash,
                            };
                        }
                    } else {
                        self.request_proposal_to_any(vote_step.height, vote_step.view);
                    }
                }
            }
            Step::Prevote => {
                cinfo!(ENGINE, "move_to_step: Prevote.");
                // If the number of the collected prevotes is less than 2/3,
                // move_to_step called with again with the Prevote.
                // In the case, self.votes_received is not empty.
                self.request_messages_to_all(vote_step, &BitSet::all_set() - &self.votes_received);
                if !self.already_generated_message() {
                    let block_hash_candidate = match &self.last_two_thirds_majority {
                        TwoThirdsMajority::Empty => self.proposal.imported_block_hash(),
                        TwoThirdsMajority::Unlock(_) => self.proposal.imported_block_hash(),
                        TwoThirdsMajority::Lock(_, block_hash) => Some(*block_hash),
                    };
                    let block_hash = block_hash_candidate.filter(|hash| {
                        let block = match self.client().block(&BlockId::Hash(*hash)) {
                            Some(block) => block,
                            // When a node locks on a proposal and doesn't imported the proposal yet,
                            // we could not check the proposal's generated time.
                            // To make the network healthier in the corner case, we send a prevote message to the locked block.
                            None => return true,
                        };
                        self.is_generation_time_relevant(&block.decode_header())
                    });
                    self.generate_and_broadcast_message(block_hash, is_restoring);
                }
            }
            Step::Precommit => {
                cinfo!(ENGINE, "move_to_step: Precommit.");
                // If the number of the collected precommits is less than 2/3,
                // move_to_step called with again with the Precommit.
                // In the case, self.votes_received is not empty.
                self.request_messages_to_all(vote_step, &BitSet::all_set() - &self.votes_received);
                if !self.already_generated_message() {
                    let block_hash = match &self.last_two_thirds_majority {
                        TwoThirdsMajority::Empty => None,
                        TwoThirdsMajority::Unlock(_) => None,
                        TwoThirdsMajority::Lock(locked_view, block_hash) => {
                            if locked_view == &self.view {
                                Some(*block_hash)
                            } else {
                                None
                            }
                        }
                    };
                    self.generate_and_broadcast_message(block_hash, is_restoring);
                }
            }
            Step::Commit => {
                cinfo!(ENGINE, "move_to_step: Commit.");
            }
        }
    }

    fn is_generation_time_relevant(&self, block_header: &Header) -> bool {
        let acceptable_past_gap = self.time_gap_params.allowed_past_gap;
        let acceptable_future_gap = self.time_gap_params.allowed_future_gap;
        let now = SystemTime::now();
        let allowed_min = now - acceptable_past_gap;
        let allowed_max = now + acceptable_future_gap;
        let block_generation_time = UNIX_EPOCH.checked_add(Duration::from_secs(block_header.timestamp()));

        match block_generation_time {
            Some(generation_time) => generation_time <= allowed_max && allowed_min <= generation_time,
            // Overflow occurred
            None => false,
        }
    }

    fn locked_proposal_block(&self, locked_view: View) -> Result<encoded::Block, String> {
        let vote_step = VoteStep::new(self.height, locked_view, Step::Propose);
        let locked_proposal_hash = self.votes.get_block_hashes(&vote_step).first().cloned();

        let locked_proposal_hash = locked_proposal_hash.ok_or_else(|| {
            self.request_proposal_to_any(self.height, locked_view);
            format!("Have a lock on {}-{}, but do not received a locked proposal", self.height, locked_view)
        })?;

        let locked_proposal_block = self.client().block(&BlockId::Hash(locked_proposal_hash)).ok_or_else(|| {
            format!(
                "Have a lock on {}-{}, and received the locked proposal, but the proposal is not imported yet.",
                self.height, locked_view
            )
        })?;

        Ok(locked_proposal_block)
    }

    fn already_generated_message(&self) -> bool {
        match self.signer_index() {
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
        let signer_index = self.signer_index().or_else(|| {
            ctrace!(ENGINE, "No message, since there is no engine signer.");
            None
        })?;
        let signature = self
            .sign(blake256(&vote_info))
            .map_err(|error| {
                ctrace!(ENGINE, "{}th validator could not sign the message {}", signer_index, error);
                error
            })
            .ok()?;
        let message = ConsensusMessage {
            signature,
            signer_index,
            on,
        };
        self.votes_received.set(signer_index);
        self.votes.vote(message.clone());
        cinfo!(ENGINE, "Generated {:?} as {}th validator.", message, signer_index);
        self.handle_valid_message(&message, is_restoring);

        Some(message.rlp_bytes().into_vec())
    }

    fn handle_valid_message(&mut self, message: &ConsensusMessage, is_restoring: bool) {
        let vote_step = &message.on.step;
        let is_newer_than_lock = match self.last_two_thirds_majority.view() {
            Some(last_lock) => vote_step.view > last_lock,
            None => true,
        };
        let has_enough_aligned_votes = self.has_enough_aligned_votes(message);
        let lock_change = is_newer_than_lock
            && vote_step.height == self.height
            && vote_step.step == Step::Prevote
            && has_enough_aligned_votes;
        if lock_change {
            cinfo!(
                ENGINE,
                "handle_valid_message: Lock change to {}-{}-{:?} at {}-{}-{:?}",
                vote_step.height,
                vote_step.view,
                message.on.block_hash,
                self.height,
                self.view,
                self.last_two_thirds_majority.block_hash(),
            );
            self.last_two_thirds_majority =
                TwoThirdsMajority::from_message(message.on.step.view, message.on.block_hash);
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
            let height = self.height;
            self.move_to_height(height + 1);
            self.move_to_step(Step::Propose, is_restoring);
            return
        }

        // self.move_to_step() calls self.broadcast_state()
        // If self.move_to_step() is not called, call self.broadcast_state() in here.
        self.votes_received_changed = true;
    }

    pub fn on_imported_proposal(&mut self, proposal: &Header) {
        if proposal.number() < 1 {
            return
        }

        let height = proposal.number() as Height;
        let seal_view = TendermintSealView::new(proposal.seal());
        let prev_block_view = seal_view.previous_block_view().expect("The proposal is verified");
        let on = VoteOn {
            step: VoteStep::new(height - 1, prev_block_view, Step::Precommit),
            block_hash: Some(*proposal.parent_hash()),
        };
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
            height: proposal.number() - 1,
            view: 0,
            step: Step::Propose,
        });

        let current_height = self.height;
        let vote_step = VoteStep::new(self.height, self.view, self.step.to_step());
        let proposal_at_current_view = self.votes.get_block_hashes(&vote_step).first().cloned();
        if proposal_at_current_view == Some(proposal.hash()) {
            self.proposal = Proposal::new_imported(proposal.hash());
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
                        self.extension
                            .send(network::Event::SetTimerEmptyProposal {
                                view: self.view,
                            })
                            .unwrap();
                    }
                }
                TendermintState::ProposeWaitEmptyBlockTimer {
                    ..
                } => unreachable!(),
                _ => {}
            };
        } else if current_height < height {
            self.move_to_height(height);
            let prev_block_view = TendermintSealView::new(proposal.seal()).previous_block_view().unwrap();
            self.save_last_confirmed_view(prev_block_view);
            let proposal_at_view_0 = self
                .votes
                .get_block_hashes(&VoteStep {
                    height,
                    view: 0,
                    step: Step::Propose,
                })
                .first()
                .cloned();
            if proposal_at_view_0 == Some(proposal.hash()) {
                self.proposal = Proposal::new_imported(proposal.hash())
            }
            self.move_to_step(Step::Prevote, false);
        }
    }

    fn submit_proposal_block(&mut self, sealed_block: &SealedBlock) {
        cinfo!(ENGINE, "Submitting proposal block {}", sealed_block.header().hash());
        self.move_to_step(Step::Prevote, false);
        self.broadcast_proposal_block(self.view, encoded::Block::new(sealed_block.rlp_bytes()));
    }

    fn backup(&self) {
        backup(self.client().get_kvdb().as_ref(), BackupView {
            height: &self.height,
            view: &self.view,
            step: &self.step.to_step(),
            votes: &self.votes.get_all(),
            last_confirmed_view: &self.last_confirmed_view,
        });
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
                if client.block(&BlockId::Hash(proposal)).is_some() {
                    self.proposal = Proposal::ProposalImported(proposal);
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

    fn seal_fields(&self) -> usize {
        SEAL_FIELDS
    }

    fn generate_seal(&self, height: Height, parent_hash: H256) -> Seal {
        // Block is received from other nodes while creating a block
        if height < self.height {
            return Seal::None
        }

        assert!(self.is_signer_proposer(&parent_hash));
        assert_eq!(Proposal::None, self.proposal);
        assert_eq!(height, self.height);

        let view = self.view;

        let last_block_view = &self.last_confirmed_view;
        assert_eq!(self.prev_block_hash(), parent_hash);

        let (precommits, precommit_indices) = self
            .votes
            .round_signatures_and_indices(&VoteStep::new(height - 1, *last_block_view, Step::Precommit), &parent_hash);
        ctrace!(ENGINE, "Collected seal: {:?}({:?})", precommits, precommit_indices);
        let precommit_bitset = BitSet::new_with_indices(&precommit_indices);
        Seal::Tendermint {
            prev_view: *last_block_view,
            cur_view: view,
            precommits,
            precommit_bitset,
        }
    }

    fn proposal_generated(&mut self, sealed_block: &SealedBlock) {
        let header = sealed_block.header();
        let hash = header.hash();
        let parent_hash = header.parent_hash();

        if let TendermintState::ProposeWaitBlockGeneration {
            parent_hash: expected_parent_hash,
        } = self.step
        {
            assert_eq!(
                *parent_hash, expected_parent_hash,
                "Generated hash({:?}) is different from expected({:?})",
                parent_hash, expected_parent_hash
            );
        } else {
            panic!("Block is generated at unexpected step {:?}", self.step);
        }
        let prev_proposer_idx = self.block_proposer_idx(*parent_hash).expect("Prev block must exists");

        debug_assert_eq!(Ok(self.view), TendermintSealView::new(header.seal()).consensus_view());

        let vote_step = VoteStep::new(header.number() as Height, self.view, Step::Propose);
        let vote_info = message_info_rlp(vote_step, Some(hash));
        let signature = self.sign(blake256(&vote_info)).expect("I am proposer");
        self.votes.vote(
            ConsensusMessage::new_proposal(signature, &*self.validators, header, self.view, prev_proposer_idx)
                .expect("I am proposer"),
        );

        self.step = TendermintState::ProposeWaitImported {
            block: Box::new(sealed_block.clone()),
        };
    }

    fn verify_header_basic(&self, header: &Header) -> Result<(), Error> {
        let seal_length = header.seal().len();
        let expected_seal_fields = self.seal_fields();
        if seal_length != expected_seal_fields {
            return Err(BlockError::InvalidSealArity(Mismatch {
                expected: expected_seal_fields,
                found: seal_length,
            })
            .into())
        }

        let height = header.number();
        let view = TendermintSealView::new(header.seal()).consensus_view().unwrap();
        let score = calculate_score(height, view);

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
        let view = TendermintSealView::new(header.seal()).consensus_view()?;
        ctrace!(ENGINE, "Verify external at {}-{}, {:?}", height, view, header);
        let proposer = header.author();
        if !self.is_authority(header.parent_hash(), proposer) {
            return Err(EngineError::BlockNotAuthorized(*proposer).into())
        }
        self.check_view_proposer(header.parent_hash(), header.number(), view, &proposer)?;
        let seal_view = TendermintSealView::new(header.seal());
        let bitset_count = seal_view.bitset()?.count();
        let precommits_count = seal_view.precommits().item_count()?;

        if bitset_count < precommits_count {
            cwarn!(
                ENGINE,
                "verify_block_external: The header({})'s bitset count is less than the precommits count",
                header.hash()
            );
            return Err(BlockError::InvalidSeal.into())
        }

        if bitset_count > precommits_count {
            cwarn!(
                ENGINE,
                "verify_block_external: The header({})'s bitset count is greater than the precommits count",
                header.hash()
            );
            return Err(BlockError::InvalidSeal.into())
        }

        let previous_block_view = TendermintSealView::new(header.seal()).previous_block_view()?;
        let step = VoteStep::new(header.number() - 1, previous_block_view, Step::Precommit);
        let precommit_hash = message_hash(step, *header.parent_hash());

        let mut voted_validators = BitSet::new();
        let grand_parent_hash = self
            .client()
            .block_header(&(*header.parent_hash()).into())
            .expect("The parent block must exist")
            .parent_hash();
        for (bitset_index, signature) in seal_view.signatures()? {
            let public = self.validators.get(&grand_parent_hash, bitset_index);
            if !verify_schnorr(&public, &signature, &precommit_hash)? {
                let address = public_to_address(&public);
                return Err(EngineError::BlockNotAuthorized(address.to_owned()).into())
            }
            assert!(!voted_validators.is_set(bitset_index), "Double vote");
            voted_validators.set(bitset_index);
        }

        // Genesisblock does not have signatures
        if header.number() == 1 {
            return Ok(())
        }
        self.validators.check_enough_votes(&grand_parent_hash, &voted_validators)?;
        Ok(())
    }

    fn calculate_score(&self, block_number: Height) -> U256 {
        calculate_score(block_number, self.view)
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

        if token == ENGINE_TIMEOUT_BROADCAST_STEP_STATE {
            if self.votes_received_changed {
                self.votes_received_changed = false;
                self.broadcast_state(
                    self.vote_step(),
                    self.proposal.block_hash(),
                    self.last_two_thirds_majority.view(),
                    self.votes_received,
                );
            }
            return
        }

        // Timeout from Tendermint step
        if self.is_expired_timeout_token(token) {
            return
        }

        let next_step = match self.step {
            TendermintState::Propose => {
                cinfo!(ENGINE, "Propose timeout.");
                if self.proposal.is_none() {
                    // Report the proposer if no proposal was received.
                    let height = self.height;
                    let current_proposer = self
                        .view_proposer(&self.prev_block_hash(), self.view)
                        .expect("Height is increased when previous block is imported");
                    self.validators.report_benign(&current_proposer, height as BlockNumber, height as BlockNumber);
                }
                Step::Prevote
            }
            TendermintState::ProposeWaitBlockGeneration {
                ..
            } => {
                cwarn!(ENGINE, "Propose timed out but block is not generated yet");
                return
            }
            TendermintState::ProposeWaitImported {
                ..
            } => {
                cwarn!(ENGINE, "Propose timed out but still waiting for the block imported");
                return
            }
            TendermintState::ProposeWaitEmptyBlockTimer {
                ..
            } => {
                cwarn!(ENGINE, "Propose timed out but still waiting for the empty block");
                return
            }
            TendermintState::Prevote if self.has_enough_any_votes() => {
                cinfo!(ENGINE, "Prevote timeout.");
                Step::Precommit
            }
            TendermintState::Prevote => {
                cinfo!(ENGINE, "Prevote timeout without enough votes.");
                Step::Prevote
            }
            TendermintState::Precommit if self.has_enough_any_votes() => {
                cinfo!(ENGINE, "Precommit timeout.");
                self.increment_view(1);
                Step::Propose
            }
            TendermintState::Precommit => {
                cinfo!(ENGINE, "Precommit timeout without enough votes.");
                Step::Precommit
            }
            TendermintState::Commit => {
                cinfo!(ENGINE, "Commit timeout.");
                if !self.check_current_block_exists() {
                    cwarn!(ENGINE, "Best chain is not updated yet, wait until imported");
                    self.step = TendermintState::CommitTimedout;
                    return
                }
                let height = self.height;
                self.move_to_height(height + 1);
                Step::Propose
            }
            TendermintState::CommitTimedout => unreachable!(),
        };

        self.move_to_step(next_step, false);
    }

    fn is_expired_timeout_token(&self, nonce: usize) -> bool {
        nonce < self.timeout_token_nonce
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
                    .get_index(&prev_block_hash, &sender_public)
                    .expect("is_authority already checked the existence");
                self.votes_received.set(vote_index);
            }

            if let Some(double) = self.votes.vote(message.clone()) {
                let height = message.on.step.height as BlockNumber;
                cerror!(ENGINE, "Double vote found {:?}", double);
                self.report_double_vote(&double);
                self.validators.report_malicious(&sender, height, height, ::rlp::encode(&double).into_vec());
                return Err(EngineError::DoubleVote(sender))
            }
            ctrace!(ENGINE, "Handling a valid {:?} from {}.", message, sender);
            self.handle_valid_message(&message, is_restoring);
        }
        Ok(())
    }

    fn report_double_vote(&self, double: &DoubleVote<ConsensusMessage>) {
        let network_id = self.client().common_params(BlockId::Latest).unwrap().network_id();
        let seq = match self.signer.address() {
            Some(address) => self.client().latest_seq(address),
            None => {
                cerror!(ENGINE, "Found double vote, but signer was not assigned yet");
                return
            }
        };

        let tx = Transaction {
            seq,
            fee: 0,
            network_id,
            action: Action::Custom {
                handler_id: CUSTOM_ACTION_HANDLER_ID,
                bytes: double.to_action().rlp_bytes().into_vec(),
            },
        };
        let signature = match self.signer.sign_ecdsa(tx.hash()) {
            Ok(signature) => signature,
            Err(e) => {
                cerror!(ENGINE, "Found double vote, but could not sign the message: {}", e);
                return
            }
        };
        let unverified = UnverifiedTransaction::new(tx, signature);
        let signed = SignedTransaction::try_new(unverified).expect("secret is valid so it's recoverable");

        match self.client().queue_own_transaction(signed) {
            Ok(_) => {}
            Err(e) => {
                cerror!(ENGINE, "Failed to queue double vote transaction: {}", e);
            }
        }
    }

    fn is_proposal(&self, block_number: BlockNumber, block_hash: H256) -> bool {
        if self.height > block_number {
            return false
        }

        // if next header is imported, current header is not a proposal
        if self
            .client()
            .block_header(&BlockId::Number(block_number + 1))
            .map_or(false, |next| next.parent_hash() == block_hash)
        {
            return false
        }

        !self.has_enough_precommit_votes(block_hash)
    }

    fn repropose_block(&mut self, block: encoded::Block) {
        let header = block.decode_header();
        let vote_step = VoteStep::new(header.number() as Height, self.view, Step::Propose);
        let vote_info = message_info_rlp(vote_step, Some(header.hash()));
        let parent_hash = header.parent_hash();
        let prev_proposer_idx = self.block_proposer_idx(*parent_hash).expect("Prev block must exists");
        let signature = self.sign(blake256(&vote_info)).expect("I am proposer");
        self.votes.vote(
            ConsensusMessage::new_proposal(signature, &*self.validators, &header, self.view, prev_proposer_idx)
                .expect("I am proposer"),
        );

        self.proposal = Proposal::new_imported(header.hash());
        self.broadcast_proposal_block(self.view, block);
    }

    fn broadcast_proposal_block(&self, view: View, block: encoded::Block) {
        let header = block.decode_header();
        let hash = header.hash();
        let parent_hash = header.parent_hash();
        let vote_step = VoteStep::new(header.number() as Height, view, Step::Propose);
        cdebug!(ENGINE, "Send proposal {:?}", vote_step);

        assert!(self.is_signer_proposer(&parent_hash));

        let signature = self.votes.round_signature(&vote_step, &hash).expect("Proposal vote is generated before");
        self.extension
            .send(network::Event::BroadcastProposalBlock {
                signature,
                view,
                message: block.into_inner(),
            })
            .unwrap();
    }

    fn set_signer(&mut self, ap: Arc<AccountProvider>, address: Address) {
        self.signer.set_to_keep_decrypted_account(ap, address);
    }

    fn sign(&self, hash: H256) -> Result<SchnorrSignature, Error> {
        self.signer.sign(hash).map_err(Into::into)
    }

    fn signer_index(&self) -> Option<usize> {
        let parent = self.prev_block_hash();
        // FIXME: More effecient way to find index
        self.signer.public().and_then(|public| self.validators.get_index(&parent, public))
    }

    fn new_blocks(&mut self, imported: Vec<H256>, enacted: Vec<H256>) {
        let c = match self.client.upgrade() {
            Some(client) => client,
            None => {
                cdebug!(ENGINE, "NewBlocks event before the client is registered");
                return
            }
        };

        if !imported.is_empty() {
            let mut height_changed = false;
            for hash in imported {
                // New Commit received, skip to next height.
                let header = c.block_header(&hash.into()).expect("ChainNotify is called after the block is imported");

                let full_header = header.decode();
                if self.is_proposal(full_header.number(), full_header.hash()) {
                    self.on_imported_proposal(&full_header);
                } else if self.height < header.number() {
                    height_changed = true;
                    cinfo!(ENGINE, "Received a commit: {:?}.", header.number());
                    let prev_block_view = TendermintSealView::new(full_header.seal())
                        .previous_block_view()
                        .expect("Imported block already checked");
                    self.move_to_height(header.number());
                    self.save_last_confirmed_view(prev_block_view);
                }
            }
            if height_changed {
                self.move_to_step(Step::Propose, false);
                return
            }
        }
        if !enacted.is_empty() && self.can_move_from_commit_to_propose() {
            let new_height = self.height + 1;
            self.move_to_height(new_height);
            self.move_to_step(Step::Propose, false)
        }
    }

    fn send_proposal_block(
        &self,
        signature: SchnorrSignature,
        view: View,
        message: Bytes,
        result: crossbeam::Sender<Bytes>,
    ) {
        let message = TendermintMessage::ProposalBlock {
            signature,
            message,
            view,
        }
        .rlp_bytes()
        .into_vec();
        result.send(message).unwrap();
    }

    fn send_request_messages(
        &self,
        token: &NodeId,
        vote_step: VoteStep,
        requested_votes: BitSet,
        result: &crossbeam::Sender<Bytes>,
    ) {
        ctrace!(ENGINE, "Request messages {:?} {:?} to {:?}", vote_step, requested_votes, token);
        let message = TendermintMessage::RequestMessage {
            vote_step,
            requested_votes,
        }
        .rlp_bytes()
        .into_vec();
        result.send(message).unwrap();
    }

    fn send_request_proposal(&self, token: &NodeId, height: Height, view: View, result: &crossbeam::Sender<Bytes>) {
        ctrace!(ENGINE, "Request proposal {} {} to {:?}", height, view, token);
        let message = TendermintMessage::RequestProposal {
            height,
            view,
        }
        .rlp_bytes()
        .into_vec();
        result.send(message).unwrap();
    }

    fn on_proposal_message(
        &mut self,
        signature: SchnorrSignature,
        proposed_view: View,
        bytes: Bytes,
    ) -> Option<Arc<ConsensusClient>> {
        let c = self.client.upgrade()?;

        // This block borrows bytes
        {
            let block_view = BlockView::new(&bytes);
            let header_view = block_view.header();
            let number = header_view.number();
            cinfo!(ENGINE, "Proposal received for {}-{:?}", number, header_view.hash());

            let parent_hash = header_view.parent_hash();
            {
                if c.block(&BlockId::Hash(*parent_hash)).is_none() {
                    let best_block_number = c.best_block_header().number();
                    ctrace!(
                        ENGINE,
                        "Received future proposal {}-{}, current best block number is {}. ignore it",
                        number,
                        parent_hash,
                        best_block_number
                    );
                    return None
                }
            }

            let prev_proposer_idx = match self.block_proposer_idx(*parent_hash) {
                Some(idx) => idx,
                None => {
                    cwarn!(ENGINE, "Prev block proposer does not exist for height {}", number);
                    return None
                }
            };

            let message = ConsensusMessage::new_proposal(
                signature,
                &*self.validators,
                &header_view,
                proposed_view,
                prev_proposer_idx,
            )
            .map_err(|err| {
                cwarn!(ENGINE, "Invalid proposal received: {:?}", err);
            })
            .ok()?;

            // If the proposal's height is current height + 1 and the proposal has valid precommits,
            // we should import it and increase height
            if number > (self.height + 1) as u64 {
                ctrace!(ENGINE, "Received future proposal, ignore it");
                return None
            }

            if number == self.height as u64 && proposed_view > self.view {
                ctrace!(ENGINE, "Received future proposal, ignore it");
                return None
            }

            let signer_public = self.validators.get(&parent_hash, message.signer_index);
            match message.verify(&signer_public) {
                Ok(false) => {
                    cwarn!(ENGINE, "Proposal verification failed: signer is different");
                    return None
                }
                Err(err) => {
                    cwarn!(ENGINE, "Proposal verification failed: {:?}", err);
                    return None
                }
                _ => {}
            }

            if self.votes.is_old_or_known(&message) {
                cdebug!(ENGINE, "Proposal is already known");
                return None
            }

            if number == self.height as u64 && proposed_view == self.view {
                // The proposer re-proposed its locked proposal.
                // If we already imported the proposal, we should set `proposal` here.
                if c.block(&BlockId::Hash(header_view.hash())).is_some() {
                    let generated_view = TendermintSealView::new(header_view.seal())
                        .consensus_view()
                        .expect("Imported block is verified");
                    cdebug!(
                        ENGINE,
                        "Received a proposal({}) by a locked proposer. current view: {}, original proposal's view: {}",
                        header_view.hash(),
                        proposed_view,
                        generated_view
                    );
                    self.proposal = Proposal::new_imported(header_view.hash());
                } else {
                    self.proposal = Proposal::new_received(header_view.hash(), bytes.clone(), signature);
                }
                self.broadcast_state(
                    VoteStep::new(self.height, self.view, self.step.to_step()),
                    self.proposal.block_hash(),
                    self.last_two_thirds_majority.view(),
                    self.votes_received,
                );
            }

            self.votes.vote(message);
        }

        Some(c)
    }

    fn on_step_state_message(
        &self,
        token: &NodeId,
        peer_vote_step: VoteStep,
        peer_proposal: Option<H256>,
        peer_lock_view: Option<View>,
        peer_known_votes: BitSet,
        result: crossbeam::Sender<Bytes>,
    ) {
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

        if self.height > peer_vote_step.height {
            // no messages to receive
            return
        }

        // Since the peer may not have a proposal in its height,
        // we may need to receive votes instead of block.
        if self.height + 2 < peer_vote_step.height {
            // need to get blocks from block sync
            return
        }

        let peer_has_proposal = (self.view == peer_vote_step.view && peer_proposal.is_some())
            || self.view < peer_vote_step.view
            || self.height < peer_vote_step.height;

        let need_proposal = self.need_proposal();
        if need_proposal && peer_has_proposal {
            self.send_request_proposal(token, self.height, self.view, &result);
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

            let current_votes = self.votes_received;
            let difference = &peer_known_votes - &current_votes;
            if !difference.is_empty() {
                self.send_request_messages(token, current_vote_step, difference, &result);
            }
        }

        if peer_vote_step.height == self.height {
            match (self.last_two_thirds_majority.view(), peer_lock_view) {
                (None, Some(peer_lock_view)) if peer_lock_view < self.view => {
                    ctrace!(
                        ENGINE,
                        "Peer has a two thirds majority prevotes on {}-{} but I don't have it",
                        peer_vote_step.height,
                        peer_lock_view
                    );
                    self.send_request_messages(
                        token,
                        VoteStep {
                            height: self.height,
                            view: peer_lock_view,
                            step: Step::Prevote,
                        },
                        BitSet::all_set(),
                        &result,
                    );
                }
                (Some(my_lock_view), Some(peer_lock_view))
                    if my_lock_view < peer_lock_view && peer_lock_view < self.view =>
                {
                    ctrace!(
                        ENGINE,
                        "Peer has a two thirds majority prevotes on {}-{} which is newer than mine {}",
                        peer_vote_step.height,
                        peer_lock_view,
                        my_lock_view
                    );
                    self.send_request_messages(
                        token,
                        VoteStep {
                            height: self.height,
                            view: peer_lock_view,
                            step: Step::Prevote,
                        },
                        BitSet::all_set(),
                        &result,
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
        token: &NodeId,
        request_height: Height,
        request_view: View,
        result: crossbeam::Sender<Bytes>,
    ) {
        if request_height > self.height {
            return
        }

        if request_height == self.height && request_view > self.view {
            return
        }

        if let Some((signature, _signer_index, block)) = self.proposal_at(request_height, request_view) {
            ctrace!(ENGINE, "Send proposal {}-{} to {:?}", request_height, request_view, token);
            self.send_proposal_block(signature, request_view, block, result);
            return
        }

        if request_height == self.height && request_view == self.view {
            if let Proposal::ProposalReceived(_hash, block, signature) = &self.proposal {
                self.send_proposal_block(*signature, request_view, block.clone(), result);
            }
        }
    }
}

fn calculate_score(height: Height, view: View) -> U256 {
    let height = U256::from(height);
    u256_from_u128(std::u128::MAX) * height - view
}
