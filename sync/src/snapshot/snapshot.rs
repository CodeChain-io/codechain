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

use std::collections::HashSet;
use std::convert::AsRef;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::iter::once;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ccore::COL_STATE;
use cmerkle::Node;
use journaldb::{self, Algorithm, JournalDB};
use kvdb::KeyValueDB;
use primitives::H256;
use rlp::{Rlp, RlpStream};
use snap;

use super::error::Error;

pub struct Snapshot {
    path: PathBuf,
}

impl Snapshot {
    pub fn try_new<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>, {
        create_dir_all(&path)?;
        Ok(Snapshot {
            path: path.as_ref().to_owned(),
        })
    }
}

impl Snapshot {
    fn file_for(&self, root: &H256) -> PathBuf {
        self.path.join(format!("{:x}", root))
    }

    fn write_nodes<'a, I>(&self, root: &H256, iter: I) -> Result<(), Error>
    where
        I: IntoIterator<Item = &'a (H256, Vec<u8>)>, {
        let file = File::create(self.file_for(root))?;
        let mut snappy = snap::Writer::new(file);

        let mut stream = RlpStream::new();
        stream.begin_unbounded_list();
        for (key, value) in iter {
            stream.begin_list(2);
            stream.append(key);
            stream.append(value);
        }
        stream.complete_unbounded_list();

        snappy.write(&stream.drain())?;
        Ok(())
    }

    fn read_chunk(&self, backing: Arc<KeyValueDB>, root: &H256) -> Result<Chunk, Error> {
        let file = File::open(self.file_for(root))?;
        let mut buf = Vec::new();
        let mut snappy = snap::Reader::new(file);
        snappy.read_to_end(&mut buf)?;

        let rlp = Rlp::new(&buf);
        let mut journal = journaldb::new(backing, Algorithm::Archive, COL_STATE);
        let mut inserted_keys = HashSet::new();
        let mut referenced_keys = HashSet::new();
        referenced_keys.insert(*root);
        for rlp_pair in rlp.iter() {
            if rlp_pair.item_count() != 2 {
                return Err(Error::SyncError("Chunk contains invalid size of pair".to_string()))
            }

            let key = rlp_pair.val_at(0);
            let value: Vec<_> = rlp_pair.val_at(1);

            let node =
                Node::decoded(&value).ok_or_else(|| Error::SyncError("Chunk condtains an invalid node".to_string()))?;

            if journal.contains(&key) {
                cwarn!(SNAPSHOT, "Chunk contains duplicated key: {}", key);
            }

            if let Node::Branch(_, childs) = node {
                for child in &childs {
                    if let Some(child) = child {
                        referenced_keys.insert(*child);
                    }
                }
            }

            let hash_key = journal.insert(&value);
            if hash_key != key {
                return Err(Error::SyncError("Chunk contains an invalid key for a value".to_string()))
            }
            inserted_keys.insert(hash_key);
        }

        let never_referenced_keys: Vec<H256> =
            inserted_keys.iter().filter(|key| !referenced_keys.contains(key)).cloned().collect();

        Ok(Chunk {
            journal,
            never_referenced_keys,
        })
    }
}

struct Chunk {
    journal: Box<JournalDB>,
    never_referenced_keys: Vec<H256>,
}

impl Chunk {
    fn purge(&mut self) -> bool {
        if self.never_referenced_keys.is_empty() {
            return false
        }
        for key in &self.never_referenced_keys {
            self.journal.remove(key);
        }
        self.never_referenced_keys.clear();
        return true
    }

