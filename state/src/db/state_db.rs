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

// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.
//
// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use std::sync::Arc;

use ckey::Address;
use ctypes::BlockNumber;
use hashdb::HashDB;
use journaldb::{self, Algorithm, JournalDB};
use kvdb::DBTransaction;
use kvdb_memorydb;
use parking_lot::Mutex;
use primitives::H256;
use util_error::UtilError;

use super::global_cache::GlobalCache;
use super::global_cache_buffer::GlobalCacheBuffer;

use super::super::{
    Account, ActionData, ActionHandler, AssetScheme, AssetSchemeAddress, Backend, CacheableItem, Metadata,
    MetadataAddress, OwnedAsset, OwnedAssetAddress, RegularAccount, RegularAccountAddress, Shard, ShardAddress,
    ShardBackend, ShardMetadata, ShardMetadataAddress, TopBackend, World, WorldAddress,
};

// The percentage of supplied cache size to go to accounts.
const ACCOUNT_CACHE_RATIO: usize = 35;
const REGULAR_ACCOUNT_CACHE_RATIO: usize = 5;
const METADATA_CACHE_RATIO: usize = 1;
const SHARD_CACHE_RATIO: usize = 6;
const SHARD_METADATA_CACHE_RATIO: usize = 1;
const WORLD_CACHE_RATIO: usize = 1;
const ASSET_SCHEME_CACHE_RATIO: usize = 10;
const ASSET_CACHE_RATIO: usize = 40;
const ACTION_DATA_CACHE_RATIO: usize = 1;

/// State database abstraction.
/// Manages shared global state cache which reflects the canonical
/// state as it is on the disk. All the entries in the cache are clean.
/// A clone of `StateDB` may be created as canonical or not.
/// For canonical clones local cache is accumulated and applied
/// in `sync_cache`
/// For non-canonical clones local cache is dropped.
///
/// Global cache propagation.
/// After a `State` object has been committed to the trie it
/// propagates its local cache into the `StateDB` local cache
/// using `add_to_account_cache` function.
/// Then, after the block has been added to the chain the local cache in the
/// `StateDB` is propagated into the global cache.
pub struct StateDB {
    /// Backing database.
    db: Box<JournalDB>,
    /// Shared canonical state cache.
    account_cache: Arc<Mutex<GlobalCache<Account>>>,
    regular_account_cache: Arc<Mutex<GlobalCache<RegularAccount>>>,
    metadata_cache: Arc<Mutex<GlobalCache<Metadata>>>,
    shard_cache: Arc<Mutex<GlobalCache<Shard>>>,
    shard_metadata_cache: Arc<Mutex<GlobalCache<ShardMetadata>>>,
    world_cache: Arc<Mutex<GlobalCache<World>>>,
    asset_scheme_cache: Arc<Mutex<GlobalCache<AssetScheme>>>,
    asset_cache: Arc<Mutex<GlobalCache<OwnedAsset>>>,
    action_data_cache: Arc<Mutex<GlobalCache<ActionData>>>,

    /// Local dirty cache.
    account_cache_buffer: GlobalCacheBuffer<Account>,
    regular_account_cache_buffer: GlobalCacheBuffer<RegularAccount>,
    metadata_cache_buffer: GlobalCacheBuffer<Metadata>,
    shard_cache_buffer: GlobalCacheBuffer<Shard>,
    shard_metadata_cache_buffer: GlobalCacheBuffer<ShardMetadata>,
    world_cache_buffer: GlobalCacheBuffer<World>,
    asset_scheme_cache_buffer: GlobalCacheBuffer<AssetScheme>,
    asset_cache_buffer: GlobalCacheBuffer<OwnedAsset>,
    action_data_cache_buffer: GlobalCacheBuffer<ActionData>,

    /// Hash of the block on top of which this instance was created or
    /// `None` if cache is disabled
    parent_hash: Option<H256>,
    /// Hash of the committing block or `None` if not committed yet.
    commit_hash: Option<H256>,
    /// Number of the committing block or `None` if not committed yet.
    commit_number: Option<BlockNumber>,

    custom_handlers: Vec<Arc<ActionHandler>>,
}

