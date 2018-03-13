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

use std::cmp;

use cbytes::Bytes;
use ctypes::{H256, H520};
use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

use super::{Height, View, BlockHash, Step};
use super::super::vote_collector::Message;

/// Complete step of the consensus process.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct VoteStep {
    pub height: Height,
    pub view: View,
    pub step: Step,
}

impl VoteStep {
    pub fn new(height: Height, view: View, step: Step) -> Self {
        VoteStep { height: height, view: view, step: step }
    }

    pub fn is_height(&self, height: Height) -> bool {
        self.height == height
    }

    pub fn is_view(&self, height: Height, view: View) -> bool {
        self.height == height && self.view == view
    }
}

impl Default for VoteStep {
    fn default() -> Self {
        VoteStep::new(0, 0, Step::Propose)
    }
}

impl PartialOrd for VoteStep {
    fn partial_cmp(&self, m: &VoteStep) -> Option<cmp::Ordering> {
        Some(self.cmp(m))
    }
}

impl Ord for VoteStep {
    fn cmp(&self, m: &VoteStep) -> cmp::Ordering {
        if self.height != m.height {
            self.height.cmp(&m.height)
        } else if self.view != m.view {
            self.view.cmp(&m.view)
        } else {
            self.step.number().cmp(&m.step.number())
        }
    }
}

/// Message transmitted between consensus participants.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ConsensusMessage {
    pub vote_step: VoteStep,
    pub block_hash: Option<BlockHash>,
    pub signature: H520,
}

impl ConsensusMessage {
    pub fn new(signature: H520, height: Height, view: View, step: Step, block_hash: Option<BlockHash>) -> Self {
        ConsensusMessage {
            signature,
            block_hash,
            vote_step: VoteStep::new(height, view, step),
        }
    }
}

impl Message for ConsensusMessage {
    type Round = VoteStep;

    fn signature(&self) -> H520 { self.signature }

    fn block_hash(&self) -> Option<H256> { self.block_hash }

    fn round(&self) -> &VoteStep { &self.vote_step }

    fn is_broadcastable(&self) -> bool { self.vote_step.step.is_pre() }
}

/// (signature, (height, view, step, block_hash))
impl Decodable for ConsensusMessage {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let m = rlp.at(1)?;
        let block_message: H256 = m.val_at(3)?;
        Ok(ConsensusMessage {
            vote_step: VoteStep::new(m.val_at(0)?, m.val_at(1)?, m.val_at(2)?),
            block_hash: match block_message.is_zero() {
                true => None,
                false => Some(block_message),
            },
            signature: rlp.val_at(0)?,
        })
    }
}

impl Encodable for ConsensusMessage {
    fn rlp_append(&self, s: &mut RlpStream) {
        let info = message_info_rlp(&self.vote_step, self.block_hash);
        s.begin_list(2)
            .append(&self.signature)
            .append_raw(&info, 1);
    }
}

pub fn message_info_rlp(vote_step: &VoteStep, block_hash: Option<BlockHash>) -> Bytes {
    let mut s = RlpStream::new_list(4);
    s.append(&vote_step.height).append(&vote_step.view).append(&vote_step.step).append(&block_hash.unwrap_or_else(H256::zero));
    s.out()
}

#[cfg(test)]
mod tests {
    use super::super::Step;
    use super::*;

    #[test]
    fn step_ordering() {
        assert!(VoteStep::new(10, 123, Step::Precommit) < VoteStep::new(11, 123, Step::Precommit));
        assert!(VoteStep::new(10, 123, Step::Propose) < VoteStep::new(11, 123, Step::Precommit));
        assert!(VoteStep::new(10, 122, Step::Propose) < VoteStep::new(11, 123, Step::Propose));
    }
}
