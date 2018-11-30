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
use std::fmt;

use ccrypto::{blake256, BLAKE_NULL_RLP};
use hashdb::DBValue;
use hashdb::HashDB;
use primitives::H256;

use crate::{Trie, TrieError, TrieMut};
use nibbleslice::NibbleSlice;
use node::Node as RlpNode;
use triedb::TrieDB;


fn empty_children() -> [Option<H256>; 16] {
    [None; 16]
}

pub struct TrieDBMut<'a> {
    db: &'a mut HashDB,
    // When Trie is empty, root has None.
    root: &'a mut H256,
}

impl<'a> TrieDBMut<'a> {
    /// Create a new trie with backing database `db` and empty `root`.
    pub fn new(db: &'a mut HashDB, root: &'a mut H256) -> Self {
        *root = BLAKE_NULL_RLP;

        TrieDBMut {
            db,
            root,
        }
    }

    /// Create a new trie with the backing database `db` and `root.
    /// Returns an error if `root` does not exist.
    pub fn from_existing(db: &'a mut HashDB, root: &'a mut H256) -> crate::Result<Self> {
        if !db.contains(root) {
            return Err(Box::new(TrieError::InvalidStateRoot(*root)))
        }

        Ok(TrieDBMut {
            db,
            root,
        })
    }

    /// Insert auxiliary
    fn insert_aux(
        &mut self,
        path: NibbleSlice,
        insert_value: &[u8],
        cur_node_hash: Option<H256>,
        old_val: &mut Option<DBValue>,
    ) -> crate::Result<H256> {
        match cur_node_hash {
            Some(hash) => {
                let node_rlp = self.db.get(&hash).ok_or_else(|| Box::new(TrieError::IncompleteDatabase(hash)))?;

                match RlpNode::decoded(&node_rlp) {
                    Some(RlpNode::Leaf(partial, value)) => {
                        // Renew the Leaf
                        if partial == path {
                            let node = RlpNode::Leaf(path, insert_value);
                            let node_rlp = RlpNode::encoded(node);
                            let hash = self.db.insert(&node_rlp);

                            *old_val = Some(DBValue::from_slice(value));

                            Ok(hash)
                        } else {
                            // Make branch node and insert Leaves
                            let common = partial.common_prefix(&path);
                            let mut new_child = empty_children();
                            let new_partial = partial.mid(common);
                            let new_path = path.mid(common);

                            new_child[new_partial.at(0) as usize] = Some(self.insert_aux(
                                new_partial.mid(1),
                                value,
                                new_child[new_partial.at(0) as usize],
                                old_val,
                            )?);
                            new_child[new_path.at(0) as usize] = Some(self.insert_aux(
                                new_path.mid(1),
                                insert_value,
                                new_child[new_path.at(0) as usize],
                                old_val,
                            )?);

                            let node_rlp = RlpNode::encoded_until(RlpNode::Branch(partial, new_child.into()), common);
                            let hash = self.db.insert(&node_rlp);

                            Ok(hash)
                        }
                    }
                    Some(RlpNode::Branch(partial, mut children)) => {
                        let common = partial.common_prefix(&path);

                        // Make new branch node and insert leaf and branch with new path
                        if common < partial.len() {
                            let mut new_child = empty_children();
                            let new_partial = partial.mid(common);
                            let new_path = path.mid(common);
                            let o_branch = RlpNode::Branch(new_partial.mid(1), children);

                            let mut node_rlp = RlpNode::encoded(o_branch);
                            let b_hash = self.db.insert(&node_rlp);

                            new_child[new_partial.at(0) as usize] = Some(b_hash);
                            new_child[new_path.at(0) as usize] = Some(self.insert_aux(
                                new_path.mid(1),
                                insert_value,
                                new_child[new_path.at(0) as usize],
                                old_val,
                            )?);

                            node_rlp = RlpNode::encoded_until(RlpNode::Branch(partial, new_child.into()), common);
                            let hash = self.db.insert(&node_rlp);

                            Ok(hash)
                        } else {
                            // Insert leaf into the branch node
                            let new_path = path.mid(common);

                            children[new_path.at(0) as usize] = Some(self.insert_aux(
                                new_path.mid(1),
                                insert_value,
                                children[new_path.at(0) as usize],
                                old_val,
                            )?);

                            let new_branch = RlpNode::Branch(partial, children);
                            let node_rlp = RlpNode::encoded(new_branch);
                            let hash = self.db.insert(&node_rlp);

                            Ok(hash)
                        }
                    }
                    None => {
                        let node = RlpNode::Leaf(path, insert_value);
                        let node_rlp = RlpNode::encoded(node);
                        let hash = self.db.insert(&node_rlp);

                        Ok(hash)
                    }
                }
            }
            None => {
                let node = RlpNode::Leaf(path, insert_value);
                let node_rlp = RlpNode::encoded(node);
                let hash = self.db.insert(&node_rlp);

                Ok(hash)
            }
        }
    }

