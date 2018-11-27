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

use std::cell::{RefCell, RefMut};
use std::collections::hash_map::Entry as HashMapEntry;
use std::collections::HashMap;
use std::convert::AsRef;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::vec::Vec;

use cmerkle::{self, Result as TrieResult, Trie, TrieDB, TrieMut};

use super::CacheableItem;

static TOUCHED_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;
fn touched_count() -> usize {
    TOUCHED_COUNT.fetch_add(1, Ordering::SeqCst)
}

#[derive(Clone, Debug)]
struct Entry<Item>
where
    Item: CacheableItem, {
    item: Option<Item>,
    is_dirty: bool,
    /// Touched time
    touched: usize,
}

// Account cache item. Contains account data and
// modification state
impl<Item> Entry<Item>
where
    Item: CacheableItem,
{
    // Create a new account entry and mark it as dirty.
    fn new_dirty(item: Option<Item>) -> Self {
        Self {
            item,
            is_dirty: true,
            touched: touched_count(),
        }
    }

    // Create a new account entry and mark it as clean.
    fn new_clean(item: Option<Item>) -> Self {
        Self::new_clean_with_touched(item, touched_count())
    }

    // Create a new account entry and mark it as clean.
    fn new_clean_with_touched(item: Option<Item>, touched: usize) -> Self {
        Self {
            item,
            is_dirty: false,
            touched,
        }
    }
}

pub struct WriteBack<Item>
where
    Item: CacheableItem, {
    cache: RefCell<HashMap<Item::Address, Entry<Item>>>,
    // The original item is preserved in
    checkpoints: RefCell<Vec<HashMap<Item::Address, Option<Entry<Item>>>>>,
}