impl StateDB {
    /// Create a new instance wrapping `JournalDB` and the maximum allowed size
    /// of the LRU cache in bytes. Actual used memory may (read: will) be higher due to bookkeeping.
    // TODO: make the cache size actually accurate by moving the account storage cache
    // into the `AccountCache` structure as its own `LruCache<(Address, H256), H256>`.
    pub fn new(db: Box<JournalDB>, cache_size: usize, custom_handlers: Vec<Arc<ActionHandler>>) -> StateDB {
        assert_eq!(
            100,
            ACCOUNT_CACHE_RATIO
                + METADATA_CACHE_RATIO
                + SHARD_CACHE_RATIO
                + SHARD_METADATA_CACHE_RATIO
                + WORLD_CACHE_RATIO
                + ASSET_SCHEME_CACHE_RATIO
                + ASSET_CACHE_RATIO
                + ACTION_DATA_CACHE_RATIO
                + REGULAR_ACCOUNT_CACHE_RATIO
        );

        let account_cache_size = cache_size * ACCOUNT_CACHE_RATIO / 100;
        let account_cache_items = account_cache_size / ::std::mem::size_of::<Option<Account>>();

        let regular_account_cache_size = cache_size * REGULAR_ACCOUNT_CACHE_RATIO / 100;
        let regular_account_cache_items = regular_account_cache_size / ::std::mem::size_of::<Option<RegularAccount>>();

        let metadata_cache_size = cache_size * METADATA_CACHE_RATIO / 100;
        let metadata_cache_items = metadata_cache_size / ::std::mem::size_of::<Option<Metadata>>();

        let shard_cache_size = cache_size * SHARD_CACHE_RATIO / 100;
        let shard_cache_items = shard_cache_size / ::std::mem::size_of::<Option<Shard>>();

        let shard_metadata_cache_size = cache_size * SHARD_METADATA_CACHE_RATIO / 100;
        let shard_metadata_cache_items = shard_metadata_cache_size / ::std::mem::size_of::<Option<ShardMetadata>>();

        let world_cache_size = cache_size * WORLD_CACHE_RATIO / 100;
        let world_cache_items = world_cache_size / ::std::mem::size_of::<Option<World>>();

        let asset_scheme_cache_size = cache_size * ASSET_SCHEME_CACHE_RATIO / 100;
        let asset_scheme_cache_items = asset_scheme_cache_size / ::std::mem::size_of::<Option<AssetScheme>>();

        let asset_cache_size = cache_size * ASSET_CACHE_RATIO / 100;
        let asset_cache_items = asset_cache_size / ::std::mem::size_of::<Option<OwnedAsset>>();

        let action_data_cache_size = cache_size * ACTION_DATA_CACHE_RATIO / 100;
        let action_data_cache_items = action_data_cache_size / ::std::mem::size_of::<Option<ActionData>>();

        StateDB {
            db,
            account_cache: Arc::new(Mutex::new(GlobalCache::new(account_cache_items))),
            regular_account_cache: Arc::new(Mutex::new(GlobalCache::new(regular_account_cache_items))),
            metadata_cache: Arc::new(Mutex::new(GlobalCache::new(metadata_cache_items))),
            shard_cache: Arc::new(Mutex::new(GlobalCache::new(shard_cache_items))),
            shard_metadata_cache: Arc::new(Mutex::new(GlobalCache::new(shard_metadata_cache_items))),
            world_cache: Arc::new(Mutex::new(GlobalCache::new(world_cache_items))),
            asset_scheme_cache: Arc::new(Mutex::new(GlobalCache::new(asset_scheme_cache_items))),
            asset_cache: Arc::new(Mutex::new(GlobalCache::new(asset_cache_items))),
            action_data_cache: Arc::new(Mutex::new(GlobalCache::new(action_data_cache_items))),

            account_cache_buffer: GlobalCacheBuffer::new(),
            regular_account_cache_buffer: GlobalCacheBuffer::new(),
            metadata_cache_buffer: GlobalCacheBuffer::new(),
            shard_cache_buffer: GlobalCacheBuffer::new(),
            shard_metadata_cache_buffer: GlobalCacheBuffer::new(),
            world_cache_buffer: GlobalCacheBuffer::new(),
            asset_scheme_cache_buffer: GlobalCacheBuffer::new(),
            asset_cache_buffer: GlobalCacheBuffer::new(),
            action_data_cache_buffer: GlobalCacheBuffer::new(),
            parent_hash: None,
            commit_hash: None,
            commit_number: None,
            custom_handlers,
        }
    }

