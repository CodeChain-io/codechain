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

//! Disk-backed `HashDB` implementation.

use std::collections::HashMap;
use std::sync::Arc;

use kvdb::{DBTransaction, KeyValueDB};
use primitives::{Bytes, H256};
use rlp::{decode, encode};

use super::traits::JournalDB;
use super::{DB_PREFIX_LEN, LATEST_ERA_KEY};
use crate::hashdb::*;
use crate::memorydb::*;
use crate::DatabaseError;

/// Implementation of the `HashDB` trait for a disk-backed database with a memory overlay
/// and latent-removal semantics.
///
/// Like `OverlayDB`, there is a memory overlay; `commit()` must be called in order to
/// write operations out to disk. Unlike `OverlayDB`, `remove()` operations do not take effect
/// immediately. As this is an "archive" database, nothing is ever removed. This means
/// that the states of any block the node has ever processed will be accessible.
pub struct ArchiveDB {
    overlay: MemoryDB,
    backing: Arc<dyn KeyValueDB>,
    latest_era: Option<u64>,
    column: Option<u32>,
}

impl ArchiveDB {
    /// Create a new instance from a key-value db.
    pub fn new(backing: Arc<dyn KeyValueDB>, col: Option<u32>) -> ArchiveDB {
        let latest_era = backing
            .get(col, &LATEST_ERA_KEY)
            .expect("Low-level database error.")
            .map(|val| decode::<u64>(&val).unwrap());
        ArchiveDB {
            overlay: MemoryDB::new(),
            backing,
            latest_era,
            column: col,
        }
    }

    fn payload(&self, key: &H256) -> Option<DBValue> {
        self.backing
            .get(self.column, &**key)
            .expect("Low-level database error. Some issue with your hard disk?")
            .map(|data| data.to_vec())
    }
}

impl HashDB for ArchiveDB {
    fn keys(&self) -> HashMap<H256, i32> {
        let mut ret: HashMap<H256, i32> =
            self.backing.iter(self.column).map(|(key, _)| (H256::from_slice(&*key), 1)).collect();

        for (key, refs) in self.overlay.keys() {
            let rc = ret.entry(key).or_default();
            *rc += refs;
            assert!(*rc >= -1, "rc should be equal to or greater than -1, but {}", rc);
        }
        ret
    }

    fn get(&self, key: &H256) -> Option<DBValue> {
        if let Some((d, rc)) = self.overlay.raw(key) {
            if rc > 0 {
                return Some(d)
            }
        }
        self.payload(key)
    }

    fn contains(&self, key: &H256) -> bool {
        self.get(key).is_some()
    }

    fn insert(&mut self, value: &[u8]) -> H256 {
        self.overlay.insert(value)
    }

    fn remove(&mut self, key: &H256) {
        self.overlay.remove(key);
    }

    fn is_empty(&self) -> bool {
        self.latest_era.is_none()
    }
}

impl JournalDB for ArchiveDB {
    fn boxed_clone(&self) -> Box<dyn JournalDB> {
        Box::new(ArchiveDB {
            overlay: self.overlay.clone(),
            backing: self.backing.clone(),
            latest_era: self.latest_era,
            column: self.column,
        })
    }

    fn latest_era(&self) -> Option<u64> {
        self.latest_era
    }

    fn journal_under(&mut self, batch: &mut DBTransaction, now: u64, _id: &H256) -> Result<u32, DatabaseError> {
        let mut inserts = 0usize;
        let mut deletes = 0usize;

        for i in self.overlay.drain() {
            let (key, (value, rc)) = i;
            if rc > 0 {
                batch.put(self.column, &key, &value);
                inserts += 1;
            }
            if rc < 0 {
                assert_eq!(-1, rc);
                deletes += 1;
            }
        }

        if self.latest_era.map_or(true, |e| now > e) {
            batch.put(self.column, &LATEST_ERA_KEY, &encode(&now));
            self.latest_era = Some(now);
        }
        Ok((inserts + deletes) as u32)
    }

