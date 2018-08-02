// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! State database abstraction. For more info, see the doc for `StateDB`

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use ckey::Address;
use ctypes::BlockNumber;
use hashdb::HashDB;
use journaldb::{self, Algorithm, JournalDB};
use kvdb::DBTransaction;
use kvdb_memorydb;
use lru_cache::LruCache;
use parking_lot::Mutex;
use primitives::{Bytes, H256};
use util_error::UtilError;

use super::{
    Account, ActionHandler, Asset, AssetAddress, AssetScheme, AssetSchemeAddress, Backend, CacheableItem, Metadata,
    MetadataAddress, RegularAccount, RegularAccountAddress, Shard, ShardAddress, ShardBackend, TopBackend, World,
    WorldAddress,
};

const STATE_CACHE_BLOCKS: usize = 12;

// The percentage of supplied cache size to go to accounts.
const ACCOUNT_CACHE_RATIO: usize = 35;
const REGULAR_ACCOUNT_CACHE_RATIO: usize = 5;
const METADATA_CACHE_RATIO: usize = 1;
const SHARD_CACHE_RATIO: usize = 7;
const WORLD_CACHE_RATIO: usize = 1;
const ASSET_SCHEME_CACHE_RATIO: usize = 10;
const ASSET_CACHE_RATIO: usize = 40;
const ACTION_DATA_CACHE_RATIO: usize = 1;

/// Shared canonical state cache.
struct Cache<Item>
where
    Item: CacheableItem, {
    /// `None` indicates that item is known to be missing.
    // When changing the type of the values here, be sure to update `mem_used` and
    // `new`.
    cache: LruCache<Item::Address, Option<Item>>,
    /// Information on the modifications in recently committed blocks; specifically which addresses
    /// changed in which block. Ordered by block number.
    modifications: VecDeque<BlockChanges<Item>>,
}

/// Buffered cache item.
struct CacheQueueItem<Item>
where
    Item: CacheableItem, {
    address: Item::Address,
    /// Item or `None` if item does not exist.
    item: Option<Item>,
    /// Indicates that the item was modified before being added to the cache.
    modified: bool,
}