    /// Remove auxiliary
    fn remove_aux(
        &mut self,
        path: &NibbleSlice,
        cur_node_hash: Option<H256>,
        old_val: &mut Option<DBValue>,
    ) -> crate::Result<Option<H256>> {
        match cur_node_hash {
            Some(hash) => {
                let node_rlp = self.db.get(&hash).ok_or_else(|| Box::new(TrieError::IncompleteDatabase(hash)))?;

                match RlpNode::decoded(&node_rlp) {
                    Some(RlpNode::Leaf(partial, value)) => {
                        if path == &partial {
                            *old_val = Some(DBValue::from_slice(&value));

                            Ok(None)
                        } else {
                            Ok(cur_node_hash)
                        }
                    }
                    Some(RlpNode::Branch(partial, mut children)) => {
                        if path.starts_with(&partial) {
                            let new_path = path.mid(partial.len());
                            children[new_path.at(0) as usize] =
                                self.remove_aux(&new_path.mid(1), children[new_path.at(0) as usize], old_val)?;

                            if children[new_path.at(0) as usize] == None {
                                // Fix the node
                                let child_count = children.iter().filter(|x| x.is_none()).count();

                                match child_count {
                                    16 => {
                                        // Branch can be removed
                                        Ok(None)
                                    }
                                    15 => {
                                        // Transform the branch into Leaf
                                        let index = children
                                            .iter()
                                            .position(Option::is_some)
                                            .expect("Can not find leaf in the branch");
                                        let new_leaf_hash = children[index].expect("Index is wrong");
                                        let new_leaf_data = self
                                            .db
                                            .get(&new_leaf_hash)
                                            .ok_or_else(|| Box::new(TrieError::IncompleteDatabase(hash)))?;
                                        let new_leaf_node = RlpNode::decoded(&new_leaf_data);

                                        match new_leaf_node {
                                            None => Err(Box::new(TrieError::IncompleteDatabase(hash))),
                                            Some(RlpNode::Leaf(child_partial, child_value)) => {
                                                let mut vec = partial.to_vec();
                                                vec.push(index as u8);
                                                vec.append(&mut child_partial.to_vec());

                                                let (new_partial, offset) = NibbleSlice::from_vec(&vec);
                                                let new_leaf = RlpNode::Leaf(
                                                    NibbleSlice::new_offset(&new_partial, offset),
                                                    child_value,
                                                );
                                                let mut node_rlp = RlpNode::encoded(new_leaf);
                                                let new_hash = self.db.insert(&node_rlp);

                                                Ok(Some(new_hash))
                                            }
                                            Some(RlpNode::Branch(child_partial, children)) => {
                                                let mut vec = partial.to_vec();
                                                vec.push(index as u8);
                                                vec.append(&mut child_partial.to_vec());

                                                let (new_partial, offset) = NibbleSlice::from_vec(&vec);
                                                let new_branch = RlpNode::Branch(
                                                    NibbleSlice::new_offset(&new_partial, offset),
                                                    children,
                                                );
                                                let mut node_rlp = RlpNode::encoded(new_branch);
                                                let new_hash = self.db.insert(&node_rlp);

                                                Ok(Some(new_hash))
                                            }
                                        }
                                    }
                                    _ => {
                                        let new_branch = RlpNode::Branch(partial, children);
                                        let mut node_rlp = RlpNode::encoded(new_branch);
                                        let new_hash = self.db.insert(&node_rlp);

                                        Ok(Some(new_hash))
                                    }
                                }
                            } else {
                                let new_branch = RlpNode::Branch(partial, children);
                                let mut node_rlp = RlpNode::encoded(new_branch);
                                let new_hash = self.db.insert(&node_rlp);

                                Ok(Some(new_hash))
                            }
                        } else {
                            Ok(cur_node_hash)
                        }
                    }
                    None => Ok(cur_node_hash),
                }
            }
            None => Ok(cur_node_hash),
        }
    }
}

