// Copyright 2018-2019 Kodebox, Inc.
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

use super::block_info::BestHeaderChanged;
use super::extras::BlockDetails;
use super::route::tree_route;
use crate::consensus::CodeChainEngine;
use crate::db::{self, CacheUpdatePolicy, Readable, Writable};
use crate::encoded;
use crate::header::{Header, Seal};
use crate::views::HeaderView;

const BEST_HEADER_KEY: &[u8] = b"best-header";
const HIGHEST_HEADER_KEY: &[u8] = b"highest-header";

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct HeaderChain {
    // All locks must be captured in the order declared here.
    /// The hash of the best block of the canonical chain.
    best_header_hash: RwLock<H256>,
    /// The hash of the block which has the highest score among the blocks
    /// that is/can be the best block of the canonical chain.
    highest_header_hash: RwLock<H256>,

    // cache
    header_cache: RwLock<HashMap<H256, Bytes>>,
    detail_cache: RwLock<HashMap<H256, BlockDetails>>,
    hash_cache: RwLock<HashMap<BlockNumber, H256>>,

    db: Arc<KeyValueDB>,

    pending_best_header_hash: RwLock<Option<H256>>,
    pending_highest_block_hash: RwLock<Option<H256>>,
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
                };

                let mut batch = DBTransaction::new();
                batch.put(db::COL_HEADERS, &hash, genesis.rlp().as_raw());

                batch.write(db::COL_EXTRA, &hash, &details);
                batch.write(db::COL_EXTRA, &genesis.number(), &hash);

                batch.put(db::COL_EXTRA, BEST_HEADER_KEY, &hash);
                batch.put(db::COL_EXTRA, HIGHEST_HEADER_KEY, &hash);
                db.write(batch).expect("Low level database error. Some issue with disk?");
                hash
            }
        };

        let highest_header_hash = H256::from_slice(
            &db.get(db::COL_EXTRA, HIGHEST_HEADER_KEY).unwrap().expect("highest header is set by best header"),
        );

        Self {
            best_header_hash: RwLock::new(best_header_hash),
            highest_header_hash: RwLock::new(highest_header_hash),

            header_cache: RwLock::new(HashMap::new()),
            detail_cache: RwLock::new(HashMap::new()),
            hash_cache: RwLock::new(HashMap::new()),

            db,

            pending_best_header_hash: RwLock::new(None),
            pending_highest_block_hash: RwLock::new(None),
            pending_hashes: RwLock::new(HashMap::new()),
            pending_details: RwLock::new(HashMap::new()),
        }
    }

    /// Inserts the header into backing cache database.
    /// Expects the header to be valid and already verified.
    /// If the header is already known, does nothing.
    // FIXME: Find better return type. Returning `None` at duplication is not natural
    pub fn insert_header(
        &self,
        batch: &mut DBTransaction,
        header: &HeaderView,
        engine: &CodeChainEngine,
    ) -> Option<BestHeaderChanged> {
        let hash = header.hash();

        ctrace!(HEADERCHAIN, "Inserting block header #{}({}) to the headerchain.", header.number(), hash);

        if self.is_known_header(&hash) {
            ctrace!(HEADERCHAIN, "Block header #{}({}) is already known.", header.number(), hash);
            return None
        }

        assert!(self.pending_best_header_hash.read().is_none());
        assert!(self.pending_highest_block_hash.read().is_none());

        // store block in db
        let compressed_header = compress(header.rlp().as_raw(), blocks_swapper());
        batch.put(db::COL_HEADERS, &hash, &compressed_header);

        let best_header_changed = self.best_header_changed(header, engine);

        let new_hashes = self.new_hash_entries(&best_header_changed);
        let new_details = self.new_detail_entries(header);

        let mut pending_best_header_hash = self.pending_best_header_hash.write();
        let mut pending_highest_header_hash = self.pending_highest_block_hash.write();
        if let Some(best_block_hash) = best_header_changed.new_best_hash() {
            batch.put(db::COL_EXTRA, BEST_HEADER_KEY, &best_block_hash);
            *pending_best_header_hash = Some(best_block_hash);

            batch.put(db::COL_EXTRA, HIGHEST_HEADER_KEY, &hash);
            *pending_highest_header_hash = Some(hash);
        }

        let mut pending_hashes = self.pending_hashes.write();
        let mut pending_details = self.pending_details.write();

        batch.extend_with_cache(db::COL_EXTRA, &mut *pending_details, new_details, CacheUpdatePolicy::Overwrite);
        batch.extend_with_cache(db::COL_EXTRA, &mut *pending_hashes, new_hashes, CacheUpdatePolicy::Overwrite);

        Some(best_header_changed)
    }

    /// Apply pending insertion updates
    pub fn commit(&self) {
        ctrace!(HEADERCHAIN, "Committing.");
        let mut pending_best_header_hash = self.pending_best_header_hash.write();
        let mut pending_highest_header_hash = self.pending_highest_block_hash.write();
        let mut pending_write_hashes = self.pending_hashes.write();
        let mut pending_block_details = self.pending_details.write();

        let mut best_header_hash = self.best_header_hash.write();
        let mut highest_header_hash = self.highest_header_hash.write();
        let mut write_block_details = self.detail_cache.write();
        let mut write_hashes = self.hash_cache.write();
        // update best block
        if let Some(hash) = pending_best_header_hash.take() {
            *best_header_hash = hash;
        }
        if let Some(hash) = pending_highest_header_hash.take() {
            *highest_header_hash = hash;
        }

        write_hashes.extend(mem::replace(&mut *pending_write_hashes, HashMap::new()));
        write_block_details.extend(mem::replace(&mut *pending_block_details, HashMap::new()));
    }

    /// This function returns modified block hashes.
    fn new_hash_entries(&self, best_header_changed: &BestHeaderChanged) -> HashMap<BlockNumber, H256> {
        let mut hashes = HashMap::new();

        match best_header_changed {
            BestHeaderChanged::None => (),
            BestHeaderChanged::CanonChainAppended {
                best_header,
            } => {
                let best_header_view = HeaderView::new(best_header);
                hashes.insert(best_header_view.number(), best_header_view.hash());
            }
            BestHeaderChanged::BranchBecomingCanonChain {
                tree_route,
                best_header,
            } => {
                let ancestor_number = self.block_number(&tree_route.ancestor).expect("Ancestor always exist in DB");
                let start_number = ancestor_number + 1;

                for (index, hash) in tree_route.enacted.iter().enumerate() {
                    hashes.insert(start_number + index as BlockNumber, *hash);
                }

                let best_header_view = HeaderView::new(best_header);
                hashes.insert(best_header_view.number(), best_header_view.hash());
            }
        }

        hashes
    }

    /// This function returns modified block details.
    /// Uses the given parent details or attempts to load them from the database.
    fn new_detail_entries(&self, header: &HeaderView) -> HashMap<H256, BlockDetails> {
        let parent_hash = header.parent_hash();
        let parent_details = self.block_details(&parent_hash).expect("Invalid parent hash");

        // create current block details.
        let details = BlockDetails {
            number: header.number(),
            total_score: parent_details.total_score + header.score(),
            parent: parent_hash,
        };

        // write to batch
        let mut block_details = HashMap::new();
        block_details.insert(header.hash(), details);
        block_details
    }

    /// Calculate how best block is changed
    fn best_header_changed(&self, new_header: &HeaderView, engine: &CodeChainEngine) -> BestHeaderChanged {
        let parent_hash_of_new_header = new_header.parent_hash();
        let parent_details_of_new_header = self.block_details(&parent_hash_of_new_header).expect("Invalid parent hash");
        let is_new_best =
            parent_details_of_new_header.total_score + new_header.score() > self.best_header_detail().total_score;

        if is_new_best {
            ctrace!(
                HEADERCHAIN,
                "Block header #{}({}) has higher total score, changing the highest/best chain.",
                new_header.number(),
                new_header.hash()
            );
            // on new best block we need to make sure that all ancestors
            // are moved to "canon chain"
            // find the route between old best block and the new one
            let prev_best_hash = self.best_header_hash();
            let route = tree_route(self, prev_best_hash, parent_hash_of_new_header)
                .expect("blocks being imported always within recent history; qed");

            let new_best_block_hash = engine.get_best_block_from_highest_score_header(&new_header);
            let new_best_header = if new_best_block_hash != new_header.hash() {
                self.block_header(&new_best_block_hash)
                    .expect("Best block is already imported as a branch")
                    .rlp(&Seal::With)
            } else {
                new_header.rlp().as_raw().to_vec()
            };
            match route.retracted.len() {
                0 => BestHeaderChanged::CanonChainAppended {
                    best_header: new_best_header.clone(),
                },
                _ => BestHeaderChanged::BranchBecomingCanonChain {
                    tree_route: route,
                    best_header: new_best_header.clone(),
                },
            }
        } else {
            BestHeaderChanged::None
        }
    }

    /// Update the best block as the given block hash from the commit state
    /// in Tendermint.
    ///
    /// Used in BlockChain::update_best_as_committed().
    pub fn update_best_as_committed(&self, batch: &mut DBTransaction, block_hash: H256) {
        ctrace!(HEADERCHAIN, "Update the best block to {}", block_hash);
        assert!(self.pending_best_header_hash.read().is_none());
        let block_detail = self.block_details(&block_hash).expect("The given hash should exist");
        let mut new_hashes = HashMap::new();
        new_hashes.insert(block_detail.number, block_hash);

        let mut pending_best_header_hash = self.pending_best_header_hash.write();
        batch.put(db::COL_EXTRA, BEST_HEADER_KEY, &block_hash);
        *pending_best_header_hash = Some(block_hash);

        let mut pending_highest_block_hash = self.pending_highest_block_hash.write();
        batch.put(db::COL_EXTRA, HIGHEST_HEADER_KEY, &block_hash);
        *pending_highest_block_hash = Some(block_hash);

        let mut pending_hashes = self.pending_hashes.write();
        batch.extend_with_cache(db::COL_EXTRA, &mut *pending_hashes, new_hashes, CacheUpdatePolicy::Overwrite);
    }

    /// Get best block hash.
    pub fn best_header_hash(&self) -> H256 {
        *self.best_header_hash.read()
    }

    pub fn highest_header_hash(&self) -> H256 {
        *self.highest_header_hash.read()
    }

    pub fn best_header(&self) -> encoded::Header {
        self.block_header_data(&self.best_header_hash()).expect("Best header always exists")
    }

    pub fn best_header_detail(&self) -> BlockDetails {
        self.block_details(&self.best_header_hash()).expect("Best header always exists")
    }

    pub fn highest_header(&self) -> encoded::Header {
        self.block_header_data(&self.highest_header_hash()).expect("Highest header always exists")
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

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.detail_cache, hash)?;
        Some(result)
    }

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        // Highest block should not be accessed by block number.
        if self.best_header().number() < index {
            return None
        }
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.hash_cache, &index)?;
        Some(result)
    }

    /// Get block header data
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
        let result = block_header_data(hash, &self.header_cache, &*self.db).map(encoded::Header::new);
        if let Some(header) = &result {
            debug_assert_eq!(*hash, header.hash());
        }
        result
    }
}

/// Get block header data
fn block_header_data(hash: &H256, header_cache: &RwLock<HashMap<H256, Bytes>>, db: &KeyValueDB) -> Option<Vec<u8>> {
    // Check cache first
    {
        let read = header_cache.read();
        if let Some(v) = read.get(hash) {
            return Some(v.clone())
        }
    }
    // Read from DB and populate cache
    let b = db.get(db::COL_HEADERS, hash).expect("Low level database error. Some issue with disk?")?;

    let bytes = decompress(&b, blocks_swapper()).into_vec();

    let mut write = header_cache.write();
    if let Some(v) = write.get(hash) {
        assert_eq!(&bytes, v);
        return Some(v.clone())
    }

    write.insert(*hash, bytes.clone());

    Some(bytes)
}