    pub fn new_with_memorydb(cache_size: usize, custom_handlers: Vec<Arc<ActionHandler>>) -> Self {
        let memorydb = Arc::new(kvdb_memorydb::create(0));
        StateDB::new(journaldb::new(memorydb, Algorithm::Archive, None), cache_size, custom_handlers)
    }

    /// Journal all recent operations under the given era and ID.
    pub fn journal_under(&mut self, batch: &mut DBTransaction, now: u64, id: &H256) -> Result<u32, UtilError> {
        let records = self.db.journal_under(batch, now, id)?;
        self.commit_hash = Some(id.clone());
        self.commit_number = Some(now);
        Ok(records)
    }

    /// Mark a given candidate from an ancient era as canonical, enacting its removals from the
    /// backing database and reverting any non-canonical historical commit's insertions.
    pub fn mark_canonical(
        &mut self,
        batch: &mut DBTransaction,
        end_era: u64,
        canon_id: &H256,
    ) -> Result<u32, UtilError> {
        self.db.mark_canonical(batch, end_era, canon_id)
    }

    /// Propagate local cache into the global cache and synchonize
    /// the global cache with the best block state.
    /// This function updates the global cache by removing entries
    /// that are invalidated by chain reorganization. `sync_cache`
    /// should be called after the block has been committed and the
    /// blockchain route has ben calculated.
    pub fn sync_cache(&mut self, enacted: &[H256], retracted: &[H256], is_best: bool) {
        ctrace!(
            STATE_DB,
            "sync_cache id = (#{:?}, {:?}), parent={:?}, best={}",
            self.commit_number,
            self.commit_hash,
            self.parent_hash,
            is_best
        );

        let mut account_cache = self.account_cache.lock();
        let mut regular_account_cache = self.regular_account_cache.lock();
        let mut shard_cache = self.shard_cache.lock();
        let mut shard_metadata_cache = self.shard_metadata_cache.lock();
        let mut world_cache = self.world_cache.lock();
        let mut asset_scheme_cache = self.asset_scheme_cache.lock();
        let mut asset_cache = self.asset_cache.lock();
        let mut action_data_cache = self.action_data_cache.lock();

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut account_cache,
            &mut self.account_cache_buffer,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut regular_account_cache,
            &mut self.regular_account_cache_buffer,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut shard_cache,
            &mut self.shard_cache_buffer,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut shard_metadata_cache,
            &mut self.shard_metadata_cache_buffer,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut world_cache,
            &mut self.world_cache_buffer,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut asset_scheme_cache,
            &mut self.asset_scheme_cache_buffer,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut asset_cache,
            &mut self.asset_cache_buffer,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut action_data_cache,
            &mut self.action_data_cache_buffer,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );
    }

    fn sync_cache_impl<Item>(
        enacted: &[H256],
        retracted: &[H256],
        is_best: bool,
        cache: &mut GlobalCache<Item>,
        cache_buffer: &mut GlobalCacheBuffer<Item>,
        parent_hash: &Option<H256>,
        commit_hash: &Option<H256>,
        commit_number: &Option<BlockNumber>,
    ) where
        Item: CacheableItem, {
        // Purge changes from re-enacted and retracted blocks.
        // Filter out committing block if any.
        let clear =
            !(enacted.iter().filter(|h| commit_hash.as_ref().map_or(true, |p| *h != p)).all(|block| cache.enact(block))
                && retracted.iter().all(|block| cache.retract(block)));
        if clear {
            // We don't know anything about the block; clear everything
            ctrace!(STATE_DB, "Wiping cache");
            cache.clear();
        }

        // Propagate cache only if committing on top of the latest canonical state
        // blocks are ordered by number and only one block with a given number is marked as canonical
        // (contributed to canonical state cache)
        if let (Some(number), Some(hash), Some(parent)) = (commit_number, commit_hash, parent_hash) {
            cache_buffer.sync_cache(cache, *number, *hash, *parent, is_best);
        }
    }

    /// Conversion method to interpret self as `HashDB` reference
    pub fn as_hashdb(&self) -> &HashDB {
        self.db.as_hashdb()
    }

    /// Conversion method to interpret self as mutable `HashDB` reference
    pub fn as_hashdb_mut(&mut self) -> &mut HashDB {
        self.db.as_hashdb_mut()
    }

    pub fn clone_with_immutable_global_cache(&self) -> Self {
        Self {
            db: self.db.boxed_clone(),
            account_cache: Arc::clone(&self.account_cache),
            regular_account_cache: Arc::clone(&self.regular_account_cache),
            metadata_cache: Arc::clone(&self.metadata_cache),
            shard_cache: Arc::clone(&self.shard_cache),
            shard_metadata_cache: Arc::clone(&self.shard_metadata_cache),
            world_cache: Arc::clone(&self.world_cache),
            asset_scheme_cache: Arc::clone(&self.asset_scheme_cache),
            asset_cache: Arc::clone(&self.asset_cache),
            action_data_cache: Arc::clone(&self.action_data_cache),

            account_cache_buffer: GlobalCacheBuffer::new(),
            regular_account_cache_buffer: GlobalCacheBuffer::new(),
            metadata_cache_buffer: GlobalCacheBuffer::new(),
            shard_cache_buffer: GlobalCacheBuffer::new(),
            shard_metadata_cache_buffer: GlobalCacheBuffer::new(),
            world_cache_buffer: GlobalCacheBuffer::new(),
            asset_scheme_cache_buffer: GlobalCacheBuffer::new(),
            asset_cache_buffer: GlobalCacheBuffer::new(),
            action_data_cache_buffer: GlobalCacheBuffer::new(),

            parent_hash: None,
            commit_hash: None,
            commit_number: None,

            custom_handlers: self.custom_handlers.clone(),
        }
    }

    pub fn clone_with_mutable_global_cache(&self) -> StateDB {
        StateDB {
            db: self.db.boxed_clone(),
            account_cache: Arc::clone(&self.account_cache),
            regular_account_cache: Arc::clone(&self.regular_account_cache),
            metadata_cache: Arc::clone(&self.metadata_cache),
            shard_cache: Arc::clone(&self.shard_cache),
            shard_metadata_cache: Arc::clone(&self.shard_metadata_cache),
            world_cache: Arc::clone(&self.world_cache),
            asset_scheme_cache: Arc::clone(&self.asset_scheme_cache),
            asset_cache: Arc::clone(&self.asset_cache),
            action_data_cache: Arc::clone(&self.action_data_cache),

            account_cache_buffer: GlobalCacheBuffer::new(),
            regular_account_cache_buffer: GlobalCacheBuffer::new(),
            metadata_cache_buffer: GlobalCacheBuffer::new(),
            shard_cache_buffer: GlobalCacheBuffer::new(),
            shard_metadata_cache_buffer: GlobalCacheBuffer::new(),
            world_cache_buffer: GlobalCacheBuffer::new(),
            asset_scheme_cache_buffer: GlobalCacheBuffer::new(),
            asset_cache_buffer: GlobalCacheBuffer::new(),
            action_data_cache_buffer: GlobalCacheBuffer::new(),

            parent_hash: self.parent_hash.clone(),
            commit_hash: None,
            commit_number: None,
            custom_handlers: self.custom_handlers.clone(),
        }
    }

    /// Clone the database for a canonical state.
    pub fn clone_canon(&self, parent: &H256) -> StateDB {
        StateDB {
            db: self.db.boxed_clone(),
            account_cache: Arc::clone(&self.account_cache),
            regular_account_cache: Arc::clone(&self.regular_account_cache),
            metadata_cache: Arc::clone(&self.metadata_cache),
            shard_cache: Arc::clone(&self.shard_cache),
            shard_metadata_cache: Arc::clone(&self.shard_metadata_cache),
            world_cache: Arc::clone(&self.world_cache),
            asset_scheme_cache: Arc::clone(&self.asset_scheme_cache),
            asset_cache: Arc::clone(&self.asset_cache),
            action_data_cache: Arc::clone(&self.action_data_cache),

            account_cache_buffer: GlobalCacheBuffer::new(),
            regular_account_cache_buffer: GlobalCacheBuffer::new(),
            metadata_cache_buffer: GlobalCacheBuffer::new(),
            shard_cache_buffer: GlobalCacheBuffer::new(),
            shard_metadata_cache_buffer: GlobalCacheBuffer::new(),
            world_cache_buffer: GlobalCacheBuffer::new(),
            asset_scheme_cache_buffer: GlobalCacheBuffer::new(),
            asset_cache_buffer: GlobalCacheBuffer::new(),
            action_data_cache_buffer: GlobalCacheBuffer::new(),

            parent_hash: Some(parent.clone()),
            commit_hash: None,
            commit_number: None,
            custom_handlers: self.custom_handlers.clone(),
        }
    }

    /// Check if pruning is enabled on the database.
    pub fn is_pruned(&self) -> bool {
        self.db.is_pruned()
    }

    fn mem_used_impl<Item>(cache: &GlobalCache<Item>) -> usize
    where
        Item: CacheableItem, {
        let items = cache.len();
        items * ::std::mem::size_of::<Option<Item>>()
    }

    /// Heap size used.
    pub fn mem_used(&self) -> usize {
        // TODO: account for LRU-cache overhead; this is a close approximation.
        self.db.mem_used()
            + Self::mem_used_impl(&self.account_cache.lock())
            + Self::mem_used_impl(&self.regular_account_cache.lock())
            + Self::mem_used_impl(&self.shard_cache.lock())
            + Self::mem_used_impl(&self.shard_metadata_cache.lock())
            + Self::mem_used_impl(&self.asset_scheme_cache.lock())
            + Self::mem_used_impl(&self.asset_cache.lock())
            + Self::mem_used_impl(&self.action_data_cache.lock())
    }

    /// Returns underlying `JournalDB`.
    pub fn journal_db(&self) -> &JournalDB {
        &*self.db
    }

    fn get_cached<Item>(&self, addr: &Item::Address, cache: &Mutex<GlobalCache<Item>>) -> Option<Option<Item>>
    where
        Item: CacheableItem, {
        let mut cache = cache.lock();
        // Check if the account can be returned from cache by matching current block parent hash against canonical
        // state and filtering out account modified in later blocks.
        if !cache.is_allowed(addr, &self.parent_hash) {
            return None
        }
        cache.get_mut(addr).cloned()
    }
}

