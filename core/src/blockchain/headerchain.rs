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
use std::mem;
use std::sync::Arc;

use ctypes::BlockNumber;
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;
use primitives::{Bytes, H256};
use rlp_compress::{blocks_swapper, compress, decompress};

use super::super::db::{self, CacheUpdatePolicy, Readable, Writable};
use super::super::encoded;
use super::super::header::Header;
use super::super::views::HeaderView;
use super::block_info::BlockLocation;
use super::extras::BlockDetails;
use super::route::tree_route;

const BEST_HEADER_KEY: &[u8] = b"best-header";

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct HeaderChain {
    // All locks must be captured in the order declared here.
    best_header_hash: RwLock<H256>,

    // cache
    header_cache: RwLock<HashMap<H256, Bytes>>,
    detail_cache: RwLock<HashMap<H256, BlockDetails>>,
    hash_cache: RwLock<HashMap<BlockNumber, H256>>,

    db: Arc<KeyValueDB>,

    pending_best_header_hash: RwLock<Option<H256>>,
    pending_hashes: RwLock<HashMap<BlockNumber, H256>>,
    pending_details: RwLock<HashMap<H256, BlockDetails>>,
}

impl HeaderChain {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &HeaderView, db: Arc<KeyValueDB>) -> Self {
        // load best header
        let best_header_hash = match db.get(db::COL_EXTRA, BEST_HEADER_KEY).unwrap() {
            Some(hash) => H256::from_slice(&hash),
            None => {
                // best header does not exist
                // we need to insert genesis into the cache
                let hash = genesis.hash();

                let details = BlockDetails {
                    number: genesis.number(),
                    total_score: genesis.score(),
                    parent: genesis.parent_hash(),
                    children: vec![],
                };

                let mut batch = DBTransaction::new();
                batch.put(db::COL_HEADERS, &hash, genesis.rlp().as_raw());

                batch.write(db::COL_EXTRA, &hash, &details);
                batch.write(db::COL_EXTRA, &genesis.number(), &hash);

                batch.put(db::COL_EXTRA, BEST_HEADER_KEY, &hash);
                db.write(batch).expect("Low level database error. Some issue with disk?");
                hash
            }
        };

