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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::Iterator;

use ckey::SchnorrSignature;
use parking_lot::RwLock;
use primitives::H256;
use rlp::{Encodable, RlpStream};

pub trait Message: Clone + PartialEq + Eq + Hash + Encodable + Debug {
    type Round: Clone + Copy + PartialEq + Eq + Hash + Default + Debug + Ord;

    fn signature(&self) -> SchnorrSignature;

    fn signer_index(&self) -> usize;

    fn block_hash(&self) -> Option<H256>;

    fn round(&self) -> &Self::Round;

    fn is_broadcastable(&self) -> bool;
}

/// Storing all Proposals, Prevotes and Precommits.
#[derive(Debug)]
pub struct VoteCollector<M: Message> {
    votes: RwLock<BTreeMap<M::Round, StepCollector<M>>>,
}

#[derive(Debug, Default)]
struct StepCollector<M: Message> {
    voted: HashMap<usize, M>,
    block_votes: HashMap<Option<H256>, BTreeMap<usize, SchnorrSignature>>,
    messages: HashSet<M>,
}

#[derive(Debug)]
pub struct DoubleVote<M: Message> {
    author_index: usize,
    vote_one: M,
    vote_two: M,
}

impl<M: Message> Encodable for DoubleVote<M> {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&self.vote_one).append(&self.vote_two);
    }
}

impl<M: Message> StepCollector<M> {
    /// Returns Some(&Address) when validator is double voting.
    fn insert(&mut self, message: M) -> Option<DoubleVote<M>> {
        // Do nothing when message was seen.
        if self.messages.insert(message.clone()) {
            if let Some(previous) = self.voted.insert(message.signer_index(), message.clone()) {
                // Bad validator sent a different message.
                return Some(DoubleVote {
                    author_index: message.signer_index(),
                    vote_one: previous,
                    vote_two: message,
                })
            } else {
                self.block_votes
                    .entry(message.block_hash())
                    .or_default()
                    .insert(message.signer_index(), message.signature());
            }
        }
        None
    }

    /// Count all votes for the given block hash at this round.
    fn count_block(&self, block_hash: &Option<H256>) -> usize {
        self.block_votes.get(block_hash).map_or(0, BTreeMap::len)
    }

    /// Count all votes collected for the given round.
    fn count(&self) -> usize {
        self.block_votes.values().map(BTreeMap::len).sum()
    }
}

impl<M: Message + Default> Default for VoteCollector<M> {
    fn default() -> Self {
        let mut collector = BTreeMap::new();
        // Insert dummy entry to fulfill invariant: "only messages newer than the oldest are inserted".
        collector.insert(Default::default(), Default::default());
        VoteCollector {
            votes: RwLock::new(collector),
        }
    }
}

impl<M: Message + Default + Encodable + Debug> VoteCollector<M> {
    /// Insert vote if it is newer than the oldest one.
    pub fn vote(&self, message: M) -> Option<DoubleVote<M>> {
        self.votes.write().entry(*message.round()).or_insert_with(Default::default).insert(message)
    }

    /// Checks if the message should be ignored.
    pub fn is_old_or_known(&self, message: &M) -> bool {
        let read_guard = self.votes.read();

        let is_known = read_guard.get(&message.round()).map_or(false, |c| c.messages.contains(message));
        if is_known {
            ctrace!(ENGINE, "Known message: {:?}.", message);
            return true
        }

        // The reason not using `message.round() <= oldest` is to allow precommit messages on Commit step.
        let is_old = read_guard.keys().next().map_or(true, |oldest| message.round() < oldest);
        if is_old {
            ctrace!(ENGINE, "Old message {:?}.", message);
            return true
        }

        false
    }

    /// Throws out messages older than message, leaves message as marker for the oldest.
    pub fn throw_out_old(&self, vote_round: &M::Round) {
        let mut guard = self.votes.write();
        let new_collector = guard.split_off(vote_round);
        assert!(!new_collector.is_empty());
        *guard = new_collector;
    }

    /// Collects the signatures and the indices for the given round and hash.
    /// Returning indices is in ascending order, and signature and indices are matched with another.
    pub fn round_signatures_and_indices(
        &self,
        round: &M::Round,
        block_hash: &H256,
    ) -> (Vec<SchnorrSignature>, Vec<usize>) {
        let guard = self.votes.read();
        guard
            .get(round)
            .and_then(|c| c.block_votes.get(&Some(*block_hash)))
            .map(|votes| {
                let (indices, sigs) = votes.iter().unzip();
                (sigs, indices)
            })
            .unwrap_or_default()
    }


    /// Returns the first signature and the index of its signer for a given round and hash if exists.
    pub fn round_signature(&self, round: &M::Round, block_hash: &H256) -> Option<SchnorrSignature> {
        let guard = self.votes.read();
        guard
            .get(round)
            .and_then(|c| c.block_votes.get(&Some(*block_hash)))
            .and_then(|votes| votes.values().next().cloned())
    }

    /// Count votes which agree with the given message.
    pub fn count_aligned_votes(&self, message: &M) -> usize {
        self.votes.read().get(&message.round()).map_or(0, |m| m.count_block(&message.block_hash()))
    }

    pub fn count_block_round_votes(&self, round: &M::Round, block_hash: &Option<H256>) -> usize {
        self.votes.read().get(round).map_or(0, |m| m.count_block(block_hash))
    }

    /// Count all votes collected for a given round.
    pub fn count_round_votes(&self, vote_round: &M::Round) -> usize {
        self.votes.read().get(vote_round).map_or(0, StepCollector::count)
    }

    pub fn get_block_hashes(&self, round: &M::Round) -> Vec<H256> {
        let guard = self.votes.read();
        guard.get(round).map(|c| c.block_votes.keys().cloned().filter_map(|x| x).collect()).unwrap_or_else(Vec::new)
    }

    pub fn get_all(&self) -> Vec<M> {
        self.votes.read().iter().flat_map(|(_round, collector)| collector.messages.iter()).cloned().collect()
    }

    pub fn get_all_votes_in_round(&self, round: &M::Round) -> Vec<M> {
        let guard = self.votes.read();
        let c = guard.get(round);
        c.map(|c| c.messages.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn get_all_votes_and_indices_in_round(&self, round: &M::Round) -> Vec<(usize, M)> {
        let guard = self.votes.read();
        guard.get(round).map(|c| c.voted.iter().map(|(k, v)| (*k, v.clone())).collect()).unwrap_or_default()
    }
}