impl Backend for StateDB {
    fn as_hashdb(&self) -> &HashDB {
        self.db.as_hashdb()
    }

    fn as_hashdb_mut(&mut self) -> &mut HashDB {
        self.db.as_hashdb_mut()
    }
}

impl TopBackend for StateDB {
    fn add_to_account_cache(&mut self, addr: Address, data: Option<Account>, modified: bool) {
        self.account_cache_buffer.push(addr, data, modified);
    }

    fn add_to_regular_account_cache(
        &mut self,
        address: RegularAccountAddress,
        data: Option<RegularAccount>,
        modified: bool,
    ) {
        self.regular_account_cache_buffer.push(address, data, modified);
    }

    fn add_to_metadata_cache(&mut self, address: MetadataAddress, item: Option<Metadata>, modified: bool) {
        self.metadata_cache_buffer.push(address, item, modified);
    }

    fn add_to_shard_cache(&mut self, address: ShardAddress, item: Option<Shard>, modified: bool) {
        self.shard_cache_buffer.push(address, item, modified);
    }

    fn add_to_action_data_cache(&mut self, address: H256, item: Option<ActionData>, modified: bool) {
        self.action_data_cache_buffer.push(address, item, modified);
    }

    fn get_cached_account(&self, addr: &Address) -> Option<Option<Account>> {
        self.get_cached(addr, &self.account_cache)
    }

