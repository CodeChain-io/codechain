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

use ccrypto::blake256;
use cdb::HashDB;
use primitives::H256;

use crate::nibbleslice::NibbleSlice;
use crate::node::Node as RlpNode;
use crate::{Trie, TrieError};
/// A `Trie` implementation using a generic `HashDB` backing database.
///
/// Use it as a `Trie` trait object. You can use `db()` to get the backing database object.
/// Use `get` and `contains` to query values associated with keys in the trie.
///
/// # Example
/// ```
/// extern crate codechain_db as cdb;
/// extern crate primitives;
/// extern crate codechain_merkle as cmerkle;
///
/// use cmerkle::*;
/// use cdb::*;
/// use primitives::H256;
///
/// let mut memdb = MemoryDB::new();
/// let mut root = H256::new();
/// TrieFactory::create(&mut memdb, &mut root).insert(b"foo", b"bar").unwrap();
/// let t = TrieFactory::readonly(&memdb, &root).unwrap();
/// assert!(t.contains(b"foo").unwrap());
/// assert_eq!(t.get(b"foo").unwrap().unwrap(), b"bar".to_vec());
/// ```
pub(crate) struct TrieDB<'db> {
    db: &'db dyn HashDB,
    root: &'db H256,
}

/// Description of what kind of query will be made to the trie.
type Query<T> = dyn Fn(&[u8]) -> T;

impl<'db> TrieDB<'db> {
    /// Create a new trie with the backing database `db` and `root`
    /// Returns an error if `root` does not exist
    pub fn try_new(db: &'db dyn HashDB, root: &'db H256) -> crate::Result<Self> {
        if !db.contains(root) {
            Err(TrieError::InvalidStateRoot(*root))
        } else {
            Ok(TrieDB {
                db,
                root,
            })
        }
    }

    /// Get auxiliary
    fn get_aux<T>(
        &self,
        path: &NibbleSlice,
        cur_node_hash: Option<H256>,
        query: &Query<T>,
    ) -> crate::Result<Option<T>> {
        match cur_node_hash {
            Some(hash) => {
                let node_rlp = self.db.get(&hash).ok_or_else(|| TrieError::IncompleteDatabase(hash))?;

                match RlpNode::decoded(&node_rlp) {
                    Some(RlpNode::Leaf(partial, value)) => {
                        if &partial == path {
                            Ok(Some(query(value)))
                        } else {
                            Ok(None)
                        }
                    }
                    Some(RlpNode::Branch(partial, children)) => {
                        if path.starts_with(&partial) {
                            self.get_aux(
                                &path.mid(partial.len() + 1),
                                children[path.mid(partial.len()).at(0) as usize],
                                query,
                            )
                        } else {
                            Ok(None)
                        }
                    }
                    None => Ok(None),
                }
            }
            None => Ok(None),
        }
    }
}

impl<'db> Trie for TrieDB<'db> {
    fn root(&self) -> &H256 {
        self.root
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, TrieError> {
        let path = blake256(key);
        let root = *self.root;

        self.get_aux(&NibbleSlice::new(&path), Some(root), &|bytes| bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use cdb::MemoryDB;

    use super::*;
    use crate::*;

    #[test]
    fn get() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        {
            let mut t = TrieDBMut::new(&mut memdb, &mut root);
            t.insert(b"A", b"ABC").unwrap();
            t.insert(b"B", b"ABCBA").unwrap();
        }

        let t = TrieDB::try_new(&memdb, &root).unwrap();
        assert_eq!(t.get(b"A"), Ok(Some(b"ABC".to_vec())));
        assert_eq!(t.get(b"B"), Ok(Some(b"ABCBA".to_vec())));
        assert_eq!(t.get(b"C"), Ok(None));
    }
}
