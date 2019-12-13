// Copyright 2019 Kodebox, Inc.
// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of CodeChain.
//
// This is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Reference-counted memory-based `HashDB` implementation.
extern crate codechain_crypto;
extern crate plain_hasher;
extern crate primitives;
extern crate rlp;

use super::{DBValue, HashDB};
use codechain_crypto::{blake256, BLAKE_NULL_RLP};
use plain_hasher::PlainHasher;
use primitives::H256;
use rlp::NULL_RLP;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash;
use std::mem;

type H256FastMap<T> = HashMap<H256, T, hash::BuildHasherDefault<PlainHasher>>;

/// Reference-counted memory-based `HashDB` implementation.
///
/// Use `new()` to create a new database. Insert items with `insert()`, remove items
/// with `remove()`, check for existence with `contains()` and lookup a hash to derive
/// the data with `get()`. Clear with `clear()` and purge the portions of the data
/// that have no references with `purge()`.
///
/// # Example
/// ```rust
/// extern crate codechain_db as cdb;
/// use cdb::*;
///
/// let mut m = MemoryDB::new();
/// let d = "Hello world!".as_bytes();
///
/// let k = m.insert(d);
/// assert!(m.contains(&k));
/// assert_eq!(m.get(&k).unwrap(), d);
///
/// m.insert(d);
/// assert!(m.contains(&k));
///
/// m.remove(&k);
/// assert!(m.contains(&k));
///
/// m.remove(&k);
/// assert!(!m.contains(&k));
///
/// m.remove(&k);
/// assert!(!m.contains(&k));
///
/// m.insert(d);
/// assert!(!m.contains(&k));

/// m.insert(d);
/// assert!(m.contains(&k));
/// assert_eq!(m.get(&k).unwrap(), d);
///
/// m.remove(&k);
/// assert!(!m.contains(&k));
/// ```
#[derive(Default, Clone, PartialEq)]
pub struct MemoryDB {
    data: H256FastMap<(DBValue, i32)>,
}

impl MemoryDB {
    /// Create a new instance of the memory DB.
    pub fn new() -> MemoryDB {
        Default::default()
    }

    /// Clear all data from the database.
    ///
    /// # Examples
    /// ```rust
    /// extern crate codechain_db as cdb;
    /// use cdb::*;
    ///
    /// let mut m = MemoryDB::new();
    /// let hello_bytes = "Hello world!".as_bytes();
    /// let hash = m.insert(hello_bytes);
    /// assert!(m.contains(&hash));
    /// m.clear();
    /// assert!(!m.contains(&hash));
    /// ```
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Purge all zero-referenced data from the database.
    pub fn purge(&mut self) {
        self.data.retain(|_, &mut (_, rc)| rc != 0);
    }

    /// Return the internal map of hashes to data, clearing the current state.
    pub fn drain(&mut self) -> H256FastMap<(DBValue, i32)> {
        mem::replace(&mut self.data, H256FastMap::default())
    }

    /// Grab the raw information associated with a key. Returns None if the key
    /// doesn't exist.
    ///
    /// Even when Some is returned, the data is only guaranteed to be useful
    /// when the refs > 0.
    pub fn raw(&self, key: &H256) -> Option<(DBValue, i32)> {
        if key == &BLAKE_NULL_RLP {
            return Some((NULL_RLP.to_vec(), 1))
        }
        self.data.get(key).cloned()
    }

    /// Remove an element and delete it from storage if reference count reaches zero.
    /// If the value was purged, return the old value.
    pub fn remove_and_purge(&mut self, key: &H256) -> Option<DBValue> {
        if key == &BLAKE_NULL_RLP {
            return None
        }
        match self.data.entry(*key) {
            Entry::Occupied(mut entry) => {
                if entry.get().1 == 1 {
                    Some(entry.remove().0)
                } else {
                    entry.get_mut().1 -= 1;
                    None
                }
            }
            Entry::Vacant(entry) => {
                entry.insert((DBValue::new(), -1));
                None
            }
        }
    }