    fn get_cached_regular_account(&self, addr: &RegularAccountAddress) -> Option<Option<RegularAccount>> {
        self.get_cached(addr, &self.regular_account_cache)
    }

    fn get_cached_metadata(&self, addr: &MetadataAddress) -> Option<Option<Metadata>> {
        self.get_cached(addr, &self.metadata_cache)
    }

    fn get_cached_shard(&self, addr: &ShardAddress) -> Option<Option<Shard>> {
        self.get_cached(addr, &self.shard_cache)
    }

    fn get_cached_action_data(&self, key: &H256) -> Option<Option<ActionData>> {
        self.get_cached(key, &self.action_data_cache)
    }

    fn custom_handlers(&self) -> &[Arc<ActionHandler>] {
        &self.custom_handlers
    }
}

impl ShardBackend for StateDB {
    fn add_to_shard_metadata_cache(
        &mut self,
        address: ShardMetadataAddress,
        item: Option<ShardMetadata>,
        modified: bool,
    ) {
        self.shard_metadata_cache_buffer.push(address, item, modified);
    }

    fn add_to_world_cache(&mut self, address: WorldAddress, item: Option<World>, modified: bool) {
        self.world_cache_buffer.push(address, item, modified);
    }

    fn add_to_asset_scheme_cache(&mut self, addr: AssetSchemeAddress, item: Option<AssetScheme>, modified: bool) {
        self.asset_scheme_cache_buffer.push(addr, item, modified);
    }

