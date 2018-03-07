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
use codechain_types::{H520};
use super::{Height, View, BlockHash, Step};

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
