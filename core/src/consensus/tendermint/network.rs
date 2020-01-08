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

use super::super::BitSet;
use super::message::*;
use super::params::TimeoutParams;
use super::types::{Height, PeerState, Step, View};
use super::worker;
use super::{
    ENGINE_TIMEOUT_BROADCAST_STEP_STATE, ENGINE_TIMEOUT_BROADCAT_STEP_STATE_INTERVAL, ENGINE_TIMEOUT_EMPTY_PROPOSAL,
    ENGINE_TIMEOUT_TOKEN_NONCE_BASE,
};
use crate::consensus::EngineError;
use ckey::SchnorrSignature;
use cnetwork::{Api, NetworkExtension, NodeId};
use crossbeam_channel as crossbeam;
use ctimer::TimerToken;
use ctypes::BlockHash;
use primitives::Bytes;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rlp::{Encodable, Rlp};
use std::cmp;
use std::collections::HashMap;
use std::iter::Iterator;
use std::sync::Arc;
use std::time::Duration;

pub struct TendermintExtension {
    inner: crossbeam::Sender<worker::Event>,
    peers: HashMap<NodeId, PeerState>,
    api: Box<dyn Api>,
    timeouts: TimeoutParams,
}

const MIN_PEERS_PROPAGATION: usize = 4;
const MAX_PEERS_PROPAGATION: usize = 128;

impl TendermintExtension {
    pub fn new(inner: crossbeam::Sender<worker::Event>, timeouts: TimeoutParams, api: Box<dyn Api>) -> Self {
        let initial = timeouts.initial();
        ctrace!(ENGINE, "Setting the initial timeout to {:?}.", initial);
        api.set_timer_once(ENGINE_TIMEOUT_TOKEN_NONCE_BASE, initial).expect("Timer set succeeds");
        api.set_timer(
            ENGINE_TIMEOUT_BROADCAST_STEP_STATE,
            Duration::from_secs(ENGINE_TIMEOUT_BROADCAT_STEP_STATE_INTERVAL),
        )
        .expect("Timer set succeeds");
        Self {
            inner,
            peers: Default::default(),
            api,
            timeouts,
        }
    }

    fn update_peer_state(
        &mut self,
        token: &NodeId,
        vote_step: VoteStep,
        proposal: Option<BlockHash>,
        messages: BitSet,
    ) {
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
        let message = Arc::new(TendermintMessage::ConsensusMessage(vec![message]).rlp_bytes());
        for token in tokens {
            self.api.send(&token, Arc::clone(&message));
        }
    }

    fn send_votes(&self, token: &NodeId, messages: Vec<Bytes>) {
        ctrace!(ENGINE, "Send messages({}) to {}", messages.len(), token);
        let message = Arc::new(TendermintMessage::ConsensusMessage(messages).rlp_bytes());
        self.api.send(token, message);
    }

    fn broadcast_state(
        &self,
        vote_step: VoteStep,
        proposal: Option<BlockHash>,
        lock_view: Option<View>,
        votes: BitSet,
    ) {
        ctrace!(ENGINE, "Broadcast state {:?} {:?} {:?}", vote_step, proposal, votes);
        let tokens = self.select_random_peers();
        let message = Arc::new(
            TendermintMessage::StepState {
                vote_step,
                proposal,
                lock_view,
                known_votes: votes,
            }
            .rlp_bytes(),
        );

        for token in tokens {
            self.api.send(&token, Arc::clone(&message));
        }
    }

