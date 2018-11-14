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
use std::vec::Vec;

use cmerkle::{self, Result as TrieResult, Trie, TrieDB, TrieMut};

use super::CacheableItem;

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
/// Used to check if the item was modified in between commits and overall.
enum EntryState {
    /// Loaded from disk and never modified in this state object.
    CleanFresh,
    /// Modified and not committed to the trie yet.
    /// This is set if any of the data is changed.
    Dirty,
    /// Committed to the trie.
    Committed,
}

#[derive(Clone, Debug)]
struct Entry<Item>
where
    Item: CacheableItem, {
    item: Option<Item>,
    /// Entry state.
    state: EntryState,
}

// Account cache item. Contains account data and
// modification state
impl<Item> Entry<Item>
where
    Item: CacheableItem,
{
    fn is_dirty(&self) -> bool {
        self.state == EntryState::Dirty
    }

    // Create a new account entry and mark it as dirty.
    fn new_dirty(item: Option<Item>) -> Self {
        Self {
            item,
            state: EntryState::Dirty,
        }
    }

    // Create a new account entry and mark it as clean.
    fn new_clean(item: Option<Item>) -> Self {
        Self {
            item,
            state: EntryState::CleanFresh,
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
                            if e.get().is_dirty() {
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
        let is_dirty = item.is_dirty();
        let old_value = self.cache.borrow_mut().insert(address.clone(), item);
        if !is_dirty {
            return
        }
        if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(address.clone()).or_insert(old_value);
        }
    }

    pub fn remove(&self, address: &Item::Address) {
        self.insert(address, Entry::<Item>::new_dirty(None))
    }

    fn note(&self, address: &Item::Address) {
        if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(address.clone()).or_insert_with(|| self.cache.borrow().get(address).cloned());
        }
    }

    pub fn commit<'db>(&mut self, trie: &mut Box<TrieMut + 'db>) -> TrieResult<()> {
        let mut cache = self.cache.borrow_mut();
        for (address, ref mut a) in cache.iter_mut().filter(|&(_, ref a)| a.is_dirty()) {
            a.state = EntryState::Committed;
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
        if let Some(cached_item) = self.cache.borrow().get(a) {
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
            entry.state = EntryState::Dirty;
            entry.item.as_mut().expect("Required item must always exist; qed")
        }))
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