    fn add_to_asset_cache(&mut self, addr: OwnedAssetAddress, item: Option<OwnedAsset>, modified: bool) {
        self.asset_cache_buffer.push(addr, item, modified);
    }

    fn get_cached_shard_metadata(&self, addr: &ShardMetadataAddress) -> Option<Option<ShardMetadata>> {
        self.get_cached(addr, &self.shard_metadata_cache)
    }

    fn get_cached_world(&self, hash: &WorldAddress) -> Option<Option<World>> {
        self.get_cached(hash, &self.world_cache)
    }

    fn get_cached_asset_scheme(&self, hash: &AssetSchemeAddress) -> Option<Option<AssetScheme>> {
        self.get_cached(hash, &self.asset_scheme_cache)
    }

    fn get_cached_asset(&self, hash: &OwnedAssetAddress) -> Option<Option<OwnedAsset>> {
        self.get_cached(hash, &self.asset_cache)
    }
}

#[cfg(test)]
mod tests {
    use primitives::{H160, U256};

    use super::super::super::tests::helpers::get_temp_state_db;
    use super::*;

    #[test]
    fn account_cache() {
        let state_db = get_temp_state_db();
        let root_parent = H256::random();
        let address = Address::random();
        let h0 = H256::random();
        let mut batch = DBTransaction::new();

        let mut s = state_db.clone_canon(&root_parent);
        s.add_to_account_cache(address, Some(Account::new(2.into(), 0.into())), false);
        assert!(s.get_cached_account(&address).is_none());
        assert!(s.commit_hash.is_none());
        assert!(s.commit_number.is_none());
        assert!(batch.ops.len() == 0);

        s.journal_under(&mut batch, 0, &h0).unwrap();
        assert!(s.get_cached_account(&address).is_none());
        assert_eq!(s.commit_hash.unwrap(), h0);
        assert_eq!(s.commit_number.unwrap(), 0u64);
        assert!(batch.ops.len() > 0);

        s.sync_cache(&[], &[], true);
        assert!(s.get_cached_account(&address).is_none());

        let s = state_db.clone_canon(&h0);
        assert!(s.get_cached_account(&address).is_some());
    }

    #[test]
    fn state_db_smoke() {
        let state_db = get_temp_state_db();
        let root_parent = H256::random();
        let address = Address::random();
        let h0 = H256::random();
        let h1a = H256::random();
        let h1b = H256::random();
        let h2a = H256::random();
        let h2b = H256::random();
        let h3a = H256::random();
        let h3b = H256::random();
        let mut batch = DBTransaction::new();

        // blocks  [ 3a(c) 2a(c) 2b 1b 1a(c) 0 ]
        // balance [ 5     5     4  3  2     2 ]
        let mut s = state_db.clone_canon(&root_parent);
        s.add_to_account_cache(address, Some(Account::new(2.into(), 0.into())), false);
        s.journal_under(&mut batch, 0, &h0).unwrap();
        s.sync_cache(&[], &[], true);

        let mut s = state_db.clone_canon(&h0);
        s.journal_under(&mut batch, 1, &h1a).unwrap();
        s.sync_cache(&[], &[], true);

        let mut s = state_db.clone_canon(&h0);
        s.add_to_account_cache(address, Some(Account::new(3.into(), 0.into())), true);
        s.journal_under(&mut batch, 1, &h1b).unwrap();
        s.sync_cache(&[], &[], false);

        let mut s = state_db.clone_canon(&h1b);
        s.add_to_account_cache(address, Some(Account::new(4.into(), 0.into())), true);
        s.journal_under(&mut batch, 2, &h2b).unwrap();
        s.sync_cache(&[], &[], false);

        let mut s = state_db.clone_canon(&h1a);
        s.add_to_account_cache(address, Some(Account::new(5.into(), 0.into())), true);
        s.journal_under(&mut batch, 2, &h2a).unwrap();
        s.sync_cache(&[], &[], true);

        let mut s = state_db.clone_canon(&h2a);
        s.journal_under(&mut batch, 3, &h3a).unwrap();
        s.sync_cache(&[], &[], true);

        let s = state_db.clone_canon(&h3a);
        assert_eq!(s.get_cached_account(&address).unwrap().unwrap().balance(), &U256::from(5));

        let s = state_db.clone_canon(&h1a);
        assert!(s.get_cached_account(&address).is_none());

        let s = state_db.clone_canon(&h2b);
        assert!(s.get_cached_account(&address).is_none());

        let s = state_db.clone_canon(&h1b);
        assert!(s.get_cached_account(&address).is_none());

        // reorg to 3b
        // blocks  [ 3b(c) 3a 2a 2b(c) 1b 1a 0 ]
        let mut s = state_db.clone_canon(&h2b);
        s.journal_under(&mut batch, 3, &h3b).unwrap();
        s.sync_cache(&[h1b, h2b, h3b], &[h1a, h2a, h3a], true);
        let s = state_db.clone_canon(&h3a);
        assert!(s.get_cached_account(&address).is_none());
    }

