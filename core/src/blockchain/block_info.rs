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

use ctypes::{H256, U256};

use super::super::types::BlockNumber;

/// Brief info about inserted block.
#[derive(Clone)]
pub struct BlockInfo {
    /// Block hash.
    pub hash: H256,
    /// Block number.
    pub number: BlockNumber,
    /// Total block score.
    pub total_score: U256,
    /// Block location in blockchain.
    pub location: BlockLocation
}

/// Describes location of newly inserted block.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockLocation {
    /// It's part of the canon chain.
    CanonChain,
    /// It's not a part of the canon chain.
    Branch,
    /// It's part of the fork which should become canon chain,
    /// because its total difficulty is higher than current
    /// canon chain difficulty.
    BranchBecomingCanonChain(BranchBecomingCanonChainData),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BranchBecomingCanonChainData {
    /// Hash of the newest common ancestor with old canon chain.
    pub ancestor: H256,
    /// Hashes of the blocks between ancestor and this block.
    pub enacted: Vec<H256>,
    /// Hashes of the blocks which were invalidated.
    pub retracted: Vec<H256>,
}