    fn mark_canonical(
        &mut self,
        _batch: &mut DBTransaction,
        _end_era: u64,
        _canon_id: &H256,
    ) -> Result<u32, DatabaseError> {
        // keep everything! it's an archive, after all.
        Ok(0)
    }

    fn inject(&mut self, batch: &mut DBTransaction) -> Result<u32, DatabaseError> {
        let mut inserts = 0usize;
        let mut deletes = 0usize;

        for i in self.overlay.drain() {
            let (key, (value, rc)) = i;
            if rc > 0 {
                if self.backing.get(self.column, &key)?.is_some() {
                    return Err(DatabaseError::AlreadyExists(key))
                }
                batch.put(self.column, &key, &value);
                inserts += 1;
            }
            if rc < 0 {
                assert_eq!(-1, rc);
                if self.backing.get(self.column, &key)?.is_none() {
                    return Err(DatabaseError::NegativelyReferencedHash(key))
                }
                batch.delete(self.column, &key);
                deletes += 1;
            }
        }

        Ok((inserts + deletes) as u32)
    }

    fn state(&self, id: &H256) -> Option<Bytes> {
        self.backing.get_by_prefix(self.column, &id[0..DB_PREFIX_LEN]).map(<[u8]>::into_vec)
    }

    fn is_pruned(&self) -> bool {
        false
    }

    fn backing(&self) -> &Arc<dyn KeyValueDB> {
        &self.backing
    }