        Self {
            best_header_hash: RwLock::new(best_header_hash),

            header_cache: RwLock::new(HashMap::new()),
            detail_cache: RwLock::new(HashMap::new()),
            hash_cache: RwLock::new(HashMap::new()),

            db,

            pending_best_header_hash: RwLock::new(None),
            pending_hashes: RwLock::new(HashMap::new()),
            pending_details: RwLock::new(HashMap::new()),
        }
    }

    /// Inserts the header into backing cache database.
    /// Expects the header to be valid and already verified.
    /// If the header is already known, does nothing.
    // FIXME: Find better return type. Returning `None` at duplication is not natural
    pub fn insert_header(&self, batch: &mut DBTransaction, header: &HeaderView) -> Option<BlockLocation> {
        let hash = header.hash();

        if self.is_known_header(&hash) {
            return None
        }

        assert!(self.pending_best_header_hash.read().is_none());

        // store block in db
        let compressed_header = compress(header.rlp().as_raw(), blocks_swapper());
        batch.put(db::COL_HEADERS, &hash, &compressed_header);

        let location = self.block_location(header);

        let new_hashes = self.new_hash_entries(header, &location);
        let new_details = self.new_detail_entries(header);

        let mut pending_best_header_hash = self.pending_best_header_hash.write();
        if location != BlockLocation::Branch {
            batch.put(db::COL_EXTRA, BEST_HEADER_KEY, &header.hash());
            *pending_best_header_hash = Some(header.hash());
        }

        let mut pending_hashes = self.pending_hashes.write();
        let mut pending_details = self.pending_details.write();

        batch.extend_with_cache(db::COL_EXTRA, &mut *pending_details, new_details, CacheUpdatePolicy::Overwrite);
        batch.extend_with_cache(db::COL_EXTRA, &mut *pending_hashes, new_hashes, CacheUpdatePolicy::Overwrite);

        Some(location)
    }

    /// Apply pending insertion updates
    pub fn commit(&self) {
        let mut pending_best_header_hash = self.pending_best_header_hash.write();
        let mut pending_write_hashes = self.pending_hashes.write();
        let mut pending_block_details = self.pending_details.write();

        let mut best_header_hash = self.best_header_hash.write();
        let mut write_block_details = self.detail_cache.write();
        let mut write_hashes = self.hash_cache.write();
        // update best block
        if let Some(hash) = pending_best_header_hash.take() {
            *best_header_hash = hash;
        }

        write_hashes.extend(mem::replace(&mut *pending_write_hashes, HashMap::new()));
        write_block_details.extend(mem::replace(&mut *pending_block_details, HashMap::new()));
    }

    /// This function returns modified block hashes.
    fn new_hash_entries(&self, header: &HeaderView, location: &BlockLocation) -> HashMap<BlockNumber, H256> {
        let mut hashes = HashMap::new();
        let number = header.number();

        match location {
            BlockLocation::Branch => (),
            BlockLocation::CanonChain => {
                hashes.insert(number, header.hash());
            }
            BlockLocation::BranchBecomingCanonChain(data) => {
                let ancestor_number = self.block_number(&data.ancestor).expect("Ancestor always exist in DB");
                let start_number = ancestor_number + 1;

                for (index, hash) in data.enacted.iter().cloned().enumerate() {
                    hashes.insert(start_number + index as BlockNumber, hash);
                }

                hashes.insert(number, header.hash());
            }
        }

        hashes
    }

    /// This function returns modified block details.
    /// Uses the given parent details or attempts to load them from the database.
    fn new_detail_entries(&self, header: &HeaderView) -> HashMap<H256, BlockDetails> {
        let parent_hash = header.parent_hash();
        // update parent
        let mut parent_details = self.block_details(&parent_hash).expect("Invalid parent hash");
        parent_details.children.push(header.hash());

        // create current block details.
        let details = BlockDetails {
            number: header.number(),
            total_score: parent_details.total_score + header.score(),
            parent: parent_hash,
            children: vec![],
        };

        // write to batch
        let mut block_details = HashMap::new();
        block_details.insert(parent_hash, parent_details);
        block_details.insert(header.hash(), details);
        block_details
    }

    /// Calculate insert location for new block
    fn block_location(&self, header: &HeaderView) -> BlockLocation {
        let parent_hash = header.parent_hash();
        let parent_details = self.block_details(&parent_hash).expect("Invalid parent hash");
        let is_new_best = parent_details.total_score + header.score() > self.best_header_detail().total_score;

        if is_new_best {
            // on new best block we need to make sure that all ancestors
            // are moved to "canon chain"
            // find the route between old best block and the new one
            let best_hash = self.best_header_hash();
            let route = tree_route(self, best_hash, parent_hash)
                .expect("blocks being imported always within recent history; qed");

            match route.retracted.len() {
                0 => BlockLocation::CanonChain,
                _ => BlockLocation::BranchBecomingCanonChain(route),
            }
        } else {
            BlockLocation::Branch
        }
    }

    /// Get best block hash.
    pub fn best_header_hash(&self) -> H256 {
        self.best_header_hash.read().clone()
    }

    #[allow(dead_code)]
    pub fn best_header(&self) -> encoded::Header {
        self.block_header_data(&self.best_header_hash()).expect("Best header always exists")
    }

    pub fn best_header_detail(&self) -> BlockDetails {
        self.block_details(&self.best_header_hash()).expect("Best header always exists")
    }
}

/// Interface for querying blocks by hash and by number.
pub trait HeaderProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known_header(&self, hash: &H256) -> bool;

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails>;

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256>;

    /// Get the partial-header of a block.
    fn block_header(&self, hash: &H256) -> Option<Header> {
        self.block_header_data(hash).map(|header| header.decode())
    }

    /// Get the header RLP of a block.
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header>;

    /// Get the number of given block's hash.
    fn block_number(&self, hash: &H256) -> Option<BlockNumber> {
        self.block_details(hash).map(|details| details.number)
    }

    /// Returns reference to genesis hash.
    fn genesis_hash(&self) -> H256 {
        self.block_hash(0).expect("Genesis hash should always exist")
    }

    /// Returns the header of the genesis block.
    fn genesis_header(&self) -> Header {
        self.block_header(&self.genesis_hash()).expect("Genesis header always stored; qed")
    }
}

impl HeaderProvider for HeaderChain {
    fn is_known_header(&self, hash: &H256) -> bool {
        self.db.exists_with_cache(db::COL_EXTRA, &self.detail_cache, hash)
    }

    /// Get block header data
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
        // Check cache first
        {
            let read = self.header_cache.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Header::new(v.clone()))
            }
        }

        // Read from DB and populate cache
        let b = self.db.get(db::COL_HEADERS, hash).expect("Low level database error. Some issue with disk?")?;

        let bytes = decompress(&b, blocks_swapper()).into_vec();
        let mut write = self.header_cache.write();
        write.insert(*hash, bytes.clone());

        Some(encoded::Header::new(bytes))
    }

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.detail_cache, hash)?;
        Some(result)
    }

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.hash_cache, &index)?;
        Some(result)
    }
}
