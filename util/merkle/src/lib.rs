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

extern crate codechain_crypto as ccrypto;
extern crate codechain_db as cdb;
extern crate primitives;
extern crate rlp;

#[cfg(test)]
extern crate trie_standardmap as standardmap;

use std::fmt;

use ccrypto::BLAKE_NULL_RLP;
use cdb::{DBValue, HashDB};
use primitives::H256;

mod nibbleslice;
pub mod node;
mod skewed;
pub mod triedb;
pub mod triedbmut;
pub mod triehash;

pub use crate::node::Node;
pub use crate::skewed::skewed_merkle_root;
use crate::triedb::TrieDB;
use crate::triedbmut::TrieDBMut;

/// Trie Errors.
///
/// These borrow the data within them to avoid excessive copying on every
/// trie operation.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TrieError {
    /// Attempted to create a trie with a state root not in the DB.
    InvalidStateRoot(H256),
    /// Trie item not found in the database,
    IncompleteDatabase(H256),
}

impl fmt::Display for TrieError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrieError::InvalidStateRoot(root) => write!(f, "Invalid state root: {}", root),
            TrieError::IncompleteDatabase(missing) => write!(f, "Database missing expected key: {}", missing),
        }
    }
}

pub type Result<T> = ::std::result::Result<T, TrieError>;

/// A key-value datastore implemented as a database-backed Merkle trie.
pub trait Trie {
    /// Return the root of the trie.
    fn root(&self) -> &H256;

    /// Is the trie empty?
    fn is_empty(&self) -> bool {
        *self.root() == BLAKE_NULL_RLP
    }

    /// Does the trie contain a given key?
    fn contains(&self, key: &[u8]) -> Result<bool> {
        self.get(key).map(|x| x.is_some())
    }

    /// What is the value of the given key in this trie?
    fn get(&self, key: &[u8]) -> Result<Option<DBValue>>;
}

/// A key-value datastore implemented as a database-backed modified Merkle tree.
pub trait TrieMut: Trie {
    /// Insert a `key`/`value` pair into the trie. An empty value is equivalent to removing
    /// `key` from the trie. Returns the old value associated with this key, if it existed.
    fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<Option<DBValue>>;

    /// Remove a `key` from the trie. Equivalent to making it equal to the empty
    /// value. Returns the old value associated with this key, if it existed.
    fn remove(&mut self, key: &[u8]) -> Result<Option<DBValue>>;
}

pub enum TrieFactory {}

impl TrieFactory {
    /// Create new immutable instance of Trie.
    pub fn readonly<'db>(db: &'db dyn HashDB, root: &'db H256) -> Result<impl Trie + 'db> {
        Ok(TrieDB::try_new(db, root)?)
    }

    /// Create new mutable instance of Trie.
    pub fn create<'db>(db: &'db mut dyn HashDB, root: &'db mut H256) -> impl TrieMut + 'db {
        TrieDBMut::new(db, root)
    }

    /// Create new mutable instance of trie and check for errors.
    pub fn from_existing<'db>(db: &'db mut dyn HashDB, root: &'db mut H256) -> Result<impl TrieMut + 'db> {
        Ok(TrieDBMut::from_existing(db, root)?)
    }
}
