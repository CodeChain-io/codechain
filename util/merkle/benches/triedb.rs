// Copyright 2019 Kodebox, Inc.
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

#![feature(test)]

extern crate codechain_db as cdb;
extern crate codechain_merkle as cmerkle;
extern crate kvdb;
extern crate kvdb_rocksdb as rocksdb;
extern crate primitives;
extern crate rand;
extern crate tempfile;
extern crate test;

use std::path::Path;
use std::sync::Arc;

use cdb::{new_journaldb, Algorithm, JournalDB};
use cmerkle::{Trie, TrieFactory, TrieMut};
use kvdb::DBTransaction;
use primitives::H256;
use rand::random;
use rocksdb::{CompactionProfile, Database, DatabaseConfig};
use tempfile::{tempdir, TempDir};
use test::Bencher;

struct TestDB {
    _dir: TempDir,
    db: Arc<Database>,
    journal: Box<dyn JournalDB>,
    root: H256,
}

impl TestDB {
    // Replicate CodeChain's db config
    fn config(path: &Path) -> DatabaseConfig {
        let mut config = DatabaseConfig::with_columns(Some(1));
        config.memory_budget = Default::default();
        config.compaction = CompactionProfile::auto(path);
        config
    }

    fn populate(path: &Path, size: usize) -> H256 {
        // Create database
        let config = Self::config(path);
        let db = Arc::new(Database::open(&config, path.to_str().unwrap()).unwrap());
        let mut journal = new_journaldb(db.clone(), Algorithm::Archive, Some(0));
        let mut root = H256::new();
        {
            let hashdb = journal.as_hashdb_mut();
            let mut trie = TrieFactory::create(hashdb, &mut root);
            for i in 0..size {
                trie.insert(&i.to_be_bytes(), &i.to_be_bytes()).unwrap();
            }
        }
        let mut batch = DBTransaction::new();
        journal.journal_under(&mut batch, 0, &H256::new()).unwrap();
        db.write_buffered(batch);
        db.flush().unwrap();

        root
    }

    fn new(size: usize) -> Self {
        // Create temporary directory
        let dir = tempdir().unwrap();
        let root = Self::populate(dir.path(), size);

        // Create database
        let config = Self::config(dir.path());
        let db = Arc::new(Database::open(&config, dir.path().to_str().unwrap()).unwrap());
        let journal = new_journaldb(db.clone(), Algorithm::Archive, Some(0));

        Self {
            _dir: dir,
            db,
            journal,
            root,
        }
    }

    fn trie<'db>(&'db self) -> impl Trie + 'db {
        let hashdb = self.journal.as_hashdb();
        TrieFactory::readonly(hashdb, &self.root).unwrap()
    }

    fn trie_mut<'db>(&'db mut self) -> impl TrieMut + 'db {
        let hashdb = self.journal.as_hashdb_mut();
        TrieFactory::create(hashdb, &mut self.root)
    }

    fn flush(&mut self) {
        let mut batch = DBTransaction::new();
        self.journal.journal_under(&mut batch, 0, &H256::new()).unwrap();
        self.db.write_buffered(batch);
        self.db.flush().unwrap();
    }
}

const DB_SIZE: usize = 10000;
const BATCH: usize = 10000;

#[bench]
fn bench_read_single(b: &mut Bencher) {
    let db = TestDB::new(DB_SIZE);
    b.iter(|| {
        let trie = db.trie();
        let item = random::<usize>() % DB_SIZE;
        let _ = trie.get(&item.to_be_bytes()).unwrap().unwrap();
    });
}

#[bench]
fn bench_read_multiple(b: &mut Bencher) {
    let db = TestDB::new(DB_SIZE);
    b.iter(|| {
        let trie = db.trie();
        for _ in 0..BATCH {
            let item = random::<usize>() % DB_SIZE;
            let _ = trie.get(&item.to_be_bytes()).unwrap().unwrap();
        }
    });
}

#[bench]
fn bench_write_single(b: &mut Bencher) {
    let mut db = TestDB::new(DB_SIZE);
    b.iter(|| {
        {
            let mut trie = db.trie_mut();
            let item = random::<usize>() % DB_SIZE + DB_SIZE;
            let _ = trie.insert(&item.to_be_bytes(), &item.to_be_bytes()).unwrap();
        }
        db.flush();
    });
}

#[bench]
fn bench_write_multiple(b: &mut Bencher) {
    let mut db = TestDB::new(DB_SIZE);
    b.iter(|| {
        {
            let mut trie = db.trie_mut();
            for _ in 0..BATCH {
                let item = random::<usize>() % DB_SIZE + DB_SIZE;
                let _ = trie.insert(&item.to_be_bytes(), &item.to_be_bytes()).unwrap();
            }
        }
        db.flush();
    });
}
