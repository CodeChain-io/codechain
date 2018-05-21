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

use super::headerchain::HeaderProvider;

/// Represents a tree route between `from` block and `to` block:
#[derive(Debug, PartialEq)]
pub struct TreeRoute {
    /// Best common ancestor of these blocks.
    pub ancestor: H256,
    /// A vector of hashes in forward direction
    /// First item of list must be child of ancestor
    pub forward: Vec<H256>,
    /// A vector of hashes in backward direction
    /// Last item of list must be child of ancestor
    pub backward: Vec<H256>,
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
///   { ancestor: A3, forward: [], backward: [A5, A4] }
///   ```
///
/// 2.) from older to newer
/// - bc: `A1 -> A2 -> A3 -> A4 -> A5`
/// - from: A3, to: A5
/// - route:
///   ```json
///   { ancestor: A3, forward: [A4, A5], backward: [] }
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
///   { ancestor: A2, forward: [A3, A4], backward: [B4, B3] }
///   ```
pub fn tree_route<P>(db: &HeaderProvider, from: H256, to: H256, predicate: P) -> Option<TreeRoute>
where
    P: Fn(&H256) -> bool, {
    let mut backward = vec![];
    let mut forward = vec![];

    let mut cur_retract = db.block_header_data(&from)?;
    let mut cur_enact = db.block_header_data(&to)?;

    while cur_retract.number() != cur_enact.number() {
        let (header, vec) = if cur_retract.number() > cur_enact.number() {
            (&mut cur_retract, &mut backward)
        } else {
            (&mut cur_enact, &mut forward)
        };
        if !predicate(&header.hash()) {
            return None
        }
        vec.push(header.hash());
        *header = db.block_header_data(&header.parent_hash())?;
    }

    assert_eq!(cur_retract.number(), cur_enact.number());

    while cur_retract.hash() != cur_enact.hash() {
        if !predicate(&cur_retract.hash()) || !predicate(&cur_enact.hash()) {
            return None
        }
        backward.push(cur_retract.hash());
        forward.push(cur_enact.hash());
        cur_retract = db.block_header_data(&cur_retract.parent_hash())?;
        cur_enact = db.block_header_data(&cur_enact.parent_hash())?;
    }

    forward.reverse();

    Some(TreeRoute {
        ancestor: cur_retract.hash(),
        backward,
        forward,
    })
}

/// Import route for newly inserted block.
#[derive(Debug, PartialEq)]
pub enum ImportRoute {
    Canonical(TreeRoute),
    Branch,
    #[allow(dead_code)]
    Dangling,
    AlreadyInChain,
}

impl<'a> ImportRoute {
    pub fn canonical_route(&'a self) -> Option<&'a TreeRoute> {
        match self {
            ImportRoute::Canonical(tree_route) => Some(tree_route),
            _ => None,
        }
    }

    pub fn enacted(&self) -> Vec<H256> {
        self.canonical_route().map_or(Vec::new(), |route| route.forward.clone())
    }

    pub fn retracted(&self) -> Vec<H256> {
        self.canonical_route().map_or(Vec::new(), |route| route.backward.clone())
    }
}
