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

use std::collections::{btree_set::Iter, BTreeMap, BTreeSet, HashMap};
use std::iter::{Iterator, Rev};

use ckey::SchnorrSignature;
use ctypes::BlockHash;
use rlp::{Encodable, RlpStream};

use super::super::{Priority, PriorityInfo};
use super::stake::Action;
use super::{ConsensusMessage, ProposalSummary, SortitionRound, Step, VoteStep};
use crate::consensus::BitSet;

/// Storing all Proposals, Prevotes and Precommits.
/// Invariant: Proposal step links to StepCollector::PP variant
///             and Other steps link to StepCollector::PVPC variant
#[derive(Debug)]
pub struct VoteCollector {
    votes: BTreeMap<VoteStep, StepCollector>,
}

#[derive(Debug)]
enum StepCollector {
    PP(PpCollector),
    PVPC(PvPcCollector),
}

impl StepCollector {
    fn new_pp() -> Self {
        StepCollector::PP(Default::default())
    }

    fn new_pvpc() -> Self {
        StepCollector::PVPC(Default::default())
    }

    fn insert_message(&mut self, message: ConsensusMessage) -> Result<bool, DoubleVote> {
        match self {
            StepCollector::PP(pp_collector) => pp_collector.message_collector.insert(message),
            StepCollector::PVPC(pv_pc_collector) => pv_pc_collector.message_collector.insert(message),
        }
    }

    fn insert_priority(&mut self, info: PriorityInfo) -> bool {
        match self {
            StepCollector::PP(pp_collector) => pp_collector.priority_collector.insert(info),
            _ => panic!("Invariant violated: propose step must be linked to PpCollector"),
        }
    }

    fn message_collector(&self) -> &MessageCollector {
        match self {
            StepCollector::PP(pp_collector) => &pp_collector.message_collector,
            StepCollector::PVPC(pv_pc_collector) => &pv_pc_collector.message_collector,
        }
    }

    fn priority_collector(&self) -> &PriorityCollector {
        match self {
            StepCollector::PP(pp_collector) => &pp_collector.priority_collector,
            _ => panic!("Invariant violated: propose step must be linked to PpCollector"),
        }
    }
}

// Struct for propose step vote and priority collecting
#[derive(Debug, Default)]
struct PpCollector {
    message_collector: MessageCollector,
    priority_collector: PriorityCollector,
}

#[derive(Debug, Default)]
struct PvPcCollector {
    message_collector: MessageCollector,
}

#[derive(Debug, Default)]
struct PriorityCollector {
    priorities: BTreeSet<PriorityInfo>,
}

#[derive(Debug, Default)]
struct MessageCollector {
    voted: HashMap<usize, ConsensusMessage>,
    block_votes: HashMap<Option<BlockHash>, BTreeMap<usize, SchnorrSignature>>,
    messages: Vec<ConsensusMessage>,
}

#[derive(Debug)]
pub struct DoubleVote {
    author_index: usize,
    vote_one: ConsensusMessage,
    vote_two: ConsensusMessage,
}

impl DoubleVote {
    pub fn to_action(&self) -> Action {
        Action::ReportDoubleVote {
            message1: Box::new(self.vote_one.clone()),
            message2: Box::new(self.vote_two.clone()),
        }
    }
}

impl Encodable for DoubleVote {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&self.vote_one).append(&self.vote_two);
    }
}

impl PriorityCollector {
    // true: a priority is new
    // false: a priority is duplicated
    fn insert(&mut self, info: PriorityInfo) -> bool {
        self.priorities.insert(info)
    }

    fn get_highest(&self) -> Option<PriorityInfo> {
        self.priorities.iter().rev().next().cloned()
    }

    fn iter_from_highest(&self) -> Rev<Iter<'_, PriorityInfo>> {
        self.priorities.iter().rev()
    }
}

