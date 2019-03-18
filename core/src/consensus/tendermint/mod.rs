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
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};
use std::thread::{Builder, JoinHandle};

use ccrypto::blake256;
use ckey::{public_to_address, recover_schnorr, verify_schnorr, Address, Message, SchnorrSignature};
use cnetwork::{Api, EventSender, NetworkExtension, NetworkService, NodeId};
use crossbeam_channel as crossbeam;
use cstate::ActionHandler;
use ctimer::TimerToken;
use ctypes::machine::WithBalances;
use ctypes::util::unexpected::{Mismatch, OutOfBounds};
use ctypes::BlockNumber;
use parking_lot::RwLock;
use primitives::{u256_from_u128, Bytes, H256, U256};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use self::backup::{backup, restore, BackupView};
use self::message::*;
pub use self::params::{TendermintParams, TimeoutParams};
use self::types::{BitSet, Height, PeerState, Step, TendermintSealView, TendermintState, TwoThirdsMajority, View};
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
/// Timer token for broadcasting step state.
const ENGINE_TIMEOUT_BROADCAST_STEP_STATE: TimerToken = 21;

/// Unit: second
const ENGINE_TIMEOUT_BROADCAT_STEP_STATE_INTERVAL: i64 = 1;

pub type BlockHash = H256;

/// ConsensusEngine using `Tendermint` consensus algorithm
pub struct Tendermint {
    client: RwLock<Option<Weak<EngineClient>>>,
    extension_initializer: crossbeam::Sender<(crossbeam::Sender<ExtensionEvent>, Weak<EngineClient>)>,
    timeouts: TimeoutParams,
    join: Option<JoinHandle<()>>,
    quit_tendermint: crossbeam::Sender<()>,
    inner: crossbeam::Sender<InnerEvent>,
    /// Set used to determine the current validators.
    validators: Arc<ValidatorSet>,
    /// Reward per block, in base units.
    block_reward: u64,
    /// codechain machine descriptor
    machine: Arc<CodeChainMachine>,
    /// Action handlers for this consensus method
    action_handlers: Vec<Arc<ActionHandler>>,
    /// Chain notify
    chain_notify: Arc<TendermintChainNotify>,
    has_signer: AtomicBool,
}

impl Drop for Tendermint {
    fn drop(&mut self) {
        self.quit_tendermint.send(()).unwrap();
        if let Some(handler) = self.join.take() {
            handler.join().unwrap();
        }
    }
}

struct TendermintInner {
    client: Weak<EngineClient>,
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
    proposal: Option<H256>,
    /// The last confirmed view from the commit step.
    last_confirmed_view: View,
    /// Set used to determine the current validators.
    validators: Arc<ValidatorSet>,
    /// Channel to the network extension, must be set later.
    extension: EventSender<ExtensionEvent>,

    timeout_token_nonce: usize,
}

impl Tendermint {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    /// Create a new instance of Tendermint engine
    pub fn new(our_params: TendermintParams, machine: CodeChainMachine) -> Arc<Self> {
        let stake = stake::Stake::new(our_params.genesis_stakes, Arc::clone(&our_params.validators));
        let timeouts = our_params.timeouts;
        let validators = Arc::clone(&our_params.validators);
        let machine = Arc::new(machine);

        let (join, extension_initializer, inner, quit_tendermint) = TendermintInner::spawn(our_params.validators);
        let action_handlers: Vec<Arc<ActionHandler>> = vec![Arc::new(stake)];
        let chain_notify = Arc::new(TendermintChainNotify::new(inner.clone()));

        Arc::new(Tendermint {
            client: Default::default(),
            extension_initializer,
            timeouts,
            join: Some(join),
            quit_tendermint,
            inner,
            validators,
            block_reward: our_params.block_reward,
            machine,
            action_handlers,
            chain_notify,
            has_signer: false.into(),
        })
    }
}