#[derive(Debug)]
/// Accumulates a list of cacheable item changed in a block.
struct BlockChanges<Item>
where
    Item: CacheableItem, {
    /// Block number.
    number: BlockNumber,
    /// Block hash.
    hash: H256,
    /// Parent block hash.
    parent: H256,
    /// A set of modified addresses.
    modified_addresses: HashSet<Item::Address>,
    /// Block is part of the canonical chain.
    is_canon: bool,
}

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
    account_cache: Arc<Mutex<Cache<Account>>>,
    regular_account_cache: Arc<Mutex<Cache<RegularAccount>>>,
    metadata_cache: Arc<Mutex<Cache<Metadata>>>,
    shard_cache: Arc<Mutex<Cache<Shard>>>,
    world_cache: Arc<Mutex<Cache<World>>>,
    asset_scheme_cache: Arc<Mutex<Cache<AssetScheme>>>,
    asset_cache: Arc<Mutex<Cache<Asset>>>,
    action_data_cache: Arc<Mutex<Cache<Bytes>>>,

    /// Local dirty cache.
    local_account_cache: Vec<CacheQueueItem<Account>>,
    local_regular_account_cache: Vec<CacheQueueItem<RegularAccount>>,
    local_metadata_cache: Vec<CacheQueueItem<Metadata>>,
    local_shard_cache: Vec<CacheQueueItem<Shard>>,
    local_world_cache: Vec<CacheQueueItem<World>>,
    local_asset_scheme_cache: Vec<CacheQueueItem<AssetScheme>>,
    local_asset_cache: Vec<CacheQueueItem<Asset>>,
    local_action_data_cache: Vec<CacheQueueItem<Bytes>>,
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

        let world_cache_size = cache_size * WORLD_CACHE_RATIO / 100;
        let world_cache_items = world_cache_size / ::std::mem::size_of::<Option<World>>();

        let asset_scheme_cache_size = cache_size * ASSET_SCHEME_CACHE_RATIO / 100;
        let asset_scheme_cache_items = asset_scheme_cache_size / ::std::mem::size_of::<Option<AssetScheme>>();

        let asset_cache_size = cache_size * ASSET_CACHE_RATIO / 100;
        let asset_cache_items = asset_cache_size / ::std::mem::size_of::<Option<Asset>>();

        let action_data_cache_size = cache_size * ACTION_DATA_CACHE_RATIO / 100;
        let action_data_cache_items = action_data_cache_size / ::std::mem::size_of::<Option<Bytes>>();

        StateDB {
            db,
            account_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(account_cache_items),
                modifications: VecDeque::new(),
            })),
            regular_account_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(regular_account_cache_items),
                modifications: VecDeque::new(),
            })),
            metadata_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(metadata_cache_items),
                modifications: VecDeque::new(),
            })),
            shard_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(shard_cache_items),
                modifications: VecDeque::new(),
            })),
            world_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(world_cache_items),
                modifications: VecDeque::new(),
            })),
            asset_scheme_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(asset_scheme_cache_items),
                modifications: VecDeque::new(),
            })),
            asset_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(asset_cache_items),
                modifications: VecDeque::new(),
            })),
            action_data_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(action_data_cache_items),
                modifications: VecDeque::new(),
            })),

            local_account_cache: Vec::new(),
            local_regular_account_cache: Vec::new(),
            local_metadata_cache: Vec::new(),
            local_shard_cache: Vec::new(),
            local_world_cache: Vec::new(),
            local_asset_scheme_cache: Vec::new(),
            local_asset_cache: Vec::new(),
            local_action_data_cache: Vec::new(),
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

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut self.account_cache,
            &mut self.local_account_cache,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut self.regular_account_cache,
            &mut self.local_regular_account_cache,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut self.shard_cache,
            &mut self.local_shard_cache,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut self.world_cache,
            &mut self.local_world_cache,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut self.asset_scheme_cache,
            &mut self.local_asset_scheme_cache,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut self.asset_cache,
            &mut self.local_asset_cache,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );

        Self::sync_cache_impl(
            enacted,
            retracted,
            is_best,
            &mut self.action_data_cache,
            &mut self.local_action_data_cache,
            &self.parent_hash,
            &self.commit_hash,
            &self.commit_number,
        );
    }

    fn sync_cache_impl<Item>(
        enacted: &[H256],
        retracted: &[H256],
        is_best: bool,
        cache: &Mutex<Cache<Item>>,
        local_cache: &mut Vec<CacheQueueItem<Item>>,
        parent_hash: &Option<H256>,
        commit_hash: &Option<H256>,
        commit_number: &Option<BlockNumber>,
    ) where
        Item: CacheableItem, {
        let mut cache = cache.lock();
        let cache = &mut *cache;

        // Purge changes from re-enacted and retracted blocks.
        // Filter out committing block if any.
        let mut clear = false;
        for block in enacted.iter().filter(|h| commit_hash.as_ref().map_or(true, |p| *h != p)) {
            clear = clear || {
                if let Some(ref mut m) = cache.modifications.iter_mut().find(|m| &m.hash == block) {
                    ctrace!(STATE_DB, "Reverting enacted block {:?}", block);
                    m.is_canon = true;
                    for a in &m.modified_addresses {
                        ctrace!(STATE_DB, "Reverting enacted address {:?}", a);
                        cache.cache.remove(a);
                    }
                    false
                } else {
                    true
                }
            };
        }

        for block in retracted {
            clear = clear || {
                if let Some(ref mut m) = cache.modifications.iter_mut().find(|m| &m.hash == block) {
                    ctrace!(STATE_DB, "Retracting block {:?}", block);
                    m.is_canon = false;
                    for a in &m.modified_addresses {
                        ctrace!(STATE_DB, "Retracted address {:?}", a);
                        cache.cache.remove(a);
                    }
                    false
                } else {
                    true
                }
            };
        }
        if clear {
            // We don't know anything about the block; clear everything
            ctrace!(STATE_DB, "Wiping cache");
            cache.cache.clear();
            cache.modifications.clear();
        }

        // Propagate cache only if committing on top of the latest canonical state
        // blocks are ordered by number and only one block with a given number is marked as canonical
        // (contributed to canonical state cache)
        if let (Some(number), Some(hash), Some(parent)) = (commit_number, commit_hash, parent_hash) {
            if cache.modifications.len() == STATE_CACHE_BLOCKS {
                cache.modifications.pop_back();
            }
            let mut modified_addresses = HashSet::new();
            ctrace!(STATE_DB, "committing {} cache entries", local_cache.len());
            for local_item in local_cache.drain(..) {
                if local_item.modified {
                    modified_addresses.insert(local_item.address.clone());
                }
                if is_best {
                    let acc = local_item.item;
                    if let Some(Some(existing)) = cache.cache.get_mut(&local_item.address) {
                        if let Some(new) = acc {
                            if local_item.modified {
                                *existing = new;
                            }
                            continue
                        }
                    }
                    cache.cache.insert(local_item.address, acc);
                }
            }

            // Save modified addresses. These are ordered by the block number.
            let block_changes = BlockChanges {
                modified_addresses,
                number: *number,
                hash: hash.clone(),
                is_canon: is_best,
                parent: parent.clone(),
            };
            let insert_at = cache.modifications.iter().enumerate().find(|&(_, m)| m.number < *number).map(|(i, _)| i);
            ctrace!(STATE_DB, "inserting modifications at {:?}", insert_at);
            if let Some(insert_at) = insert_at {
                cache.modifications.insert(insert_at, block_changes);
            } else {
                cache.modifications.push_back(block_changes);
            }
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

    /// Clone the database for a canonical state.
    pub fn clone_canon(&self, parent: &H256) -> StateDB {
        StateDB {
            db: self.db.boxed_clone(),
            account_cache: self.account_cache.clone(),
            regular_account_cache: self.regular_account_cache.clone(),
            metadata_cache: self.metadata_cache.clone(),
            shard_cache: self.shard_cache.clone(),
            world_cache: self.world_cache.clone(),
            asset_scheme_cache: self.asset_scheme_cache.clone(),
            asset_cache: self.asset_cache.clone(),
            action_data_cache: self.action_data_cache.clone(),

            local_account_cache: Vec::new(),
            local_regular_account_cache: Vec::new(),
            local_metadata_cache: Vec::new(),
            local_shard_cache: Vec::new(),
            local_world_cache: Vec::new(),
            local_asset_scheme_cache: Vec::new(),
            local_asset_cache: Vec::new(),
            local_action_data_cache: Vec::new(),

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

    fn mem_used_impl<Item>(cache: &Cache<Item>) -> usize
    where
        Item: CacheableItem, {
        let accounts = cache.cache.len();
        accounts * ::std::mem::size_of::<Option<Item>>()
    }

    /// Heap size used.
    pub fn mem_used(&self) -> usize {
        // TODO: account for LRU-cache overhead; this is a close approximation.
        self.db.mem_used()
            + Self::mem_used_impl(&self.account_cache.lock())
            + Self::mem_used_impl(&self.regular_account_cache.lock())
            + Self::mem_used_impl(&self.shard_cache.lock())
            + Self::mem_used_impl(&self.asset_scheme_cache.lock())
            + Self::mem_used_impl(&self.asset_cache.lock())
            + Self::mem_used_impl(&self.action_data_cache.lock())
    }

    /// Returns underlying `JournalDB`.
    pub fn journal_db(&self) -> &JournalDB {
        &*self.db
    }

    /// Check if the account can be returned from cache by matching current block parent hash against canonical
    /// state and filtering out account modified in later blocks.
    fn is_allowed<Item>(
        addr: &Item::Address,
        parent_hash: &Option<H256>,
        modifications: &VecDeque<BlockChanges<Item>>,
    ) -> bool
    where
        Item: CacheableItem, {
        let mut parent = match parent_hash {
            None => {
                ctrace!(STATE_DB, "Cache lookup skipped for {:?}: no parent hash", addr);
                return false
            }
            Some(parent) => parent,
        };
        if modifications.is_empty() {
            return true
        }
        // Ignore all accounts modified in later blocks
        // Modifications contains block ordered by the number
        // We search for our parent in that list first and then for
        // all its parent until we hit the canonical block,
        // checking against all the intermediate modifications.
        for m in modifications {
            if &m.hash == parent {
                if m.is_canon {
                    return true
                }
                parent = &m.parent;
            }
            if m.modified_addresses.contains(addr) {
                ctrace!(STATE_DB, "Cache lookup skipped for {:?}: modified in a later block", addr);
                return false
            }
        }
        ctrace!(STATE_DB, "Cache lookup skipped for {:?}: parent hash is unknown", addr);
        false
    }

    fn get_cached<Item>(&self, addr: &Item::Address, cache: &Mutex<Cache<Item>>) -> Option<Option<Item>>
    where
        Item: CacheableItem, {
        let mut cache = cache.lock();
        if !Self::is_allowed(addr, &self.parent_hash, &cache.modifications) {
            return None
        }
        cache.cache.get_mut(addr).cloned()
    }

    fn get_cached_with<Item, F, U>(&self, a: &Item::Address, f: F, cache: &Mutex<Cache<Item>>) -> Option<U>
    where
        Item: CacheableItem,
        F: FnOnce(Option<&mut Item>) -> U, {
        let mut cache = cache.lock();
        if !Self::is_allowed(a, &self.parent_hash, &cache.modifications) {
            return None
        }
        cache.cache.get_mut(a).map(|c| f(c.as_mut()))
    }
}

impl Clone for StateDB {
    fn clone(&self) -> Self {
        Self {
            db: self.db.boxed_clone(),
            account_cache: self.account_cache.clone(),
            regular_account_cache: self.regular_account_cache.clone(),
            metadata_cache: self.metadata_cache.clone(),
            shard_cache: self.shard_cache.clone(),
            world_cache: self.world_cache.clone(),
            asset_scheme_cache: self.asset_scheme_cache.clone(),
            asset_cache: self.asset_cache.clone(),
            action_data_cache: self.action_data_cache.clone(),

            local_account_cache: Vec::new(),
            local_regular_account_cache: Vec::new(),
            local_metadata_cache: Vec::new(),
            local_shard_cache: Vec::new(),
            local_world_cache: Vec::new(),
            local_asset_scheme_cache: Vec::new(),
            local_asset_cache: Vec::new(),
            local_action_data_cache: Vec::new(),

            parent_hash: None,
            commit_hash: None,
            commit_number: None,

            custom_handlers: self.custom_handlers.clone(),
        }
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
        self.local_account_cache.push(CacheQueueItem {
            address: addr,
            item: data,
            modified,
        })
    }

    fn add_to_regular_account_cache(
        &mut self,
        address: RegularAccountAddress,
        data: Option<RegularAccount>,
        modified: bool,
    ) {
        self.local_regular_account_cache.push(CacheQueueItem {
            address,
            item: data,
            modified,
        })
    }

    fn add_to_metadata_cache(&mut self, address: MetadataAddress, item: Option<Metadata>, modified: bool) {
        self.local_metadata_cache.push(CacheQueueItem {
            address,
            item,
            modified,
        })
    }

    fn add_to_shard_cache(&mut self, address: ShardAddress, item: Option<Shard>, modified: bool) {
        self.local_shard_cache.push(CacheQueueItem {
            address,
            item,
            modified,
        })
    }

    fn add_to_action_data_cache(&mut self, address: H256, item: Option<Bytes>, modified: bool) {
        self.local_action_data_cache.push(CacheQueueItem {
            address,
            item,
            modified,
        })
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

    fn get_cached_action_data(&self, key: &H256) -> Option<Option<Bytes>> {
        self.get_cached(key, &self.action_data_cache)
    }

    fn get_cached_account_with<F, U>(&self, a: &Address, f: F) -> Option<U>
    where
        F: FnOnce(Option<&mut Account>) -> U, {
        self.get_cached_with(a, f, &self.account_cache)
    }

    fn get_cached_regular_account_with<F, U>(&self, a: &RegularAccountAddress, f: F) -> Option<U>
    where
        F: FnOnce(Option<&mut RegularAccount>) -> U, {
        self.get_cached_with(a, f, &self.regular_account_cache)
    }

    fn custom_handlers(&self) -> &[Arc<ActionHandler>] {
        &self.custom_handlers
    }
}

impl ShardBackend for StateDB {
    fn add_to_world_cache(&mut self, address: WorldAddress, item: Option<World>, modified: bool) {
        self.local_world_cache.push(CacheQueueItem {
            address,
            item,
            modified,
        })
    }

    fn add_to_asset_scheme_cache(&mut self, addr: AssetSchemeAddress, item: Option<AssetScheme>, modified: bool) {
        self.local_asset_scheme_cache.push(CacheQueueItem {
            address: addr,
            item,
            modified,
        })
    }

    fn add_to_asset_cache(&mut self, addr: AssetAddress, item: Option<Asset>, modified: bool) {
        self.local_asset_cache.push(CacheQueueItem {
            address: addr,
            item,
            modified,
        })
    }

    fn get_cached_world(&self, hash: &WorldAddress) -> Option<Option<World>> {
        self.get_cached(hash, &self.world_cache)
    }

    fn get_cached_asset_scheme(&self, hash: &AssetSchemeAddress) -> Option<Option<AssetScheme>> {
        self.get_cached(hash, &self.asset_scheme_cache)
    }

    fn get_cached_asset(&self, hash: &AssetAddress) -> Option<Option<Asset>> {
        self.get_cached(hash, &self.asset_cache)
    }
}

#[cfg(test)]
mod tests {
    use primitives::U256;

    use super::super::tests::helpers::get_temp_state_db;
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
        s.sync_cache(&[h1b.clone(), h2b.clone(), h3b.clone()], &[h1a.clone(), h2a.clone(), h3a.clone()], true);
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

        let amount = 1234;
        let registrar = Some(Address::random());
        let asset_scheme = AssetScheme::new("A metadata for test asset_scheme".to_string(), amount, registrar);
        let asset_scheme_address = AssetSchemeAddress::new(h0, shard_id);

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
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let amount = 1000;
        let shard_id = 0;
        let asset = Asset::new(asset_scheme_address, lock_script_hash, parameters, amount);
        let asset_address = AssetAddress::new(parcel_hash, 0, shard_id);

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
