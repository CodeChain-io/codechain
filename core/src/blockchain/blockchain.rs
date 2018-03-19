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
use super::extras::TransactionAddress;
use super::super::header::{BlockNumber, Header};
use super::super::blockchain_info::BlockChainInfo;

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

    /// Returns general blockchain information
    pub fn chain_info(&self) -> BlockChainInfo {
        // ensure data consistently by locking everything first
        let best_block = self.best_block.read();
        BlockChainInfo {
            total_score: best_block.total_score.clone(),
            genesis_hash: self.genesis_hash(),
            best_block_hash: best_block.hash,
            best_block_number: best_block.number,
            best_block_timestamp: best_block.timestamp,
        }
    }
}

/// Interface for querying blocks by hash and by number.
pub trait BlockProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known(&self, hash: &H256) -> bool;

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256>;

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress>;

    /// Returns reference to genesis hash.
    fn genesis_hash(&self) -> H256 {
        self.block_hash(0).expect("Genesis hash should always exist")
    }
}

impl BlockProvider for BlockChain {
    fn is_known(&self, hash: &H256) -> bool {
        unimplemented!();
        true
    }

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        unimplemented!();
    }

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        unimplemented!();
    }
}