enum InnerEvent {
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
    VerifyBlockBasic {
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
    OnNewBlock {
        header: Box<Header>,
        epoch_begin: bool,
        result: crossbeam::Sender<Result<(), Error>>,
    },
    HandleMessage {
        message: Vec<u8>,
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
        result: crossbeam::Sender<Option<Arc<EngineClient>>>,
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

const SEAL_FIELDS: usize = 4;
type SpawnResult = (
    JoinHandle<()>,
    crossbeam::Sender<(crossbeam::Sender<ExtensionEvent>, Weak<EngineClient>)>,
    crossbeam::Sender<InnerEvent>,
    crossbeam::Sender<()>,
);

impl TendermintInner {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    /// Create a new instance of Tendermint engine
    pub fn new(
        validators: Arc<ValidatorSet>,
        extension: EventSender<ExtensionEvent>,
        client: Weak<EngineClient>,
    ) -> Self {
        TendermintInner {
            client,
            height: 1,
            view: 0,
            step: TendermintState::Propose,
            votes: Default::default(),
            signer: Default::default(),
            last_two_thirds_majority: TwoThirdsMajority::Empty,
            proposal: None,
            last_confirmed_view: 0,
            validators,
            extension,
            votes_received: BitSet::new(),
            votes_received_changed: false,

            timeout_token_nonce: ENGINE_TIMEOUT_TOKEN_NONCE_BASE,
        }
    }

    fn spawn(validators: Arc<ValidatorSet>) -> SpawnResult {
        let (sender, receiver) = crossbeam::unbounded();
        let (quit, quit_receiver) = crossbeam::bounded(1);
        let (extension_initializer, extension_receiver) = crossbeam::bounded(1);
        let join = Builder::new()
            .name("tendermint".to_string())
            .spawn(move || {
                let (extension, client) = crossbeam::select! {
                recv(extension_receiver) -> msg => {
                    match msg {
                        Ok((extension, client)) => (extension, client),
                        Err(crossbeam::RecvError) => {
                            cwarn!(ENGINE, "The tendermint extension is not initalized.");
                            return
                        }
                    }
                }
                recv(quit_receiver) -> msg => {
                    match msg {
                        Ok(()) => {},
                        Err(crossbeam::RecvError) => {
                            cwarn!(ENGINE, "The quit channel for tendermint thread had been closed.");
                        }
                    }
                    return
                }
                };
                validators.register_client(Weak::clone(&client));
                let mut inner = Self::new(validators, extension, client);
                loop {
                    crossbeam::select! {
                    recv(receiver) -> msg => {
                        match msg {
                            Ok(InnerEvent::NewBlocks {
                                imported,
                                enacted,
                            }) => {
                                inner.new_blocks(imported, enacted);
                            }
                            Ok(InnerEvent::GenerateSeal {
                                block_number,
                                parent_hash,
                                result,
                            }) => {
                                let seal = inner.generate_seal(block_number, parent_hash);
                                result.send(seal).unwrap();
                            }
                            Ok(InnerEvent::ProposalGenerated(sealed)) => {
                                inner.proposal_generated(&*sealed);
                            }
                            Ok(InnerEvent::VerifyBlockBasic{header, result}) => {
                                result.send(inner.verify_block_basic(&*header)).unwrap();
                            }
                            Ok(InnerEvent::VerifyBlockExternal{header, result, }) => {
                                result.send(inner.verify_block_external(&*header)).unwrap();
                            }
                            Ok(InnerEvent::CalculateScore {
                                block_number,
                                result,
                            }) => {
                                result.send(inner.calculate_score(block_number)).unwrap();
                            }
                            Ok(InnerEvent::OnTimeout(token)) => {
                                inner.on_timeout(token);
                            }
                            Ok(InnerEvent::OnNewBlock {
                                header,
                                epoch_begin,
                                result,
                            }) => {
                                result.send(inner.on_new_block(&header, epoch_begin)).unwrap();
                            }
                            Ok(InnerEvent::HandleMessage {
                                message,
                                result,
                            }) => {
                                result.send(inner.handle_message(&message, false)).unwrap();
                            }
                            Ok(InnerEvent::IsProposal {
                                block_number,
                                block_hash,
                                result,
                            }) => {
                                result.send(inner.is_proposal(block_number, block_hash)).unwrap();
                            }
                            Ok(InnerEvent::SetSigner {
                                ap,
                                address,
                            }) => {
                                inner.set_signer(ap, address);
                            }
                            Ok(InnerEvent::AllowedHeight {
                                result,
                            }) => {
                                let allowed_height = if inner.step.is_commit() {
                                    inner.height + 1
                                } else {
                                    inner.height
                                };
                                result.send(allowed_height).unwrap();
                            }
                            Ok(InnerEvent::Restore(result)) => {
                                inner.restore();
                                result.send(()).unwrap();
                            }
                            Ok(InnerEvent::ProposalBlock {
                                signature,
                                view,
                                message,
                                result,
                            }) => {
                                let client = inner.on_proposal_message(signature, view, message);
                                result.send(client).unwrap();
                            }
                            Ok(InnerEvent::StepState {
                                token, vote_step, proposal, lock_view, known_votes, result
                            }) => {
                                inner.on_step_state_message(&token, vote_step, proposal, lock_view, *known_votes, result);
                            }
                            Ok(InnerEvent::RequestProposal {
                                token,
                                height,
                                view,
                                result,
                            }) => {
                                inner.on_request_proposal_message(&token, height, view, result);
                            }
                            Ok(InnerEvent::GetAllVotesAndAuthors {
                                vote_step,
                                requested,
                                result,
                            }) => {
                                inner.get_all_votes_and_authors(&vote_step, &requested, result);
                            }
                            Err(crossbeam::RecvError) => {
                                cwarn!(ENGINE, "The event channel for tendermint thread had been closed.");
                                break
                            }
                        }
                    }
                    recv(quit_receiver) -> msg => {
                        match msg {
                            Ok(()) => {},
                            Err(crossbeam::RecvError) => {
                                cwarn!(ENGINE, "The quit channel for tendermint thread had been closed.");
                            }
                        }
                        break
                    }
                    }
                }
            })
            .unwrap();
        (join, extension_initializer, sender, quit)
    }

    /// The client is a thread-safe struct. Using it in multi-threads is safe.
    fn client(&self) -> Arc<EngineClient> {
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
            self.validators
                .get_index_by_address(&self.prev_block_hash(), &proposer)
                .expect("The proposer must be in the validator set")
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
        self.block_proposer_idx(*prev_block_hash).map(|prev_proposer_idx| {
            let proposer_nonce = prev_proposer_idx + 1 + view as usize;
            ctrace!(ENGINE, "Proposer nonce: {}", proposer_nonce);
            self.validators.get_address(prev_block_hash, proposer_nonce)
        })
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
    fn check_view_proposer(&self, bh: &H256, height: Height, view: View, address: &Address) -> Result<(), EngineError> {
        self.view_proposer(bh, view).map_or(
            Err(EngineError::PrevBlockNotExist {
                height: height as u64,
            }),
            |proposer| {
                if proposer == *address {
                    Ok(())
                } else {
                    Err(EngineError::NotProposer(Mismatch {
                        expected: proposer,
                        found: *address,
                    }))
                }
            },
        )
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

    fn broadcast_message(&self, message: Bytes) {
        self.extension
            .send(ExtensionEvent::BroadcastMessage {
                message,
            })
            .unwrap();
    }

    fn broadcast_state(&self, vote_step: VoteStep, proposal: Option<H256>, lock_view: Option<View>, votes: BitSet) {
        self.extension
            .send(ExtensionEvent::BroadcastState {
                vote_step,
                proposal,
                lock_view,
                votes,
            })
            .unwrap();
    }

    fn request_messages_to_all(&self, vote_step: VoteStep, requested_votes: BitSet) {
        self.extension
            .send(ExtensionEvent::RequestMessagesToAll {
                vote_step,
                requested_votes,
            })
            .unwrap();
    }

    fn request_proposal_to_any(&self, height: Height, view: View) {
        self.extension
            .send(ExtensionEvent::RequestProposalToAny {
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
        self.proposal = None;
        self.votes_received = BitSet::new();
    }

    fn move_to_height(&mut self, height: Height) {
        assert!(height > self.height, "{} < {}", height, self.height);
        cinfo!(ENGINE, "Transitioning to height {}.", height);
        self.last_two_thirds_majority = TwoThirdsMajority::Empty;
        self.height = height;
        self.view = 0;
        self.proposal = None;
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
            .send(ExtensionEvent::SetTimerStep {
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
        self.broadcast_state(vote_step, self.proposal, self.last_two_thirds_majority.view(), self.votes_received);
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
                        if let TwoThirdsMajority::Lock(lock_view, _) = self.last_two_thirds_majority {
                            cinfo!(ENGINE, "I am a proposer, I'll re-propose a locked block");
                            match self.locked_proposal_block(lock_view) {
                                Ok(block) => self.repropose_block(block),
                                Err(error_msg) => cwarn!(ENGINE, "{}", error_msg),
                            }
                        } else {
                            cinfo!(ENGINE, "I am a proposer, I'll create a block");
                            self.update_sealing(*parent_block_hash);
                            self.step = TendermintState::ProposeWaitBlockGeneration {
                                parent_hash: *parent_block_hash,
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
                    let block_hash = match &self.last_two_thirds_majority {
                        TwoThirdsMajority::Empty => self.proposal,
                        TwoThirdsMajority::Unlock(_) => self.proposal,
                        TwoThirdsMajority::Lock(_, block_hash) => Some(*block_hash),
                    };
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
        self.votes_received_changed = true;
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
            height: proposal.number() - 1,
            view: 0,
            step: Step::Propose,
        });

        let current_height = self.height;
        let vote_step = VoteStep::new(self.height, self.view, self.step.to_step());
        let proposal_at_current_view = self.votes.get_block_hashes(&vote_step).first().cloned();
        if proposal_at_current_view == Some(proposal.hash()) {
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
                        self.extension
                            .send(ExtensionEvent::SetTimerEmptyProposal {
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
            let proposal_view = consensus_view(proposal).unwrap();
            self.save_last_confirmed_view(proposal_view);
            self.proposal = Some(proposal.hash());
            self.move_to_step(Step::Prevote, false);
        }
    }

    fn submit_proposal_block(&mut self, sealed_block: &SealedBlock) {
        cinfo!(ENGINE, "Submitting proposal block {}", sealed_block.header().hash());
        self.move_to_step(Step::Prevote, false);
        self.broadcast_proposal_block(self.view, encoded::Block::new(sealed_block.rlp_bytes()));
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

    fn seal_fields(&self) -> usize {
        SEAL_FIELDS
    }

    fn generate_seal(&self, height: Height, parent_hash: H256) -> Seal {
        // Block is received from other nodes while creating a block
        if height < self.height {
            return Seal::None
        }

        assert_eq!(true, self.is_signer_proposer(&parent_hash));
        assert_eq!(true, self.proposal.is_none());
        assert_eq!(true, height == self.height);

        let view = self.view;

        let last_block_hash = &self.prev_block_hash();
        let last_block_view = &self.last_confirmed_view;
        assert_eq!(last_block_hash, &parent_hash);

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
        let prev_proposer_idx = self.block_proposer_idx(*header.parent_hash()).expect("Prev block must exists");

        debug_assert_eq!(self.view, consensus_view(&header).expect("I am proposer"));

        let vote_step = VoteStep::new(header.number() as Height, self.view, Step::Propose);
        let vote_info = message_info_rlp(vote_step, Some(hash));
        let num_validators = self.validators.count(&self.prev_block_hash());
        let signature = self.sign(blake256(&vote_info)).expect("I am proposer");
        self.votes.vote(
            ConsensusMessage::new_proposal(signature, num_validators, header, self.view, prev_proposer_idx)
                .expect("I am proposer"),
        );

        self.step = TendermintState::ProposeWaitImported {
            block: Box::new(sealed_block.clone()),
        };
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
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
        let view = consensus_view(header).unwrap();
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
        let view = consensus_view(header).unwrap();
        ctrace!(ENGINE, "Verify external at {}-{}, {:?}", height, view, header);
        let proposer = header.author();
        if !self.is_authority(header.parent_hash(), proposer) {
            return Err(EngineError::BlockNotAuthorized(*proposer).into())
        }
        self.check_view_proposer(header.parent_hash(), header.number(), consensus_view(header)?, &proposer)
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

        if bitset_count > precommits_count {
            cwarn!(
                ENGINE,
                "verify_block_external: The header({})'s bitset count is greater than the precommits count",
                header.hash()
            );
            return Err(BlockError::InvalidSeal.into())
        }

        let previous_block_view = previous_block_view(header)?;
        let step = VoteStep::new(header.number() - 1, previous_block_view, Step::Precommit);
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
                    self.proposal,
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
                if self.check_current_block_exists() {
                    let height = self.height;
                    self.move_to_height(height + 1);
                    Some(Step::Propose)
                } else {
                    cwarn!(ENGINE, "Best chain is not updated yet, wait until imported");
                    self.step = TendermintState::CommitTimedout;
                    None
                }
            }
            TendermintState::CommitTimedout => unreachable!(),
        };

        if let Some(next_step) = next_step {
            self.move_to_step(next_step, false);
        }
    }

    fn is_expired_timeout_token(&self, nonce: usize) -> bool {
        nonce < self.timeout_token_nonce
    }

    fn on_new_block(&self, header: &Header, epoch_begin: bool) -> Result<(), Error> {
        if !epoch_begin {
            return Ok(())
        }

        // genesis is never a new block, but might as well check.
        let first = header.number() == 0;

        self.validators.on_epoch_begin(first, header)
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
        let num_validators = self.validators.count(&self.prev_block_hash());
        let prev_proposer_idx = self.block_proposer_idx(*header.parent_hash()).expect("Prev block must exists");
        let signature = self.sign(blake256(&vote_info)).expect("I am proposer");
        self.votes.vote(
            ConsensusMessage::new_proposal(signature, num_validators, &header, self.view, prev_proposer_idx)
                .expect("I am proposer"),
        );

        self.proposal = Some(header.hash());
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
            .send(ExtensionEvent::BroadcastProposalBlock {
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

    fn signer_index(&self, bh: &H256) -> Option<usize> {
        // FIXME: More effecient way to find index
        self.signer.public().and_then(|public| self.validators.get_index(bh, public))
    }
}

fn calculate_score(height: Height, view: View) -> U256 {
    let height = U256::from(height);
    u256_from_u128(std::u128::MAX) * height - view
}


impl ConsensusEngine<CodeChainMachine> for Tendermint {
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
            .send(InnerEvent::GenerateSeal {
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
        self.inner.send(InnerEvent::ProposalGenerated(Box::from(sealed_block.clone()))).unwrap();
    }

    fn verify_local_seal(&self, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    fn verify_block_basic(&self, header: &Header) -> Result<(), Error> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(InnerEvent::VerifyBlockBasic {
                header: Box::from(header.clone()),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn verify_block_external(&self, header: &Header) -> Result<(), Error> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(InnerEvent::VerifyBlockExternal {
                header: Box::from(header.clone()),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
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
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(InnerEvent::CalculateScore {
                block_number: header.number(),
                result,
            })
            .unwrap();
        let score = receiver.recv().unwrap();
        header.set_score(score);
    }

    /// Equivalent to a timeout: to be used for tests.
    fn on_timeout(&self, token: usize) {
        self.inner.send(InnerEvent::OnTimeout(token)).unwrap();
    }

    fn stop(&self) {}

    fn on_new_block(&self, block: &mut ExecutedBlock, epoch_begin: bool) -> Result<(), Error> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(InnerEvent::OnNewBlock {
                header: Box::from(block.header().clone()),
                epoch_begin,
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
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

    fn register_client(&self, client: Weak<EngineClient>) {
        *self.client.write() = Some(Weak::clone(&client));
    }

    fn handle_message(&self, rlp: &[u8]) -> Result<(), EngineError> {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(InnerEvent::HandleMessage {
                message: rlp.to_owned(),
                result,
            })
            .unwrap();
        receiver.recv().unwrap()
    }

    fn is_proposal(&self, header: &Header) -> bool {
        let (result, receiver) = crossbeam::bounded(1);
        self.inner
            .send(InnerEvent::IsProposal {
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
            .send(InnerEvent::SetSigner {
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
        self.inner.send(InnerEvent::Restore(result)).unwrap();
        receiver.recv().unwrap();
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
            .send(InnerEvent::AllowedHeight {
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

struct TendermintChainNotify {
    inner: crossbeam::Sender<InnerEvent>,
}

impl TendermintChainNotify {
    fn new(inner: crossbeam::Sender<InnerEvent>) -> Self {
        Self {
            inner,
        }
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
        self.inner
            .send(InnerEvent::NewBlocks {
                imported,
                enacted,
            })
            .unwrap();
    }
}

impl TendermintInner {
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
                    let view = consensus_view(&full_header).expect("Imported block already checked");
                    self.move_to_height(header.number());
                    self.save_last_confirmed_view(view);
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
    inner: crossbeam::Sender<InnerEvent>,
    peers: HashMap<NodeId, PeerState>,
    api: Box<Api>,
    timeouts: TimeoutParams,
}

const MIN_PEERS_PROPAGATION: usize = 4;
const MAX_PEERS_PROPAGATION: usize = 128;

impl TendermintExtension {
    fn new(inner: crossbeam::Sender<InnerEvent>, timeouts: TimeoutParams, api: Box<Api>) -> Self {
        let initial = timeouts.initial();
        ctrace!(ENGINE, "Setting the initial timeout to {}.", initial);
        api.set_timer_once(ENGINE_TIMEOUT_TOKEN_NONCE_BASE, initial).expect("Timer set succeeds");
        api.set_timer(
            ENGINE_TIMEOUT_BROADCAST_STEP_STATE,
            Duration::seconds(ENGINE_TIMEOUT_BROADCAT_STEP_STATE_INTERVAL),
        )
        .expect("Timer set succeeds");
        Self {
            inner,
            peers: Default::default(),
            api,
            timeouts,
        }
    }

    fn update_peer_state(&mut self, token: &NodeId, vote_step: VoteStep, proposal: Option<H256>, messages: BitSet) {
        let peer_state = match self.peers.get_mut(token) {
            Some(peer_state) => peer_state,
            // update_peer_state could be called after the peer is disconnected
            None => return,
        };
        peer_state.vote_step = vote_step;
        peer_state.proposal = proposal;
        peer_state.messages = messages;
    }

    fn select_random_peers(&self) -> Vec<NodeId> {
        let mut peers: Vec<NodeId> = self.peers.keys().cloned().collect();
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

    fn broadcast_state(&self, vote_step: VoteStep, proposal: Option<H256>, lock_view: Option<View>, votes: BitSet) {
        ctrace!(ENGINE, "Broadcast state {:?} {:?} {:?}", vote_step, proposal, votes);
        let tokens = self.select_random_peers();
        let message = TendermintMessage::StepState {
            vote_step,
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

    fn broadcast_proposal_block(&self, signature: SchnorrSignature, view: View, message: Bytes) {
        let message = TendermintMessage::ProposalBlock {
            signature,
            message,
            view,
        }
        .rlp_bytes()
        .into_vec();
        for token in self.peers.keys() {
            self.api.send(token, &message);
        }
    }

    fn request_proposal_to_any(&self, height: Height, view: View) {
        for (token, peer) in &self.peers {
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

    fn request_messages_to_all(&self, vote_step: VoteStep, requested_votes: BitSet) {
        for token in self.select_random_peers() {
            let peer = &self.peers[&token];
            if vote_step <= peer.vote_step && !peer.messages.is_empty() {
                self.request_messages(&token, vote_step, requested_votes);
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

    fn set_timer_step(&self, step: Step, view: View, expired_token_nonce: TimerToken) {
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
}

impl TendermintInner {
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
    ) -> Option<Arc<EngineClient>> {
        let c = match self.client.upgrade() {
            Some(c) => c,
            None => return None,
        };

        // This block borrows bytes
        {
            let block_view = BlockView::new(&bytes);
            let header_view = block_view.header();
            let number = header_view.number();
            cinfo!(ENGINE, "Proposal received for {}-{:?}", number, header_view.hash());

            let parent_hash = header_view.parent_hash();
            #[cfg_attr(feature = "cargo-clippy", allow(clippy::question_mark))]
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

            let num_validators = self.validators.count(&parent_hash);
            let prev_proposer_idx = match self.block_proposer_idx(*parent_hash) {
                Some(idx) => idx,
                None => {
                    cwarn!(ENGINE, "Prev block proposer does not exist for height {}", number);
                    return None
                }
            };

            let message = match ConsensusMessage::new_proposal(
                signature,
                num_validators,
                &header_view,
                proposed_view,
                prev_proposer_idx,
            ) {
                Ok(message) => message,
                Err(err) => {
                    cwarn!(ENGINE, "Invalid proposal received: {:?}", err);
                    return None
                }
            };

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
                    let generated_view = consensus_view(&header_view).expect("Imported block is verified");
                    cdebug!(
                        ENGINE,
                        "Received a proposal({}) by a locked proposer. current view: {}, original proposal's view: {}",
                        header_view.hash(),
                        proposed_view,
                        generated_view
                    );
                    self.proposal = Some(header_view.hash());
                }
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
        }
    }
}

impl NetworkExtension<ExtensionEvent> for TendermintExtension {
    fn name() -> &'static str {
        "tendermint"
    }

    fn need_encryption() -> bool {
        false
    }

    fn versions() -> &'static [u64] {
        const VERSIONS: &[u64] = &[0];
        &VERSIONS
    }

    fn on_node_added(&mut self, token: &NodeId, _version: u64) {
        self.peers.insert(*token, PeerState::new());
    }

    fn on_node_removed(&mut self, token: &NodeId) {
        self.peers.remove(token);
    }

    fn on_message(&mut self, token: &NodeId, data: &[u8]) {
        let m = UntrustedRlp::new(data);
        match m.as_val() {
            Ok(TendermintMessage::ConsensusMessage(ref bytes)) => {
                let (result, receiver) = crossbeam::bounded(1);
                self.inner
                    .send(InnerEvent::HandleMessage {
                        message: bytes.clone(),
                        result,
                    })
                    .unwrap();
                match receiver.recv().unwrap() {
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
            }
            Ok(TendermintMessage::ProposalBlock {
                signature,
                view,
                message,
            }) => {
                let (result, receiver) = crossbeam::bounded(1);
                self.inner
                    .send(InnerEvent::ProposalBlock {
                        signature,
                        view,
                        message: message.clone(),
                        result,
                    })
                    .unwrap();
                if let Some(c) = receiver.recv().unwrap() {
                    if let Err(e) = c.import_block(message) {
                        cinfo!(ENGINE, "Failed to import proposal block {:?}", e);
                    }
                }
            }
            Ok(TendermintMessage::StepState {
                vote_step,
                proposal,
                lock_view,
                known_votes,
            }) => {
                ctrace!(
                    ENGINE,
                    "Peer state update step: {:?} proposal {:?} peer_lock_view {:?} known_votes {:?}",
                    vote_step,
                    proposal,
                    lock_view,
                    known_votes,
                );
                self.update_peer_state(token, vote_step, proposal, known_votes);
                let (result, receiver) = crossbeam::unbounded();
                self.inner
                    .send(InnerEvent::StepState {
                        token: *token,
                        vote_step,
                        proposal,
                        lock_view,
                        known_votes: Box::from(known_votes),
                        result,
                    })
                    .unwrap();

                while let Ok(message) = receiver.recv() {
                    self.api.send(token, &message);
                }
            }
            Ok(TendermintMessage::RequestProposal {
                height,
                view,
            }) => {
                let (result, receiver) = crossbeam::bounded(1);
                self.inner
                    .send(InnerEvent::RequestProposal {
                        token: *token,
                        height,
                        view,
                        result,
                    })
                    .unwrap();
                if let Ok(message) = receiver.recv() {
                    self.api.send(token, &message);
                }
            }
            Ok(TendermintMessage::RequestMessage {
                vote_step,
                requested_votes,
            }) => {
                ctrace!(ENGINE, "Received RequestMessage for {:?} from {:?}", vote_step, requested_votes);

                let (result, receiver) = crossbeam::unbounded();
                self.inner
                    .send(InnerEvent::GetAllVotesAndAuthors {
                        vote_step,
                        requested: requested_votes,
                        result,
                    })
                    .unwrap();

                for vote in receiver.iter() {
                    self.send_message(token, vote.rlp_bytes().into_vec());
                }
            }
            _ => cinfo!(ENGINE, "Invalid message from peer {}", token),
        }
    }

    fn on_timeout(&mut self, token: TimerToken) {
        debug_assert!(
            token >= ENGINE_TIMEOUT_TOKEN_NONCE_BASE
                || token == ENGINE_TIMEOUT_EMPTY_PROPOSAL
                || token == ENGINE_TIMEOUT_BROADCAST_STEP_STATE
        );
        self.inner.send(InnerEvent::OnTimeout(token)).unwrap();
    }

    fn on_event(&mut self, event: ExtensionEvent) {
        match event {
            ExtensionEvent::BroadcastMessage {
                message,
            } => {
                self.broadcast_message(message);
            }
            ExtensionEvent::BroadcastState {
                vote_step,
                proposal,
                lock_view,
                votes,
            } => {
                self.broadcast_state(vote_step, proposal, lock_view, votes);
            }
            ExtensionEvent::RequestMessagesToAll {
                vote_step,
                requested_votes,
            } => {
                self.request_messages_to_all(vote_step, requested_votes);
            }
            ExtensionEvent::RequestProposalToAny {
                height,
                view,
            } => {
                self.request_proposal_to_any(height, view);
            }
            ExtensionEvent::SetTimerStep {
                step,
                view,
                expired_token_nonce,
            } => self.set_timer_step(step, view, expired_token_nonce),
            ExtensionEvent::SetTimerEmptyProposal {
                view,
            } => {
                self.set_timer_empty_proposal(view);
            }
            ExtensionEvent::BroadcastProposalBlock {
                signature,
                view,
                message,
            } => {
                self.broadcast_proposal_block(signature, view, message);
            }
        }
    }
}

pub enum ExtensionEvent {
    BroadcastMessage {
        message: Bytes,
    },
    BroadcastState {
        vote_step: VoteStep,
        proposal: Option<H256>,
        lock_view: Option<View>,
        votes: BitSet,
    },
    RequestMessagesToAll {
        vote_step: VoteStep,
        requested_votes: BitSet,
    },
    RequestProposalToAny {
        height: Height,
        view: View,
    },
    SetTimerStep {
        step: Step,
        view: View,
        expired_token_nonce: TimerToken,
    },
    SetTimerEmptyProposal {
        view: View,
    },
    BroadcastProposalBlock {
        signature: SchnorrSignature,
        view: View,
        message: Bytes,
    },
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
    fn setup() -> (Scheme, Arc<AccountProvider>, Arc<TestBlockChainClient>) {
        let tap = AccountProvider::transient_provider();
        let scheme = Scheme::new_test_tendermint();
        let test = TestBlockChainClient::new_with_scheme(Scheme::new_test_tendermint());

        let test_client: Arc<TestBlockChainClient> = Arc::new(test);
        let engine_client = Arc::clone(&test_client) as Arc<EngineClient>;
        scheme.engine.register_client(Arc::downgrade(&engine_client));
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
    #[ignore] // FIXME
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
    #[ignore] // FIXME
    fn generate_seal() {
        let (scheme, tap, _c) = setup();

        let proposer = insert_and_register(&tap, scheme.engine.as_ref(), "1");

        let (b, seal) = propose_default(&scheme, proposer);
        assert!(b.lock().try_seal(scheme.engine.as_ref(), seal).is_ok());
    }

    #[test]
    #[ignore] // FIXME
    fn parent_block_existence_checking() {
        let (spec, tap, _c) = setup();
        let engine = spec.engine;

        let mut header = Header::default();
        header.set_number(4);
        let proposer = insert_and_unlock(&tap, "0");
        header.set_author(proposer);
        header.set_parent_hash(Default::default());

        let vote_info = message_info_rlp(VoteStep::new(3, 0, Step::Precommit), Some(*header.parent_hash()));
        let signature2 = tap.get_account(&proposer, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature2],
            precommit_bitset: BitSet::new_with_indices(&[2]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        println!(".....");
        assert!(engine.verify_block_external(&header).is_err());
    }

    #[test]
    #[ignore] // FIXME
    fn seal_signatures_checking() {
        let (spec, tap, c) = setup();
        let engine = spec.engine;

        let validator0 = insert_and_unlock(&tap, "0");
        let validator1 = insert_and_unlock(&tap, "1");
        let validator2 = insert_and_unlock(&tap, "2");
        let validator3 = insert_and_unlock(&tap, "3");

        let block1_hash = c.add_block_with_author(Some(validator1), 1, 1);

        let mut header = Header::default();
        header.set_number(2);
        let proposer = validator2;
        header.set_author(proposer);
        header.set_parent_hash(block1_hash);

        let vote_info = message_info_rlp(VoteStep::new(1, 0, Step::Precommit), Some(*header.parent_hash()));
        let signature2 = tap.get_account(&proposer, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature2],
            precommit_bitset: BitSet::new_with_indices(&[2]),
        }
        .seal_fields()
        .unwrap();
        header.set_seal(seal);

        // One good signature is not enough.
        match engine.verify_block_external(&header) {
            Err(Error::Engine(EngineError::BadSealFieldSize(_))) => {}
            _ => panic!(),
        }

        let voter = validator3;
        let signature3 = tap.get_account(&voter, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();
        let voter = validator0;
        let signature0 = tap.get_account(&voter, None).unwrap().sign_schnorr(&blake256(&vote_info)).unwrap();

        let seal = Seal::Tendermint {
            prev_view: 0,
            cur_view: 0,
            precommits: vec![signature0, signature2, signature3],
            precommit_bitset: BitSet::new_with_indices(&[0, 2, 3]),
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
            precommits: vec![signature0, signature2, bad_signature],
            precommit_bitset: BitSet::new_with_indices(&[0, 2, 3]),
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
