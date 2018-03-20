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
use std::sync::Arc;

use cbytes::Bytes;
use ctypes::H256;
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;
use rlp::{Rlp, RlpStream};
use rlp_compress::{decompress, blocks_swapper};

use super::best_block::BestBlock;
use super::extras::{BlockDetails, TransactionAddress};
use super::super::blockchain_info::BlockChainInfo;
use super::super::db::{self, Readable, Writable};
use super::super::encoded;
use super::super::header::Header;
use super::super::transaction::{LocalizedTransaction};
use super::super::types::BlockNumber;

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct BlockChain {
    // All locks must be captured in the order declared here.
    best_block: RwLock<BestBlock>,

    // block cache
    block_headers: RwLock<HashMap<H256, Bytes>>,
    block_bodies: RwLock<HashMap<H256, Bytes>>,

    // extra caches
    block_details: RwLock<HashMap<H256, BlockDetails>>,
    block_hashes: RwLock<HashMap<BlockNumber, H256>>,
    transaction_addresses: RwLock<HashMap<H256, TransactionAddress>>,

    db: Arc<KeyValueDB>,
}

impl BlockChain {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &[u8], db: Arc<KeyValueDB>) -> BlockChain {
        Self {
            best_block: RwLock::new(BestBlock::default()),
            block_headers: RwLock::new(HashMap::new()),
            block_bodies: RwLock::new(HashMap::new()),
            block_details: RwLock::new(HashMap::new()),
            block_hashes: RwLock::new(HashMap::new()),
            transaction_addresses: RwLock::new(HashMap::new()),
            db: db.clone(),
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

    /// Create a block body from a block.
    pub fn block_to_body(block: &[u8]) -> Bytes {
        let mut body = RlpStream::new_list(1);
        let block_rlp = Rlp::new(block);
        body.append_raw(block_rlp.at(1).as_raw(), 1);
        body.out()
    }
}

/// Interface for querying blocks by hash and by number.
pub trait BlockProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known(&self, hash: &H256) -> bool;

    /// Get raw block data
    fn block(&self, hash: &H256) -> Option<encoded::Block>;

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails>;

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256>;

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress>;

    /// Get the partial-header of a block.
    fn block_header(&self, hash: &H256) -> Option<Header> {
        self.block_header_data(hash).map(|header| header.decode())
    }

    /// Get the header RLP of a block.
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header>;

    /// Get the block body (uncles and transactions).
    fn block_body(&self, hash: &H256) -> Option<encoded::Body>;

    /// Get the number of given block's hash.
    fn block_number(&self, hash: &H256) -> Option<BlockNumber> {
        self.block_details(hash).map(|details| details.number)
    }

    /// Get transaction with given transaction hash.
    fn transaction(&self, address: &TransactionAddress) -> Option<LocalizedTransaction> {
        self.block_body(&address.block_hash)
            .and_then(|body| self.block_number(&address.block_hash)
                .and_then(|n| body.view().localized_transaction_at(&address.block_hash, n, address.index)))
    }

    /// Get a list of transactions for a given block.
    /// Returns None if block does not exist.
    fn transactions(&self, hash: &H256) -> Option<Vec<LocalizedTransaction>> {
        self.block_body(hash)
            .and_then(|body| self.block_number(hash)
                .map(|n| body.view().localized_transactions(hash, n)))
    }

    /// Returns reference to genesis hash.
    fn genesis_hash(&self) -> H256 {
        self.block_hash(0).expect("Genesis hash should always exist")
    }

    /// Returns the header of the genesis block.
    fn genesis_header(&self) -> Header {
        self.block_header(&self.genesis_hash())
            .expect("Genesis header always stored; qed")
    }
}

impl BlockProvider for BlockChain {
    fn is_known(&self, hash: &H256) -> bool {
        self.db.exists_with_cache(db::COL_EXTRA, &self.block_details, hash)
    }

    /// Get raw block data
    fn block(&self, hash: &H256) -> Option<encoded::Block> {
        let header = self.block_header_data(hash)?;
        let body = self.block_body(hash)?;

        let mut block = RlpStream::new_list(2);
        let body_rlp = body.rlp();
        block.append_raw(header.rlp().as_raw(), 1);
        block.append_raw(body_rlp.at(0).as_raw(), 1);
        Some(encoded::Block::new(block.out()))
    }

    /// Get block header data
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
        // Check cache first
        {
            let read = self.block_headers.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Header::new(v.clone()));
            }
        }

        // Check if it's the best block
        {
            let best_block = self.best_block.read();
            if &best_block.hash == hash {
                return Some(encoded::Header::new(
                    Rlp::new(&best_block.block).at(0).as_raw().to_vec()
                ))
            }
        }

        // Read from DB and populate cache
        let b = self.db.get(db::COL_HEADERS, hash)
            .expect("Low level database error. Some issue with disk?")?;

        let bytes = decompress(&b, blocks_swapper()).into_vec();
        let mut write = self.block_headers.write();
        write.insert(*hash, bytes.clone());

        Some(encoded::Header::new(bytes))
    }

    /// Get block body data
    fn block_body(&self, hash: &H256) -> Option<encoded::Body> {
        // Check cache first
        {
            let read = self.block_bodies.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Body::new(v.clone()));
            }
        }

        // Check if it's the best block
        {
            let best_block = self.best_block.read();
            if &best_block.hash == hash {
                return Some(encoded::Body::new(Self::block_to_body(&best_block.block)));
            }
        }

        // Read from DB and populate cache
        let b = self.db.get(db::COL_BODIES, hash)
            .expect("Low level database error. Some issue with disk?")?;

        let bytes = decompress(&b, blocks_swapper()).into_vec();
        let mut write = self.block_bodies.write();
        write.insert(*hash, bytes.clone());

        Some(encoded::Body::new(bytes))
    }

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.block_details, hash)?;
        Some(result)
    }

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.block_hashes, &index)?;
        Some(result)
    }

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.transaction_addresses, hash)?;
        Some(result)
    }
}