    #[test]
    fn asset_scheme_cache() {
        let state_db = get_temp_state_db();
        let root_parent = H256::random();
        let h0 = H256::random();
        let mut batch = DBTransaction::new();
        let shard_id = 0;
        let world_id = 0;

        let amount = 1234;
        let registrar = Some(Address::random());
        let asset_scheme = AssetScheme::new("A metadata for test asset_scheme".to_string(), amount, registrar);
        let asset_scheme_address = AssetSchemeAddress::new(h0, shard_id, world_id);

        let mut s = state_db.clone_canon(&root_parent);

        s.add_to_asset_scheme_cache(asset_scheme_address.clone(), Some(asset_scheme), false);

        assert!(s.get_cached_asset_scheme(&asset_scheme_address).is_none());
        assert!(s.commit_hash.is_none());
        assert!(s.commit_number.is_none());
        assert_eq!(0, batch.ops.len());

        s.journal_under(&mut batch, 0, &h0).unwrap();
        assert!(s.get_cached_asset_scheme(&asset_scheme_address).is_none());
        assert_eq!(s.commit_hash.unwrap(), h0);
        assert_eq!(s.commit_number.unwrap(), 0u64);
        assert!(batch.ops.len() > 0);

        s.sync_cache(&[], &[], true);
        assert!(s.get_cached_asset_scheme(&asset_scheme_address).is_none());

        let s = state_db.clone_canon(&h0);
        let asset_scheme = s.get_cached_asset_scheme(&asset_scheme_address);
        assert!(asset_scheme.is_some());

        let asset_scheme = asset_scheme.unwrap();
        assert!(asset_scheme.is_some());

        let asset_scheme = asset_scheme.unwrap();

        assert!(asset_scheme.is_permissioned());
        assert_eq!(&amount, asset_scheme.amount());
        assert_eq!(&registrar, asset_scheme.registrar());
    }

    #[test]
    fn asset_cache() {
        let state_db = get_temp_state_db();
        let root_parent = H256::random();
        let mut batch = DBTransaction::new();

        let parcel_hash = H256::random();
        let asset_scheme_address = H256::random();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let amount = 1000;
        let shard_id = 0;
        let asset = OwnedAsset::new(asset_scheme_address, lock_script_hash, parameters, amount);
        let asset_address = OwnedAssetAddress::new(parcel_hash, 0, shard_id);

        let mut s = state_db.clone_canon(&root_parent);

        s.add_to_asset_cache(asset_address.clone(), Some(asset), false);

        assert!(s.get_cached_asset(&asset_address).is_none());
        assert!(s.commit_hash.is_none());
        assert!(s.commit_number.is_none());
        assert_eq!(0, batch.ops.len());

        s.journal_under(&mut batch, 0, &asset_address.clone().into()).unwrap();
        assert!(s.get_cached_asset(&asset_address).is_none());
        assert_eq!(s.commit_hash.unwrap(), asset_address.clone().into());
        assert_eq!(s.commit_number.unwrap(), 0u64);
        assert!(batch.ops.len() > 0);

        s.sync_cache(&[], &[], true);
        assert!(s.get_cached_asset(&asset_address).is_none());

        let s = state_db.clone_canon(&asset_address.clone().into());
        assert!(s.get_cached_asset(&asset_address).is_some());
        let asset = s.get_cached_asset(&asset_address).unwrap();
        assert!(asset.is_some());
        let asset = asset.unwrap();
        assert_eq!(&amount, asset.amount());
    }
}