    fn is_deeper_than(&self, root: &H256, max_depth: usize) -> bool {
        let mut stack = Vec::new();
        stack.push((*root, 0));
        while let Some((key, depth)) = stack.pop() {
            match self.journal.get(&key) {
                None => continue,
                Some(_) if depth >= max_depth => return false,
                Some(value) => {
                    if let Some(Node::Branch(_, childs)) = Node::decoded(&value) {
                        for child in &childs {
                            if let Some(child) = child {
                                stack.push((*child, depth + 1));
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn missing_keys(&self, root: &H256) -> Vec<H256> {
        let mut result = Vec::new();
        let mut stack = Vec::new();
        stack.push(*root);
        while let Some(key) = stack.pop() {
            match self.journal.get(&key) {
                None => {
                    result.push(key);
                }
                Some(value) => {
                    if let Some(Node::Branch(_, childs)) = Node::decoded(&value) {
                        for child in &childs {
                            if let Some(child) = child {
                                stack.push(*child);
                            }
                        }
                    }
                }
            }
        }
        result
    }
}

pub trait WriteSnapshot {
    fn write_snapshot(&self, db: &KeyValueDB, root: &H256) -> Result<(), Error>;
}

pub trait ReadSnapshot {
    fn read_snapshot(&self, db: Arc<KeyValueDB>, root: &H256) -> Result<(), Error>;
}

impl WriteSnapshot for Snapshot {
    fn write_snapshot(&self, db: &KeyValueDB, root: &H256) -> Result<(), Error> {
        let root_val = match db.get(COL_STATE, root) {
            Ok(Some(value)) => value.to_vec(),
            Ok(None) => return Err(Error::SyncError("Invalid state root, or the database is empty".to_string())),
            Err(e) => return Err(Error::DBError(e)),
        };

        let children = children_of(db, &root_val)?;
        let mut grandchildren = Vec::new();
        for (_, value) in &children {
            grandchildren.extend(children_of(db, value)?);
        }

        self.write_nodes(root, once(&(*root, root_val)).chain(&children))?;
        for (grandchild, _) in &grandchildren {
            let nodes = enumerate_subtree(db, grandchild)?;
            self.write_nodes(grandchild, &nodes)?;
        }

        Ok(())
    }
}

impl ReadSnapshot for Snapshot {
    fn read_snapshot(&self, db: Arc<KeyValueDB>, root: &H256) -> Result<(), Error> {
        let head = {
            let mut head = self.read_chunk(db.clone(), root)?;
            if head.purge() {
                cinfo!(SNAPSHOT, "Head chunk contains garbages");
            }

            if head.is_deeper_than(root, 2) {
                return Err(Error::SyncError("Head chunk has an invalid shape".to_string()))
            }

            let mut transaction = db.transaction();
            head.journal.inject(&mut transaction)?;
            db.write_buffered(transaction);
            head
        };

        for chunk_root in head.missing_keys(root) {
            let mut chunk = self.read_chunk(db.clone(), &chunk_root)?;
            if chunk.purge() {
                cinfo!(SNAPSHOT, "Chunk contains garbages");
            }

            if chunk.missing_keys(&chunk_root).len() > 0 {
                return Err(Error::SyncError("Chunk is an incomplete trie".to_string()))
            }

            let mut transaction = db.transaction();
            chunk.journal.inject(&mut transaction)?;
            db.write_buffered(transaction);
        }

        Ok(())
    }
}

fn get_node(db: &KeyValueDB, key: &H256) -> Result<Vec<u8>, Error> {
    match db.get(COL_STATE, key) {
        Ok(Some(value)) => Ok(value.to_vec()),
        Ok(None) => Err(Error::NodeNotFound(*key)),
        Err(e) => Err(Error::DBError(e)),
    }
}

fn children_of(db: &KeyValueDB, node: &[u8]) -> Result<Vec<(H256, Vec<u8>)>, Error> {
    let keys = match Node::decoded(node) {
        None => Vec::new(),
        Some(Node::Leaf(..)) => Vec::new(),
        Some(Node::Branch(_, children)) => children.iter().filter_map(|child| *child).collect(),
    };

    let mut result = Vec::new();
    for key in keys {
        result.push((key, get_node(db, &key)?));
    }
    Ok(result)
}

fn enumerate_subtree(db: &KeyValueDB, root: &H256) -> Result<Vec<(H256, Vec<u8>)>, Error> {
    let node = get_node(db, root)?;
    let children = match Node::decoded(&node) {
        None => Vec::new(),
        Some(Node::Leaf(..)) => Vec::new(),
        Some(Node::Branch(_, children)) => children.iter().filter_map(|child| *child).collect(),
    };
    let mut result: Vec<_> = vec![(*root, node)];
    for child in children {
        result.extend(enumerate_subtree(db, &child)?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use ccore::COL_STATE;

    use cmerkle::{Trie, TrieDB, TrieDBMut, TrieMut};
    use hashdb::DBValue;
    use journaldb;
    use journaldb::Algorithm;
    use kvdb_memorydb;
    use primitives::H256;
    use tempfile::tempdir;
    use trie_standardmap::{Alphabet, StandardMap, ValueMode};

    use super::{ReadSnapshot, Snapshot, WriteSnapshot};

    #[test]
    fn init() {
        let snapshot_dir = tempdir().unwrap();
        let snapshot = Snapshot::try_new(&snapshot_dir).unwrap();
        let mut root = H256::new();

        let kvdb = Arc::new(kvdb_memorydb::create(1));
        let mut jdb = journaldb::new(kvdb.clone(), Algorithm::Archive, COL_STATE);
        {
            let _ = TrieDBMut::new(jdb.as_hashdb_mut(), &mut root);
        }
        /* do nothing */
        let result = snapshot.write_snapshot(kvdb.as_ref(), &root);

        assert!(result.is_err());
    }

    fn random_insert_and_restore_with_count(count: usize) {
        let mut seed = H256::new();
        let x = StandardMap {
            alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
            min_key: 5,
            journal_key: 0,
            value_mode: ValueMode::Index,
            count,
        }.make_with(&mut seed);

        let snapshot_dir = tempdir().unwrap();
        let snapshot = Snapshot::try_new(&snapshot_dir).unwrap();
        let mut root = H256::new();
        {
            let kvdb = Arc::new(kvdb_memorydb::create(1));
            let mut jdb = journaldb::new(kvdb.clone(), Algorithm::Archive, COL_STATE);
            {
                let mut t = TrieDBMut::new(jdb.as_hashdb_mut(), &mut root);
                let mut inserted_keys = HashSet::new();
                for &(ref key, ref value) in &x {
                    if inserted_keys.insert(key) == false {
                        continue
                    }
                    assert!(t.insert(key, value).unwrap().is_none());
                    assert_eq!(t.insert(key, value).unwrap(), Some(DBValue::from_slice(value)));
                }
            }
            {
                let mut batch = jdb.backing().transaction();
                let _ = jdb.inject(&mut batch).unwrap();
                jdb.backing().write(batch).unwrap();
            }

            snapshot.write_snapshot(kvdb.as_ref(), &root).unwrap();
        }

        {
            let kvdb = Arc::new(kvdb_memorydb::create(1));
            snapshot.read_snapshot(kvdb.clone(), &root).unwrap();

            let mut jdb = journaldb::new(kvdb.clone(), Algorithm::Archive, COL_STATE);
            let t = TrieDB::new(jdb.as_hashdb_mut(), &mut root).unwrap();
            let mut inserted_keys = HashSet::new();
            for &(ref key, ref value) in &x {
                if inserted_keys.insert(key) == false {
                    continue
                }
                assert_eq!(t.get(key).unwrap(), Some(DBValue::from_slice(value)));
            }
        }
    }

    #[test]
    fn random_insert_and_restore_1() {
        random_insert_and_restore_with_count(1);
    }

    #[test]
    fn random_insert_and_restore_100() {
        random_insert_and_restore_with_count(100);
    }

    #[test]
    fn random_insert_and_restore_10000() {
        random_insert_and_restore_with_count(10000);
    }
}