impl MessageCollector {
    /// Some(true): a message is new
    /// Some(false): a message is duplicated
    /// Err(DoubleVote): a double vote
    fn insert(&mut self, message: ConsensusMessage) -> Result<bool, DoubleVote> {
        // Do nothing when message was seen.
        if self.messages.contains(&message) {
            return Ok(false)
        }
        self.messages.push(message.clone());
        if let Some(previous) = self.voted.insert(message.signer_index(), message.clone()) {
            // Bad validator sent a different message.
            Err(DoubleVote {
                author_index: message.signer_index(),
                vote_one: previous,
                vote_two: message,
            })
        } else {
            self.block_votes
                .entry(message.block_hash())
                .or_default()
                .insert(message.signer_index(), message.signature());
            Ok(true)
        }
    }

    /// Count all votes for the given block hash at this round.
    fn count_block(&self, block_hash: &Option<BlockHash>) -> BitSet {
        let mut result = BitSet::new();
        if let Some(votes) = self.block_votes.get(block_hash) {
            for index in votes.keys() {
                result.set(*index);
            }
        }
        result
    }

    /// Count all votes collected for the given round.
    fn count(&self) -> BitSet {
        let mut result = BitSet::new();
        for votes in self.block_votes.values() {
            for index in votes.keys() {
                assert!(!result.is_set(*index), "Cannot vote twice in a round");
                result.set(*index);
            }
        }
        result
    }

    /// get a ConsensusMessage corresponding to a certain index.
    fn fetch_by_idx(&self, idx: usize) -> Option<ConsensusMessage> {
        self.voted.get(&idx).cloned()
    }
}

impl Default for VoteCollector {
    fn default() -> Self {
        let mut collector = BTreeMap::new();
        // Insert dummy entry to fulfill invariant: "only messages newer than the oldest are inserted".
        collector.insert(Default::default(), StepCollector::new_pp());
        VoteCollector {
            votes: collector,
        }
    }
}

impl VoteCollector {
    /// Insert vote if it is newer than the oldest one.
    pub fn collect(&mut self, message: ConsensusMessage) -> Result<bool, DoubleVote> {
        match message.round().step {
            Step::Propose => {
                self.votes.entry(*message.round()).or_insert_with(StepCollector::new_pp).insert_message(message)
            }
            _ => self.votes.entry(*message.round()).or_insert_with(StepCollector::new_pvpc).insert_message(message),
        }
    }

    /// Checks if the message should be ignored.
    pub fn is_old_or_known(&self, message: &ConsensusMessage) -> bool {
        let is_known =
            self.votes.get(&message.round()).map_or(false, |c| c.message_collector().messages.contains(message));
        if is_known {
            cdebug!(ENGINE, "Known message: {:?}.", message);
            return true
        }

        // The reason not using `message.round() <= oldest` is to allow precommit messages on Commit step.
        let is_old = self.votes.keys().next().map_or(true, |oldest| message.round() < oldest);
        if is_old {
            cdebug!(ENGINE, "Old message {:?}.", message);
            return true
        }

        false
    }

    /// Throws out messages older than message, leaves message as marker for the oldest.
    pub fn throw_out_old(&mut self, vote_round: &VoteStep) {
        let new_collector = self.votes.split_off(vote_round);
        assert!(!new_collector.is_empty());
        self.votes = new_collector;
    }

    /// Collects the signatures and the indices for the given round and hash.
    /// Returning indices is in ascending order, and signature and indices are matched with another.
    pub fn round_signatures_and_indices(
        &self,
        round: &VoteStep,
        block_hash: &BlockHash,
    ) -> (Vec<SchnorrSignature>, Vec<usize>) {
        self.votes
            .get(round)
            .and_then(|c| c.message_collector().block_votes.get(&Some(*block_hash)))
            .map(|votes| {
                let (indices, sigs) = votes.iter().unzip();
                (sigs, indices)
            })
            .unwrap_or_default()
    }


    /// Returns the first signature and the index of its signer for a given round and hash if exists.
    pub fn round_signature(&self, round: &VoteStep, block_hash: &BlockHash) -> Option<SchnorrSignature> {
        self.votes
            .get(round)
            .and_then(|c| c.message_collector().block_votes.get(&Some(*block_hash)))
            .and_then(|votes| votes.values().next().cloned())
    }

    /// Count votes which agree with the given message.
    pub fn aligned_votes(&self, message: &ConsensusMessage) -> BitSet {
        if let Some(votes) = self.votes.get(&message.round()) {
            votes.message_collector().count_block(&message.block_hash())
        } else {
            Default::default()
        }
    }

