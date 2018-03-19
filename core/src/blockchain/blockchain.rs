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

use std::collections::HashMap;

use cbytes::Bytes;
use ctypes::H256;
use parking_lot::RwLock;

use super::best_block::BestBlock;

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct BlockChain {
    // All locks must be captured in the order declared here.
    best_block: RwLock<BestBlock>,

    // block cache
    block_headers: RwLock<HashMap<H256, Bytes>>,
    block_bodies: RwLock<HashMap<H256, Bytes>>,
}

impl BlockChain {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &[u8]) -> BlockChain {
        Self {
            best_block: RwLock::new(BestBlock::default()),
            block_headers: RwLock::new(HashMap::new()),
            block_bodies: RwLock::new(HashMap::new()),
        }
    }
}

