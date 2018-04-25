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

use ctypes::{Address, H256};
use hashdb::HashDB;
use journaldb::JournalDB;
use kvdb::DBTransaction;
use lru_cache::LruCache;
use parking_lot::Mutex;
use util_error::UtilError;

use super::state::{self, Account, CacheableItem};
use super::types::BlockNumber;

const STATE_CACHE_BLOCKS: usize = 12;

// The percentage of supplied cache size to go to accounts.
const ACCOUNT_CACHE_RATIO: usize = 90;

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
    /// Local dirty cache.
    local_account_cache: Vec<CacheQueueItem<Account>>,
    /// Hash of the block on top of which this instance was created or
    /// `None` if cache is disabled
    parent_hash: Option<H256>,
    /// Hash of the committing block or `None` if not committed yet.
    commit_hash: Option<H256>,
    /// Number of the committing block or `None` if not committed yet.
    commit_number: Option<BlockNumber>,
}

impl StateDB {
    /// Create a new instance wrapping `JournalDB` and the maximum allowed size
    /// of the LRU cache in bytes. Actual used memory may (read: will) be higher due to bookkeeping.
    // TODO: make the cache size actually accurate by moving the account storage cache
    // into the `AccountCache` structure as its own `LruCache<(Address, H256), H256>`.
    pub fn new(db: Box<JournalDB>, cache_size: usize) -> StateDB {
        let acc_cache_size = cache_size * ACCOUNT_CACHE_RATIO / 100;
        let cache_items = acc_cache_size / ::std::mem::size_of::<Option<Account>>();

        StateDB {
            db,
            account_cache: Arc::new(Mutex::new(Cache {
                cache: LruCache::new(cache_items),
                modifications: VecDeque::new(),
            })),
            local_account_cache: Vec::new(),
            parent_hash: None,
            commit_hash: None,
            commit_number: None,
        }
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
        trace!(target: "state_db",
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
        )
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
                    trace!(target: "state_db", "Reverting enacted block {:?}", block);
                    m.is_canon = true;
                    for a in &m.modified_addresses {
                        trace!(target: "state_db", "Reverting enacted address {:?}", a);
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
                    trace!(target: "state_db", "Retracting block {:?}", block);
                    m.is_canon = false;
                    for a in &m.modified_addresses {
                        trace!(target: "state_db", "Retracted address {:?}", a);
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
            trace!(target: "state_db", "Wiping cache");
            cache.cache.clear();
            cache.modifications.clear();
        }

        // Propagate cache only if committing on top of the latest canonical state
        // blocks are ordered by number and only one block with a given number is marked as canonical
        // (contributed to canonical state cache)
        if let (&Some(ref number), &Some(ref hash), &Some(ref parent)) = (commit_number, commit_hash, parent_hash) {
            if cache.modifications.len() == STATE_CACHE_BLOCKS {
                cache.modifications.pop_back();
            }
            let mut modified_addresses = HashSet::new();
            trace!(target: "state_db", "committing {} cache entries", local_cache.len());
            for local_item in local_cache.drain(..) {
                if local_item.modified {
                    modified_addresses.insert(local_item.address.clone());
                }
                if is_best {
                    let acc = local_item.item;
                    if let Some(&mut Some(ref mut existing)) = cache.cache.get_mut(&local_item.address) {
                        if let Some(new) = acc {
                            if local_item.modified {
                                existing.overwrite_with(new);
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
            trace!(target: "state_db", "inserting modifications at {:?}", insert_at);
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

    /// Clone the database.
    pub fn boxed_clone(&self) -> StateDB {
        StateDB {
            db: self.db.boxed_clone(),
            account_cache: self.account_cache.clone(),
            local_account_cache: Vec::new(),
            parent_hash: None,
            commit_hash: None,
            commit_number: None,
        }
    }

    /// Clone the database for a canonical state.
    pub fn boxed_clone_canon(&self, parent: &H256) -> StateDB {
        StateDB {
            db: self.db.boxed_clone(),
            account_cache: self.account_cache.clone(),
            local_account_cache: Vec::new(),
            parent_hash: Some(parent.clone()),
            commit_hash: None,
            commit_number: None,
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
        self.db.mem_used() + Self::mem_used_impl(&self.account_cache.lock())
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
        let mut parent = match *parent_hash {
            None => {
                trace!(target: "state_db", "Cache lookup skipped for {:?}: no parent hash", addr);
                return false
            }
            Some(ref parent) => parent,
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
                trace!(target: "state_db", "Cache lookup skipped for {:?}: modified in a later block", addr);
                return false
            }
        }
        trace!(target: "state_db", "Cache lookup skipped for {:?}: parent hash is unknown", addr);
        false
    }

    fn get_cached<Item>(&self, addr: &Item::Address, cache: &Mutex<Cache<Item>>) -> Option<Option<Item>>
    where
        Item: CacheableItem, {
        let mut cache = cache.lock();
        if !Self::is_allowed(addr, &self.parent_hash, &cache.modifications) {
            return None
        }
        cache.cache.get_mut(addr).map(|a| a.as_ref().map(|a| a.clone()))
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

impl state::Backend for StateDB {
    fn as_hashdb(&self) -> &HashDB {
        self.db.as_hashdb()
    }

    fn as_hashdb_mut(&mut self) -> &mut HashDB {
        self.db.as_hashdb_mut()
    }

    fn add_to_account_cache(&mut self, addr: Address, data: Option<Account>, modified: bool) {
        self.local_account_cache.push(CacheQueueItem {
            address: addr,
            item: data,
            modified,
        })
    }

    fn get_cached_account(&self, addr: &Address) -> Option<Option<Account>> {
        self.get_cached(addr, &self.account_cache)
    }

    fn get_cached_account_with<F, U>(&self, a: &Address, f: F) -> Option<U>
    where
        F: FnOnce(Option<&mut Account>) -> U, {
        self.get_cached_with(a, f, &self.account_cache)
    }
}

#[cfg(test)]
mod tests {
    use clogger::init_log;
    use ctypes::{Address, H256, U256};
    use kvdb::DBTransaction;

    use super::super::tests::helpers::get_temp_state_db;
    use super::state::{Account, Backend};

    #[test]
    fn account_cache() {
        let state_db = get_temp_state_db();
        let root_parent = H256::random();
        let address = Address::random();
        let h0 = H256::random();
        let mut batch = DBTransaction::new();

        let mut s = state_db.boxed_clone_canon(&root_parent);
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

        let s = state_db.boxed_clone_canon(&h0);
        assert!(s.get_cached_account(&address).is_some());
    }

    #[test]
    fn state_db_smoke() {
        init_log();

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
        let mut s = state_db.boxed_clone_canon(&root_parent);
        s.add_to_account_cache(address, Some(Account::new(2.into(), 0.into())), false);
        s.journal_under(&mut batch, 0, &h0).unwrap();
        s.sync_cache(&[], &[], true);

        let mut s = state_db.boxed_clone_canon(&h0);
        s.journal_under(&mut batch, 1, &h1a).unwrap();
        s.sync_cache(&[], &[], true);

        let mut s = state_db.boxed_clone_canon(&h0);
        s.add_to_account_cache(address, Some(Account::new(3.into(), 0.into())), true);
        s.journal_under(&mut batch, 1, &h1b).unwrap();
        s.sync_cache(&[], &[], false);

        let mut s = state_db.boxed_clone_canon(&h1b);
        s.add_to_account_cache(address, Some(Account::new(4.into(), 0.into())), true);
        s.journal_under(&mut batch, 2, &h2b).unwrap();
        s.sync_cache(&[], &[], false);

        let mut s = state_db.boxed_clone_canon(&h1a);
        s.add_to_account_cache(address, Some(Account::new(5.into(), 0.into())), true);
        s.journal_under(&mut batch, 2, &h2a).unwrap();
        s.sync_cache(&[], &[], true);

        let mut s = state_db.boxed_clone_canon(&h2a);
        s.journal_under(&mut batch, 3, &h3a).unwrap();
        s.sync_cache(&[], &[], true);

        let s = state_db.boxed_clone_canon(&h3a);
        assert_eq!(s.get_cached_account(&address).unwrap().unwrap().balance(), &U256::from(5));

        let s = state_db.boxed_clone_canon(&h1a);
        assert!(s.get_cached_account(&address).is_none());

        let s = state_db.boxed_clone_canon(&h2b);
        assert!(s.get_cached_account(&address).is_none());

        let s = state_db.boxed_clone_canon(&h1b);
        assert!(s.get_cached_account(&address).is_none());

        // reorg to 3b
        // blocks  [ 3b(c) 3a 2a 2b(c) 1b 1a 0 ]
        let mut s = state_db.boxed_clone_canon(&h2b);
        s.journal_under(&mut batch, 3, &h3b).unwrap();
        s.sync_cache(&[h1b.clone(), h2b.clone(), h3b.clone()], &[h1a.clone(), h2a.clone(), h3a.clone()], true);
        let s = state_db.boxed_clone_canon(&h3a);
        assert!(s.get_cached_account(&address).is_none());
    }
}