impl<Item> WriteBack<Item>
where
    Item: CacheableItem,
{
    pub fn new() -> Self {
        Self {
            cache: Default::default(),
            checkpoints: Default::default(),
        }
    }

    pub fn new_with_iter(items: impl Iterator<Item = (Item::Address, Item)>) -> Self {
        let cache = Self::new();
        // lru_cache::iter() returns the least-recently-used to the most-recently-used
        for (touched, (addr, item)) in items.enumerate() {
            cache.insert(&addr, Entry::new_clean_with_touched(Some(item), touched))
        }
        debug_assert!(
            cache.len() <= TOUCHED_COUNT.load(Ordering::SeqCst),
            "cache.len():{} must be less than TOUCHED_COUNT:{}",
            cache.len(),
            TOUCHED_COUNT.load(Ordering::SeqCst)
        );
        cache
    }

    pub fn checkpoint(&mut self) {
        self.checkpoints.get_mut().push(HashMap::new());
    }

    pub fn discard_checkpoint(&mut self) {
        // merge with previous checkpoint
        let last = self.checkpoints.get_mut().pop();
        if let Some(mut checkpoint) = last {
            if let Some(ref mut prev) = self.checkpoints.get_mut().last_mut() {
                if prev.is_empty() {
                    **prev = checkpoint;
                } else {
                    for (k, v) in checkpoint.drain() {
                        prev.entry(k).or_insert(v);
                    }
                }
            }
        }
    }

    pub fn revert_to_checkpoint(&mut self) {
        if let Some(mut checkpoint) = self.checkpoints.get_mut().pop() {
            for (k, v) in checkpoint.drain() {
                match v {
                    Some(v) => match self.cache.get_mut().entry(k) {
                        HashMapEntry::Occupied(mut e) => {
                            *e.get_mut() = v;
                        }
                        HashMapEntry::Vacant(e) => {
                            e.insert(v);
                        }
                    },
                    None => {
                        if let HashMapEntry::Occupied(e) = self.cache.get_mut().entry(k) {
                            if e.get().is_dirty {
                                e.remove();
                            }
                        }
                    }
                }
            }
        }
    }

    fn insert(&self, address: &Item::Address, item: Entry<Item>) {
        // Dirty item which is not in the cache means this is a new item.
        // It goes directly into the checkpoint as there's nothing to revert to.
        //
        // In all other cases item is read as clean first, and after that made
        // dirty in and added to the checkpoint with `note_cache`.
        let is_dirty = item.is_dirty;
        let old_value = self.cache.borrow_mut().insert(*address, item);
        if !is_dirty {
            return
        }
        if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(*address).or_insert(old_value);
        }
    }

    pub fn remove(&self, address: &Item::Address) {
        self.insert(address, Entry::<Item>::new_dirty(None))
    }

    fn note(&self, address: &Item::Address) {
        if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(*address).or_insert_with(|| self.cache.borrow().get(address).cloned());
        }
    }

    pub fn commit<'db>(&mut self, trie: &mut Box<TrieMut + 'db>) -> TrieResult<()> {
        let mut cache = self.cache.borrow_mut();
        for (address, ref mut a) in cache.iter_mut().filter(|&(_, ref a)| a.is_dirty) {
            a.is_dirty = false;
            match &a.item {
                Some(item) => {
                    trie.insert(address.as_ref(), &item.rlp_bytes())?;
                }
                None => {
                    trie.remove(address.as_ref())?;
                }
            };
        }
        Ok(())
    }

    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    pub fn get(&self, a: &Item::Address, db: TrieDB) -> cmerkle::Result<Option<Item>> {
        // check local cache first
        if let Some(cached_item) = self.cache.borrow_mut().get_mut(a) {
            cached_item.touched = touched_count();
            return Ok(cached_item.item.clone())
        }

        // not found in the cache, get from the DB and insert into cache
        let maybe_item = db.get_with(a.as_ref(), ::rlp::decode::<Item>)?;
        self.insert(a, Entry::<Item>::new_clean(maybe_item.clone()));
        Ok(maybe_item)
    }

    /// Pull item `a` in our cache from the trie DB.
    /// If it doesn't exist, make item equal the evaluation of `default`.
    pub fn get_mut(&self, a: &Item::Address, db: TrieDB) -> cmerkle::Result<RefMut<Item>> {
        let contains_key = self.cache.borrow().contains_key(a);
        if !contains_key {
            let maybe_item = db.get_with(a.as_ref(), ::rlp::decode::<Item>)?;
            self.insert(a, Entry::<Item>::new_clean(maybe_item));
        }
        self.note(a);

        // at this point the entry is guaranteed to be in the cache.
        Ok(RefMut::map(self.cache.borrow_mut(), |c| {
            let entry = c.get_mut(a).expect("entry known to exist in the cache; qed");

            match &mut entry.item {
                Some(_) => {}
                slot @ None => *slot = Some(Item::default()),
            }

            // set the dirty flag after changing data.
            entry.is_dirty = true;
            entry.touched = touched_count();
            entry.item.as_mut().expect("Required item must always exist; qed")
        }))
    }

    pub fn items(&self) -> Vec<(usize, Item::Address, Option<Item>)> {
        let cache = self.cache.borrow();
        cache
            .iter()
            .map(|(addr, entry)| {
                if entry.is_dirty {
                    unreachable!("The cache must be committed before called items")
                } else {
                    (entry.touched, *addr, entry.item.clone())
                }
            })
            .collect()
    }

    fn len(&self) -> usize {
        let cache = self.cache.borrow();
        cache.len()
    }
}

impl<Item> fmt::Debug for WriteBack<Item>
where
    Item: CacheableItem,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.cache.borrow().fmt(f)
    }
}

impl<Item> Clone for WriteBack<Item>
where
    Item: CacheableItem,
{
    fn clone(&self) -> Self {
        assert_eq!(0, self.checkpoints.borrow().len());
        Self {
            cache: self.cache.clone(),
            checkpoints: RefCell::new(vec![]),
        }
    }
}