    pub fn block_round_votes(&self, round: &VoteStep, block_hash: &Option<BlockHash>) -> BitSet {
        if let Some(votes) = self.votes.get(round) {
            votes.message_collector().count_block(block_hash)
        } else {
            Default::default()
        }
    }

    /// Count all votes collected for a given round.
    pub fn round_votes(&self, vote_round: &VoteStep) -> BitSet {
        if let Some(votes) = self.votes.get(vote_round) {
            votes.message_collector().count()
        } else {
            Default::default()
        }
    }

    pub fn has_votes_for(&self, round: &VoteStep, block_hash: BlockHash) -> bool {
        let votes = self
            .votes
            .get(round)
            .map(|c| c.message_collector().block_votes.keys().cloned().filter_map(|x| x).collect())
            .unwrap_or_else(Vec::new);
        votes.into_iter().any(|vote_block_hash| vote_block_hash == block_hash)
    }

    pub fn fetch_by_idx(&self, round: &VoteStep, idx: usize) -> Option<ConsensusMessage> {
        self.votes.get(round).and_then(|collector| collector.message_collector().fetch_by_idx(idx))
    }

    pub fn get_all(&self) -> Vec<ConsensusMessage> {
        self.votes.iter().flat_map(|(_round, collector)| collector.message_collector().messages.clone()).collect()
    }

    pub fn get_all_votes_in_round(&self, round: &VoteStep) -> Vec<ConsensusMessage> {
        self.votes.get(round).map(|c| c.message_collector().messages.clone()).unwrap_or_default()
    }

    pub fn get_all_votes_and_indices_in_round(&self, round: &VoteStep) -> Vec<(usize, ConsensusMessage)> {
        self.votes
            .get(round)
            .map(|c| c.message_collector().voted.iter().map(|(k, v)| (*k, v.clone())).collect())
            .unwrap_or_default()
    }
}

impl VoteCollector {
    pub fn collect_priority(&mut self, sortition_round: SortitionRound, info: PriorityInfo) -> bool {
        self.votes.entry(sortition_round.into()).or_insert_with(StepCollector::new_pp).insert_priority(info)
    }

    pub fn get_highest_priority_info(&self, sortition_round: SortitionRound) -> Option<PriorityInfo> {
        self.votes
            .get(&sortition_round.into())
            .and_then(|step_collector| step_collector.priority_collector().get_highest())
    }

    pub fn get_highest_priority(&self, sortition_round: SortitionRound) -> Option<Priority> {
        self.get_highest_priority_info(sortition_round).map(|priority_info| priority_info.priority())
    }

    pub fn get_highest_proposal_hash(&self, sortition_round: SortitionRound) -> Option<BlockHash> {
        self.votes.get(&sortition_round.into()).and_then(|step_collector| {
            let highest_priority_idx =
                step_collector.priority_collector().get_highest().map(|priority_info| priority_info.signer_idx())?;
            step_collector
                .message_collector()
                .fetch_by_idx(highest_priority_idx)
                .and_then(|priority_message| priority_message.block_hash())
        })
    }

    pub fn get_highest_proposal_summary(&self, sortition_round: SortitionRound) -> Option<ProposalSummary> {
        let block_hash = self.get_highest_proposal_hash(sortition_round)?;
        let priority_info = self.get_highest_priority_info(sortition_round)?;
        Some(ProposalSummary {
            priority_info,
            block_hash,
        })
    }

    pub fn block_hashes_from_highest(&self, sortition_round: SortitionRound) -> Vec<BlockHash> {
        match self.votes.get(&sortition_round.into()) {
            Some(step_collector) => {
                let message_collector = step_collector.message_collector();
                let priority_iter_from_highest = step_collector.priority_collector().iter_from_highest();
                priority_iter_from_highest
                    .map(|priority_info| {
                        message_collector
                            .fetch_by_idx(priority_info.signer_idx())
                            .expect("Signer index was verified")
                            .block_hash()
                            .expect("Proposal vote always have BlockHash")
                    })
                    .collect()
            }
            None => vec![],
        }
    }
}