    fn consolidate(&mut self, with: MemoryDB) {
        self.overlay.consolidate(with);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::blake256;
    use {kvdb_memorydb, JournalDB};

    #[test]
    fn insert_same_in_fork() {
        // history is 1
        let mut jdb = ArchiveDB::new(Arc::new(kvdb_memorydb::create(0)), None);

        let x = jdb.insert(b"X");
        jdb.commit_batch(1, &blake256(b"1"), None).unwrap();
        jdb.commit_batch(2, &blake256(b"2"), None).unwrap();
        jdb.commit_batch(3, &blake256(b"1002a"), Some((1, blake256(b"1")))).unwrap();
        jdb.commit_batch(4, &blake256(b"1003a"), Some((2, blake256(b"2")))).unwrap();

        jdb.remove(&x);
        jdb.commit_batch(3, &blake256(b"1002b"), Some((1, blake256(b"1")))).unwrap();
        let x = jdb.insert(b"X");
        jdb.commit_batch(4, &blake256(b"1003b"), Some((2, blake256(b"2")))).unwrap();

        jdb.commit_batch(5, &blake256(b"1004a"), Some((3, blake256(b"1002a")))).unwrap();
        jdb.commit_batch(6, &blake256(b"1005a"), Some((4, blake256(b"1003a")))).unwrap();

        assert!(jdb.contains(&x));
    }

    #[test]
    fn long_history() {
        // history is 3
        let mut jdb = ArchiveDB::new(Arc::new(kvdb_memorydb::create(0)), None);
        let h = jdb.insert(b"foo");
        jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
        assert!(jdb.contains(&h));
        jdb.remove(&h);
        jdb.commit_batch(1, &blake256(b"1"), None).unwrap();
        assert!(jdb.contains(&h));
        jdb.commit_batch(2, &blake256(b"2"), None).unwrap();
        assert!(jdb.contains(&h));
        jdb.commit_batch(3, &blake256(b"3"), Some((0, blake256(b"0")))).unwrap();
        assert!(jdb.contains(&h));
        jdb.commit_batch(4, &blake256(b"4"), Some((1, blake256(b"1")))).unwrap();
        assert!(jdb.contains(&h));
    }

    #[test]
    #[should_panic]
    fn multiple_owed_removal_not_allowed() {
        let mut jdb = ArchiveDB::new(Arc::new(kvdb_memorydb::create(0)), None);
        let h = jdb.insert(b"foo");
        jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
        assert!(jdb.contains(&h));
        jdb.remove(&h);
        jdb.remove(&h);
        // commit_batch would call journal_under(),
        // and we don't allow multiple owned removals.
        jdb.commit_batch(1, &blake256(b"1"), None).unwrap();
    }

    #[test]
    fn complex() {
        // history is 1
        let mut jdb = ArchiveDB::new(Arc::new(kvdb_memorydb::create(0)), None);

        let foo_hash = jdb.insert(b"foo");
        let bar_hash = jdb.insert(b"bar");
        jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
        assert!(jdb.contains(&foo_hash));
        assert!(jdb.contains(&bar_hash));

        jdb.remove(&foo_hash);
        jdb.remove(&bar_hash);
        let baz_hash = jdb.insert(b"baz");
        jdb.commit_batch(1, &blake256(b"1"), Some((0, blake256(b"0")))).unwrap();
        assert!(jdb.contains(&foo_hash));
        assert!(jdb.contains(&bar_hash));
        assert!(jdb.contains(&baz_hash));

        let foo_hash = jdb.insert(b"foo");
        jdb.remove(&baz_hash);
        jdb.commit_batch(2, &blake256(b"2"), Some((1, blake256(b"1")))).unwrap();
        assert!(jdb.contains(&foo_hash));
        assert!(jdb.contains(&baz_hash));

        jdb.remove(&foo_hash);
        jdb.commit_batch(3, &blake256(b"3"), Some((2, blake256(b"2")))).unwrap();
        assert!(jdb.contains(&foo_hash));

        jdb.commit_batch(4, &blake256(b"4"), Some((3, blake256(b"3")))).unwrap();
    }

    #[test]
    fn fork() {
        // history is 1
        let mut jdb = ArchiveDB::new(Arc::new(kvdb_memorydb::create(0)), None);

        let foo_hash = jdb.insert(b"foo");
        let bar_hash = jdb.insert(b"bar");
        jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
        assert!(jdb.contains(&foo_hash));
        assert!(jdb.contains(&bar_hash));

        jdb.remove(&foo_hash);
        let baz_hash = jdb.insert(b"baz");
        jdb.commit_batch(1, &blake256(b"1a"), Some((0, blake256(b"0")))).unwrap();

        jdb.remove(&bar_hash);
        jdb.commit_batch(1, &blake256(b"1b"), Some((0, blake256(b"0")))).unwrap();

        assert!(jdb.contains(&foo_hash));
        assert!(jdb.contains(&bar_hash));
        assert!(jdb.contains(&baz_hash));

        jdb.commit_batch(2, &blake256(b"2b"), Some((1, blake256(b"1b")))).unwrap();
        assert!(jdb.contains(&foo_hash));
    }

    #[test]
    fn overwrite() {
        // history is 1
        let mut jdb = ArchiveDB::new(Arc::new(kvdb_memorydb::create(0)), None);

        let foo_hash = jdb.insert(b"foo");
        jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
        assert!(jdb.contains(&foo_hash));

        jdb.remove(&foo_hash);
        jdb.commit_batch(1, &blake256(b"1"), Some((0, blake256(b"0")))).unwrap();
        jdb.insert(b"foo");
        assert!(jdb.contains(&foo_hash));
        jdb.commit_batch(2, &blake256(b"2"), Some((1, blake256(b"1")))).unwrap();
        assert!(jdb.contains(&foo_hash));
        jdb.commit_batch(3, &blake256(b"2"), Some((0, blake256(b"2")))).unwrap();
        assert!(jdb.contains(&foo_hash));
    }

    #[test]
    fn fork_same_key() {
        // history is 1
        let mut jdb = ArchiveDB::new(Arc::new(kvdb_memorydb::create(0)), None);
        jdb.commit_batch(0, &blake256(b"0"), None).unwrap();

        let foo_hash = jdb.insert(b"foo");
        jdb.commit_batch(1, &blake256(b"1a"), Some((0, blake256(b"0")))).unwrap();

        jdb.insert(b"foo");
        jdb.commit_batch(1, &blake256(b"1b"), Some((0, blake256(b"0")))).unwrap();
        assert!(jdb.contains(&foo_hash));

        jdb.commit_batch(2, &blake256(b"2a"), Some((1, blake256(b"1a")))).unwrap();
        assert!(jdb.contains(&foo_hash));
    }

    #[test]
    fn reopen() {
        let shared_db = Arc::new(kvdb_memorydb::create(0));

        let (foo_hash, bar_hash) = {
            let mut jdb = ArchiveDB::new(shared_db.clone(), None);
            // history is 1
            let foo_hash = jdb.insert(b"foo");
            let bar_hash = jdb.insert(b"bar");
            jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
            (foo_hash, bar_hash)
        };

        {
            let mut jdb = ArchiveDB::new(shared_db.clone(), None);
            jdb.remove(&foo_hash);
            jdb.commit_batch(1, &blake256(b"1"), Some((0, blake256(b"0")))).unwrap();
        }

        {
            let mut jdb = ArchiveDB::new(shared_db, None);
            assert!(jdb.contains(&foo_hash));
            assert!(jdb.contains(&bar_hash));
            jdb.commit_batch(2, &blake256(b"2"), Some((1, blake256(b"1")))).unwrap();
        }
    }

    #[test]
    fn reopen_remove() {
        let shared_db = Arc::new(kvdb_memorydb::create(0));

        let foo_hash = {
            let mut jdb = ArchiveDB::new(shared_db.clone(), None);
            // history is 1
            let foo_hash = jdb.insert(b"foo");
            jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
            jdb.commit_batch(1, &blake256(b"1"), Some((0, blake256(b"0")))).unwrap();

            // foo is ancient history.

            jdb.insert(b"foo");
            jdb.commit_batch(2, &blake256(b"2"), Some((1, blake256(b"1")))).unwrap();
            foo_hash
        };

        {
            let mut jdb = ArchiveDB::new(shared_db, None);
            jdb.remove(&foo_hash);
            jdb.commit_batch(3, &blake256(b"3"), Some((2, blake256(b"2")))).unwrap();
            assert!(jdb.contains(&foo_hash));
            jdb.remove(&foo_hash);
            jdb.commit_batch(4, &blake256(b"4"), Some((3, blake256(b"3")))).unwrap();
            jdb.commit_batch(5, &blake256(b"5"), Some((4, blake256(b"4")))).unwrap();
        }
    }

    #[test]
    fn reopen_fork() {
        let shared_db = Arc::new(kvdb_memorydb::create(0));
        let (foo_hash, ..) = {
            let mut jdb = ArchiveDB::new(shared_db.clone(), None);
            // history is 1
            let foo_hash = jdb.insert(b"foo");
            let bar_hash = jdb.insert(b"bar");
            jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
            jdb.remove(&foo_hash);
            let baz_hash = jdb.insert(b"baz");
            jdb.commit_batch(1, &blake256(b"1a"), Some((0, blake256(b"0")))).unwrap();

            jdb.remove(&bar_hash);
            jdb.commit_batch(1, &blake256(b"1b"), Some((0, blake256(b"0")))).unwrap();
            (foo_hash, bar_hash, baz_hash)
        };

        {
            let mut jdb = ArchiveDB::new(shared_db, None);
            jdb.commit_batch(2, &blake256(b"2b"), Some((1, blake256(b"1b")))).unwrap();
            assert!(jdb.contains(&foo_hash));
        }
    }

    #[test]
    fn return_state() {
        let shared_db = Arc::new(kvdb_memorydb::create(0));

        let key = {
            let mut jdb = ArchiveDB::new(shared_db.clone(), None);
            let key = jdb.insert(b"foo");
            jdb.commit_batch(0, &blake256(b"0"), None).unwrap();
            key
        };

        {
            let jdb = ArchiveDB::new(shared_db, None);
            let state = jdb.state(&key);
            assert_eq!(Some("foo".to_string().into_bytes()), state);
        }
    }

    #[test]
    fn inject() {
        let mut jdb = ArchiveDB::new(Arc::new(kvdb_memorydb::create(0)), None);
        let key = jdb.insert(b"dog");
        jdb.inject_batch().unwrap();

        assert_eq!(jdb.get(&key).unwrap(), b"dog".to_vec());
        jdb.remove(&key);
        jdb.inject_batch().unwrap();

        assert_eq!(None, jdb.get(&key));
    }
}
