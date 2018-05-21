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
use super::headerchain::HeaderProvider;

/// Represents a tree route between `from` block and `to` block:
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRoute {
    /// Best common ancestor of these blocks.
    pub ancestor: H256,
    /// A vector of enacted block hashes
    /// First item of list must be child of ancestor
    pub enacted: Vec<H256>,
    /// A vector of retracted block hashes
    /// Last item of list must be child of ancestor
    pub retracted: Vec<H256>,
}

/// Returns a tree route between `from` and `to`, which is a tuple of:
/// - common ancestor of these blocks
/// - a vector of hashes of blocks in range (ancestor, to]
/// - a vector of hashes of blocks in range [from, ancestor)
///
/// Returns `None` if:
/// - any of the headers in route returns false with provided predicate
/// - no route found
///
/// 1.) from newer to older
/// - bc: `A1 -> A2 -> A3 -> A4 -> A5`
/// - from: A5, to: A3
/// - route:
///   ```json
///   { ancestor: A3, enacted: [], retracted: [A5, A4] }
///   ```
///
/// 2.) from older to newer
/// - bc: `A1 -> A2 -> A3 -> A4 -> A5`
/// - from: A3, to: A5
/// - route:
///   ```json
///   { ancestor: A3, enacted: [A4, A5], retracted: [] }
///   ```
///
/// 3.) fork:
/// - bc:
///   ```text
///   A1 -> A2 -> A3 -> A4
///              -> B3 -> B4
///   ```
/// - from: B4, to: A4
/// - route:
///   ```json
///   { ancestor: A2, enacted: [A3, A4], retracted: [B4, B3] }
///   ```
///
/// If the tree route verges into pruned or unknown blocks,
/// `None` is returned.
pub fn tree_route(db: &HeaderProvider, from: H256, to: H256) -> Option<TreeRoute> {
    let mut retracted = vec![];
    let mut enacted = vec![];

    let mut cur_retract = db.block_header_data(&from)?;
    let mut cur_enact = db.block_header_data(&to)?;

    while cur_retract.number() != cur_enact.number() {
        let (header, vec) = if cur_retract.number() > cur_enact.number() {
            (&mut cur_retract, &mut retracted)
        } else {
            (&mut cur_enact, &mut enacted)
        };
        vec.push(header.hash());
        *header = db.block_header_data(&header.parent_hash())?;
    }

    debug_assert_eq!(cur_retract.number(), cur_enact.number());

    while cur_retract.hash() != cur_enact.hash() {
        retracted.push(cur_retract.hash());
        enacted.push(cur_enact.hash());
        cur_retract = db.block_header_data(&cur_retract.parent_hash())?;
        cur_enact = db.block_header_data(&cur_enact.parent_hash())?;
    }

    debug_assert_eq!(cur_retract.hash(), cur_enact.hash());

    enacted.reverse();

    Some(TreeRoute {
        ancestor: cur_retract.hash(),
        retracted,
        enacted,
    })
}

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
