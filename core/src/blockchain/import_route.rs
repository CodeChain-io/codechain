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

use ctypes::H256;

use super::block_info::BlockLocation;

/// Import route for newly inserted block.
#[derive(Debug, PartialEq)]
pub struct ImportRoute {
    /// Blocks that were invalidated by new block.
    pub retracted: Vec<H256>,
    /// Blocks that were validated by new block.
    pub enacted: Vec<H256>,
    /// Blocks which are neither retracted nor enacted.
    pub omitted: Vec<H256>,
}

impl ImportRoute {
    pub fn new(hash: &H256, location: &BlockLocation) -> Self {
        match location {
            BlockLocation::CanonChain => ImportRoute {
                retracted: vec![],
                enacted: vec![*hash],
                omitted: vec![],
            },
            BlockLocation::Branch => ImportRoute {
                retracted: vec![],
                enacted: vec![],
                omitted: vec![*hash],
            },
            BlockLocation::BranchBecomingCanonChain(data) => {
                let mut enacted = vec![*hash];
                enacted.extend(data.enacted.iter());
                let retracted = data.retracted.clone();
                ImportRoute {
                    retracted,
                    enacted,
                    omitted: vec![],
                }
            }
        }
    }

    pub fn none() -> Self {
        ImportRoute {
            retracted: vec![],
            enacted: vec![],
            omitted: vec![],
        }
    }
}