    /// Consolidate all the entries of `other` into `self`.
    pub fn consolidate(&mut self, mut other: Self) {
        for (key, (value, rc)) in other.drain() {
            let (old_value, old_rc) = self.data.entry(key).or_default();
            if *old_rc <= 0 {
                *old_value = value;
            }
            *old_rc += rc;
            if *old_rc < -1 {
                *old_rc = -1;
            }
        }
    }
}

impl HashDB for MemoryDB {
    fn keys(&self) -> HashMap<H256, i32> {
        self.data
            .iter()
            .filter_map(|(k, v)| {
                if v.1 != 0 {
                    Some((*k, v.1))
                } else {
                    None
                }
            })
            .collect()
    }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if key == &BLAKE_NULL_RLP {
            return Some(NULL_RLP.to_vec())
        }

        match self.data.get(key) {
            Some(&(ref d, rc)) if rc > 0 => Some(d.clone()),
            _ => None,
        }
    }

    fn contains(&self, key: &H256) -> bool {
        if key == &BLAKE_NULL_RLP {
            return true
        }

        match self.data.get(key) {
            Some(&(_, x)) if x > 0 => true,
            _ => false,
        }
    }

    fn insert(&mut self, value: &[u8]) -> H256 {
        if *value == NULL_RLP {
            return BLAKE_NULL_RLP
        }
        let key = blake256(value);
        let (old_value, rc) = self.data.entry(key).or_default();
        if *rc <= 0 {
            *old_value = value.to_vec();
        }
        *rc += 1;
        key
    }

    fn remove(&mut self, key: &H256) {
        if key == &BLAKE_NULL_RLP {
            return
        }
        let (_, rc) = self.data.entry(*key).or_default();
        *rc -= 1;
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codechain_crypto::blake256;

    #[test]
    fn memorydb_remove_and_purge() {
        let hello_bytes = b"Hello world!";
        let hello_key = blake256(hello_bytes);

        let mut m = MemoryDB::new();
        m.remove(&hello_key);
        assert_eq!(m.raw(&hello_key).unwrap().1, -1);
        m.purge();
        assert_eq!(m.raw(&hello_key).unwrap().1, -1);
        m.insert(hello_bytes);
        assert_eq!(m.raw(&hello_key).unwrap().1, 0);
        m.purge();
        assert_eq!(m.raw(&hello_key), None);

        let mut m = MemoryDB::new();
        assert!(m.remove_and_purge(&hello_key).is_none());
        assert_eq!(m.raw(&hello_key).unwrap().1, -1);
        m.insert(hello_bytes);
        m.insert(hello_bytes);
        assert_eq!(m.raw(&hello_key).unwrap().1, 1);
        assert_eq!(&*m.remove_and_purge(&hello_key).unwrap(), hello_bytes);
        assert_eq!(m.raw(&hello_key), None);
        assert!(m.remove_and_purge(&hello_key).is_none());
    }

    #[test]
    fn consolidate() {
        let mut main = MemoryDB::new();
        let mut other = MemoryDB::new();
        let remove_key = other.insert(b"doggo");
        main.remove(&remove_key);

        let insert_key = other.insert(b"arf");
        main.insert(b"arf");

        let negative_remove_key = other.insert(b"negative");
        other.remove(&negative_remove_key); // ref cnt: 0
        other.remove(&negative_remove_key); // ref cnt: -1
        main.remove(&negative_remove_key); // ref cnt: -1

        main.consolidate(other);

        let overlay = main.drain();

        assert_eq!(overlay[&remove_key], (b"doggo".to_vec(), 0));
        assert_eq!(overlay[&insert_key], (b"arf".to_vec(), 2));
        assert_eq!(overlay[&negative_remove_key], (b"negative".to_vec(), -1));
    }
}