impl<'a> fmt::Display for RlpNode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RlpNode::Leaf(partial, value) => writeln!(f, "Leaf - key({:?}), value({:?})", partial, value),
            RlpNode::Branch(partial, children) => {
                writeln!(f, "Branch - path({:?})", partial)?;

                debug_assert_eq!(16, children.len());
                for (i, child) in children.iter().enumerate() {
                    writeln!(f, "child {} - hash({:?})", i, child)?;
                }
                Ok(())
            }
        }
    }
}

impl<'a> TrieMut for TrieDBMut<'a> {
    fn root(&self) -> &H256 {
        self.root
    }

    fn is_empty(&self) -> bool {
        *self.root == BLAKE_NULL_RLP
    }

    fn get(&self, key: &[u8]) -> crate::Result<Option<DBValue>> {
        let t = TrieDB::new(self.db, self.root)?;

        t.get(key)
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) -> crate::Result<Option<DBValue>> {
        let path = blake256(key);
        let mut old_val = None;
        let cur_hash = *self.root;
        *self.root = self.insert_aux(NibbleSlice::new(&path), value, Some(cur_hash), &mut old_val)?;

        Ok(old_val)
    }

    fn remove(&mut self, key: &[u8]) -> crate::Result<Option<DBValue>> {
        let path = blake256(key);
        let mut old_val = None;
        let cur_hash = *self.root;

        *self.root = match self.remove_aux(&NibbleSlice::new(&path), Some(cur_hash), &mut old_val)? {
            Some(hash) => hash,
            None => BLAKE_NULL_RLP,
        };

        Ok(old_val)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use ccrypto::BLAKE_NULL_RLP;
    use crate::triehash::trie_root;
    use crate::TrieMut;
    use memorydb::*;
    use primitives::bytes::ToPretty;
    use standardmap::*;

    fn populate_trie<'db>(db: &'db mut HashDB, root: &'db mut H256, v: &[(Vec<u8>, Vec<u8>)]) -> TrieDBMut<'db> {
        let mut t = TrieDBMut::new(db, root);
        for i in 0..v.len() {
            let key: &[u8] = &v[i].0;
            let val: &[u8] = &v[i].1;
            t.insert(key, val).unwrap();
        }
        t
    }

    fn unpopulate_trie(t: &mut TrieDBMut, v: &[(Vec<u8>, Vec<u8>)]) {
        for i in v {
            let key: &[u8] = &i.0;
            t.remove(key).unwrap();
        }
    }

    #[test]
    fn playpen() {
        let mut seed = H256::new();
        for test_i in 0..10 {
            if test_i % 50 == 0 {
                println!("{:?} of 10000 stress tests done", test_i);
            }
            let x = StandardMap {
                alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
                min_key: 5,
                journal_key: 0,
                value_mode: ValueMode::Index,
                count: 100,
            }
            .make_with(&mut seed);

            let real = trie_root(x.clone());
            let mut memdb = MemoryDB::new();
            let mut root = H256::new();
            let mut memtrie = populate_trie(&mut memdb, &mut root, &x);

            if *memtrie.root() != real {
                println!("TRIE MISMATCH");
                println!();
                println!("{:?} vs {:?}", memtrie.root(), real);
                for i in &x {
                    println!("{:?} -> {:?}", i.0.pretty(), i.1.pretty());
                }
            }
            assert_eq!(*memtrie.root(), real);
            unpopulate_trie(&mut memtrie, &x);

            if *memtrie.root() != BLAKE_NULL_RLP {
                println!("- TRIE MISMATCH");
                println!();
                println!("{:?} vs {:?}", memtrie.root(), real);
                for i in &x {
                    println!("{:?} -> {:?}", i.0.pretty(), i.1.pretty());
                }
            }
            assert_eq!(*memtrie.root(), BLAKE_NULL_RLP);
        }
    }

    #[test]
    fn init() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let t = TrieDBMut::new(&mut memdb, &mut root);
        assert_eq!(*t.root(), BLAKE_NULL_RLP);
    }