    fn broadcast_proposal_block(&self, signature: SchnorrSignature, view: View, message: Bytes) {
        let message = Arc::new(
            TendermintMessage::ProposalBlock {
                signature,
                message,
                view,
            }
            .rlp_bytes(),
        );
        for token in self.peers.keys() {
            self.api.send(token, Arc::clone(&message));
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
        let message = Arc::new(
            TendermintMessage::RequestProposal {
                height,
                view,
            }
            .rlp_bytes(),
        );
        self.api.send(&token, message);
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
        let message = Arc::new(
            TendermintMessage::RequestMessage {
                vote_step,
                requested_votes,
            }
            .rlp_bytes(),
        );
        self.api.send(&token, message);
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

impl NetworkExtension<Event> for TendermintExtension {
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
        let m = Rlp::new(data);
        match m.as_val() {
            Ok(TendermintMessage::ConsensusMessage(ref messages)) => {
                ctrace!(ENGINE, "Received messages({})", messages.len());
                let (result, receiver) = crossbeam::bounded(messages.len());
                self.inner
                    .send(worker::Event::HandleMessages {
                        messages: messages.clone(),
                        result,
                    })
                    .unwrap();
                for result in receiver.iter() {
                    match result {
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
            }
            Ok(TendermintMessage::ProposalBlock {
                signature,
                view,
                message,
            }) => {
                let (result, receiver) = crossbeam::bounded(1);
                self.inner
                    .send(worker::Event::ProposalBlock {
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
                    .send(worker::Event::StepState {
                        token: *token,
                        vote_step,
                        proposal,
                        lock_view,
                        known_votes: Box::from(known_votes),
                        result,
                    })
                    .unwrap();

                while let Ok(message) = receiver.recv() {
                    self.api.send(token, Arc::new(message));
                }
            }
            Ok(TendermintMessage::RequestProposal {
                height,
                view,
            }) => {
                let (result, receiver) = crossbeam::bounded(1);
                self.inner
                    .send(worker::Event::RequestProposal {
                        token: *token,
                        height,
                        view,
                        result,
                    })
                    .unwrap();
                if let Ok(message) = receiver.recv() {
                    self.api.send(token, Arc::new(message));
                }
            }
            Ok(TendermintMessage::RequestMessage {
                vote_step,
                requested_votes,
            }) => {
                ctrace!(ENGINE, "Received RequestMessage for {:?} from {:?}", vote_step, requested_votes);

                let (result, receiver) = crossbeam::unbounded();
                self.inner
                    .send(worker::Event::GetAllVotesAndAuthors {
                        vote_step,
                        requested: requested_votes,
                        result,
                    })
                    .unwrap();

                let votes: Vec<_> = receiver.iter().map(|vote| vote.rlp_bytes()).collect();
                if !votes.is_empty() {
                    self.send_votes(token, votes);
                }
            }
            Ok(TendermintMessage::RequestCommit {
                height,
            }) => {
                ctrace!(ENGINE, "Received RequestCommit for {} from {:?}", height, token);
                let (result, receiver) = crossbeam::bounded(1);
                self.inner
                    .send(worker::Event::RequestCommit {
                        height,
                        result,
                    })
                    .unwrap();

                if let Ok(message) = receiver.recv() {
                    ctrace!(ENGINE, "Send commit for {} to {:?}", height, token);
                    self.api.send(token, Arc::new(message));
                }
            }
            Ok(TendermintMessage::Commit {
                block,
                votes,
            }) => {
                ctrace!(ENGINE, "Received Commit from {:?}", token);
                let (result, receiver) = crossbeam::bounded(1);
                self.inner
                    .send(worker::Event::GetCommit {
                        block: block.clone(),
                        votes,
                        result,
                    })
                    .unwrap();

                if let Some(c) = receiver.recv().unwrap() {
                    if let Err(e) = c.import_block(block) {
                        cinfo!(ENGINE, "Failed to import committed block {:?}", e);
                    }
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
        self.inner.send(worker::Event::OnTimeout(token)).unwrap();
    }

    fn on_event(&mut self, event: Event) {
        match event {
            Event::BroadcastMessage {
                message,
            } => {
                self.broadcast_message(message);
            }
            Event::BroadcastState {
                vote_step,
                proposal,
                lock_view,
                votes,
            } => {
                self.broadcast_state(vote_step, proposal, lock_view, votes);
            }
            Event::RequestMessagesToAll {
                vote_step,
                requested_votes,
            } => {
                self.request_messages_to_all(vote_step, requested_votes);
            }
            Event::RequestProposalToAny {
                height,
                view,
            } => {
                self.request_proposal_to_any(height, view);
            }
            Event::SetTimerStep {
                step,
                view,
                expired_token_nonce,
            } => self.set_timer_step(step, view, expired_token_nonce),
            Event::SetTimerEmptyProposal {
                view,
            } => {
                self.set_timer_empty_proposal(view);
            }
            Event::BroadcastProposalBlock {
                signature,
                view,
                message,
            } => {
                self.broadcast_proposal_block(signature, view, message);
            }
        }
    }
}

pub enum Event {
    BroadcastMessage {
        message: Bytes,
    },
    BroadcastState {
        vote_step: VoteStep,
        proposal: Option<BlockHash>,
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
