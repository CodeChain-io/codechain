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

extern crate codechain_crypto as ccrypto;
extern crate elastic_array;
extern crate hashdb;
#[cfg(test)]
extern crate memorydb;
extern crate primitives;
extern crate rlp;

#[cfg(test)]
extern crate trie_standardmap as standardmap;

use std::fmt;

use ccrypto::BLAKE_NULL_RLP;
use hashdb::{DBValue, HashDB};
use primitives::H256;

mod nibbleslice;
pub mod node;
mod skewed;
pub mod triedb;
pub mod triedbmut;
pub mod triehash;

pub use crate::node::Node;
pub use crate::skewed::skewed_merkle_root;
pub use crate::triedb::TrieDB;
pub use crate::triedbmut::TrieDBMut;

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

/// Trie result type. Boxed to avoid copying around extra space for `H256`s on successful queries.
pub type Result<T> = ::std::result::Result<T, Box<TrieError>>;

/// Description of what kind of query will be made to the trie.
pub trait Query {
    /// Output item.
    type Item;

    /// Decode a byte-slice into the desired item.
    fn decode(self, &[u8]) -> Self::Item;
}

impl<F, T> Query for F
where
    F: for<'a> FnOnce(&'a [u8]) -> T,
{
    type Item = T;

    fn decode(self, value: &[u8]) -> T {
        (self)(value)
    }
}

/// A key-value datastore implemented as a database-backed modified Merkle tree.
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
    fn get(&self, key: &[u8]) -> Result<Option<DBValue>> {
        self.get_with(key, DBValue::from_slice)
    }

    /// Search for the key with the given query parameter. See the docs of the `Query`
    /// trait for more details.
    fn get_with<Q: Query>(&self, key: &[u8], query: Q) -> Result<Option<Q::Item>>;
}

/// A key-value datastore implemented as a database-backed modified Merkle tree.
pub trait TrieMut {
    /// Return the root of the trie.
    fn root(&self) -> &H256;

    /// Is the trie empty?
    fn is_empty(&self) -> bool;

    /// Does the trie contain a given key?
    fn contains(&self, key: &[u8]) -> Result<bool> {
        self.get(key).map(|x| x.is_some())
    }

    /// What is the value of the given key in this trie?
    fn get(&self, key: &[u8]) -> Result<Option<DBValue>>;

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
    pub fn readonly<'db>(db: &'db HashDB, root: &'db H256) -> Result<TrieDB<'db>> {
        Ok(TrieDB::try_new(db, root)?)
    }

    /// Create new mutable instance of Trie.
    pub fn create<'db>(db: &'db mut HashDB, root: &'db mut H256) -> Box<TrieMut + 'db> {
        Box::new(TrieDBMut::new(db, root))
    }

    /// Create new mutable instance of trie and check for errors.
    pub fn from_existing<'db>(db: &'db mut HashDB, root: &'db mut H256) -> Result<Box<TrieMut + 'db>> {
        Ok(Box::new(TrieDBMut::from_existing(db, root)?))
    }
}