    #[test]
    fn insert_on_empty() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        assert_eq!(*t.root(), trie_root(vec![(vec![0x01u8, 0x23], vec![0x01u8, 0x23])]));
    }

    #[test]
    fn remove_to_empty() {
        let big_value = b"00000000000000000000000000000000";

        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t1 = TrieDBMut::new(&mut memdb, &mut root);
        t1.insert(&[0x01, 0x23], big_value).unwrap();
        t1.insert(&[0x01, 0x34], big_value).unwrap();
        let mut memdb2 = MemoryDB::new();
        let mut root2 = H256::new();
        let mut t2 = TrieDBMut::new(&mut memdb2, &mut root2);
        t2.insert(&[0x01], big_value).unwrap();
        t2.insert(&[0x01, 0x23], big_value).unwrap();
        t2.insert(&[0x01, 0x34], big_value).unwrap();
        t2.remove(&[0x01]).unwrap();
    }

    #[test]
    fn insert_replace_root() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        t.insert(&[0x01u8, 0x23], &[0x23u8, 0x45]).unwrap();
        assert_eq!(*t.root(), trie_root(vec![(vec![0x01u8, 0x23], vec![0x23u8, 0x45])]));
    }

    #[test]
    fn insert_make_branch_root() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        t.insert(&[0x11u8, 0x23], &[0x11u8, 0x23]).unwrap();
        assert_eq!(
            *t.root(),
            trie_root(vec![(vec![0x01u8, 0x23], vec![0x01u8, 0x23]), (vec![0x11u8, 0x23], vec![0x11u8, 0x23])])
        );
    }

    #[test]
    fn insert_into_branch_root() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        t.insert(&[0xf1u8, 0x23], &[0xf1u8, 0x23]).unwrap();
        t.insert(&[0x81u8, 0x23], &[0x81u8, 0x23]).unwrap();
        assert_eq!(
            *t.root(),
            trie_root(vec![
                (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
                (vec![0x81u8, 0x23], vec![0x81u8, 0x23]),
                (vec![0xf1u8, 0x23], vec![0xf1u8, 0x23]),
            ])
        );
    }

    #[test]
    fn insert_value_into_branch_root() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        t.insert(&[], &[0x0]).unwrap();
        assert_eq!(*t.root(), trie_root(vec![(vec![], vec![0x0]), (vec![0x01u8, 0x23], vec![0x01u8, 0x23])]));
    }

    #[test]
    fn insert_split_leaf() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        t.insert(&[0x01u8, 0x34], &[0x01u8, 0x34]).unwrap();
        assert_eq!(
            *t.root(),
            trie_root(vec![(vec![0x01u8, 0x23], vec![0x01u8, 0x23]), (vec![0x01u8, 0x34], vec![0x01u8, 0x34])])
        );
    }

    #[test]
    fn insert_split_extenstion() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01, 0x23, 0x45], &[0x01]).unwrap();
        t.insert(&[0x01, 0xf3, 0x45], &[0x02]).unwrap();
        t.insert(&[0x01, 0xf3, 0xf5], &[0x03]).unwrap();
        assert_eq!(
            *t.root(),
            trie_root(vec![
                (vec![0x01, 0x23, 0x45], vec![0x01]),
                (vec![0x01, 0xf3, 0x45], vec![0x02]),
                (vec![0x01, 0xf3, 0xf5], vec![0x03]),
            ])
        );
    }

    #[test]
    fn insert_big_value() {
        let big_value0 = b"00000000000000000000000000000000";
        let big_value1 = b"11111111111111111111111111111111";

        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], big_value0).unwrap();
        t.insert(&[0x11u8, 0x23], big_value1).unwrap();
        assert_eq!(
            *t.root(),
            trie_root(vec![(vec![0x01u8, 0x23], big_value0.to_vec()), (vec![0x11u8, 0x23], big_value1.to_vec())])
        );
    }

    #[test]
    fn insert_duplicate_value() {
        let big_value = b"00000000000000000000000000000000";

        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], big_value).unwrap();
        t.insert(&[0x11u8, 0x23], big_value).unwrap();
        assert_eq!(
            *t.root(),
            trie_root(vec![(vec![0x01u8, 0x23], big_value.to_vec()), (vec![0x11u8, 0x23], big_value.to_vec())])
        );
    }

    #[test]
    fn empty() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let t = TrieDBMut::new(&mut memdb, &mut root);
        assert_eq!(t.get(&[0x5]), Ok(None));
    }

    #[test]
    fn one() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        assert_eq!(t.get(&[0x1, 0x23]).unwrap().unwrap(), DBValue::from_slice(&[0x1u8, 0x23]));

        assert_eq!(t.get(&[0x1, 0x23]).unwrap().unwrap(), DBValue::from_slice(&[0x1u8, 0x23]));
    }

    #[test]
    fn three() {
        let mut memdb = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut memdb, &mut root);
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        t.insert(&[0xf1u8, 0x23], &[0xf1u8, 0x23]).unwrap();
        t.insert(&[0x81u8, 0x23], &[0x81u8, 0x23]).unwrap();
        assert_eq!(t.get(&[0x01, 0x23]).unwrap().unwrap(), DBValue::from_slice(&[0x01u8, 0x23]));
        assert_eq!(t.get(&[0xf1, 0x23]).unwrap().unwrap(), DBValue::from_slice(&[0xf1u8, 0x23]));
        assert_eq!(t.get(&[0x81, 0x23]).unwrap().unwrap(), DBValue::from_slice(&[0x81u8, 0x23]));
        assert_eq!(t.get(&[0x82, 0x23]), Ok(None));

        assert_eq!(t.get(&[0x01, 0x23]).unwrap().unwrap(), DBValue::from_slice(&[0x01u8, 0x23]));
        assert_eq!(t.get(&[0xf1, 0x23]).unwrap().unwrap(), DBValue::from_slice(&[0xf1u8, 0x23]));
        assert_eq!(t.get(&[0x81, 0x23]).unwrap().unwrap(), DBValue::from_slice(&[0x81u8, 0x23]));
        assert_eq!(t.get(&[0x82, 0x23]), Ok(None));
    }

    #[test]
    fn stress() {
        let mut seed = H256::new();
        for _ in 0..50 {
            let x = StandardMap {
                alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
                min_key: 5,
                journal_key: 0,
                value_mode: ValueMode::Index,
                count: 4,
            }
            .make_with(&mut seed);

            let real = trie_root(x.clone());
            let mut memdb = MemoryDB::new();
            let mut root = H256::new();
            let mut memtrie = populate_trie(&mut memdb, &mut root, &x);
            let mut y = x.clone();
            y.sort_by(|ref a, ref b| a.0.cmp(&b.0));
            let mut memdb2 = MemoryDB::new();
            let mut root2 = H256::new();
            let mut memtrie_sorted = populate_trie(&mut memdb2, &mut root2, &y);
            if *memtrie.root() != real || *memtrie_sorted.root() != real {
                println!("TRIE MISMATCH");
                println!();
                println!("ORIGINAL... {:?}", memtrie.root());
                for i in &x {
                    println!("{:?} -> {:?}", i.0.pretty(), i.1.pretty());
                }
                println!("SORTED... {:?}", memtrie_sorted.root());
                for i in &y {
                    println!("{:?} -> {:?}", i.0.pretty(), i.1.pretty());
                }
            }
            assert_eq!(*memtrie.root(), real);
            assert_eq!(*memtrie_sorted.root(), real);
        }
    }

    #[test]
    fn trie_existing() {
        let mut root = H256::new();
        let mut db = MemoryDB::new();
        {
            let mut t = TrieDBMut::new(&mut db, &mut root);
            t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
        }

        {
            let _ = TrieDBMut::from_existing(&mut db, &mut root);
        }
    }

    #[test]
    fn from_null_rlp_succeeds() {
        let mut root = BLAKE_NULL_RLP;
        let mut db = MemoryDB::new();
        TrieDBMut::from_existing(&mut db, &mut root).unwrap();
    }

    #[test]
    #[should_panic]
    fn from_zero_fails() {
        let mut root = H256::zero();
        let mut db = MemoryDB::new();
        TrieDBMut::from_existing(&mut db, &mut root).unwrap();
    }

    #[test]
    #[ignore]
    fn insert_empty() {
        let mut seed = H256::new();
        let x = StandardMap {
            alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
            min_key: 5,
            journal_key: 0,
            value_mode: ValueMode::Index,
            count: 4,
        }
        .make_with(&mut seed);

        let mut db = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut db, &mut root);
        for &(ref key, ref value) in &x {
            t.insert(key, value).unwrap();
        }

        assert_eq!(*t.root(), trie_root(x.clone()));

        for &(ref key, _) in &x {
            t.insert(key, &[]).unwrap();
        }

        assert!(t.is_empty());
        assert_eq!(*t.root(), BLAKE_NULL_RLP);
    }

    #[test]
    fn return_old_values() {
        let mut seed = H256::new();
        let x = StandardMap {
            alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
            min_key: 5,
            journal_key: 0,
            value_mode: ValueMode::Index,
            count: 4,
        }
        .make_with(&mut seed);

        let mut db = MemoryDB::new();
        let mut root = H256::new();
        let mut t = TrieDBMut::new(&mut db, &mut root);
        for &(ref key, ref value) in &x {
            assert!(t.insert(key, value).unwrap().is_none());
            assert_eq!(t.insert(key, value).unwrap(), Some(DBValue::from_slice(value)));
        }

        for (key, value) in x {
            assert_eq!(t.remove(&key).unwrap(), Some(DBValue::from_slice(&value)));
            assert!(t.remove(&key).unwrap().is_none());
        }
    }
}
