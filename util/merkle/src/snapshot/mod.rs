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

pub mod chunk;
mod compress;
mod error;
mod ordered_heap;

use std::cmp::Ordering;

use ccrypto::BLAKE_NULL_RLP;
use hashdb::HashDB;
use primitives::H256;

use self::chunk::{Chunk, RecoveredChunk, UnresolvedChunk};
use self::ordered_heap::OrderedHeap;
use crate::nibbleslice::NibbleSlice;

const CHUNK_HEIGHT: usize = 3;
const CHUNK_MAX_NODES: usize = 256; // 16 ^ (CHUNK_HEIGHT-1)

/// Example:
/// use codechain_merkle::snapshot::Restore;
/// let mut rm = Restore::new(db, root);
/// while let Some(root) = rm.next_to_feed() {
///     let raw_chunk = request(block_hash, root)?;
///     let chunk = raw_chunk.recover(root)?;
///     rm.feed(chunk);
/// }
pub struct Restore<'a> {
    db: &'a mut dyn HashDB,
    pending: Option<ChunkPathPrefix>,
    unresolved: OrderedHeap<DepthFirst<ChunkPathPrefix>>,
}

impl<'a> Restore<'a> {
    pub fn new(db: &'a mut dyn HashDB, merkle_root: H256) -> Self {
        let mut result = Restore {
            db,
            pending: None,
            unresolved: OrderedHeap::new(),
        };
        if merkle_root != BLAKE_NULL_RLP {
            result.unresolved.push(ChunkPathPrefix::new(merkle_root).into());
        }
        result
    }

    pub fn feed(&mut self, chunk: RecoveredChunk) {
        let pending_path = self.pending.take().expect("feed() should be called after next()");
        assert_eq!(pending_path.chunk_root, chunk.root, "Unexpected chunk");

        // Pour nodes into the DB
        for (key, value) in chunk.nodes {
            self.db.emplace(key, value);
        }

        // Extend search paths
        for unresolved in chunk.unresolved_chunks {
            self.unresolved.push(pending_path.with_unresolved_chunk(&unresolved).into());
        }

        self.pending = None;
    }

    pub fn next_to_feed(&mut self) -> Option<H256> {
        if let Some(path) = self.unresolved.pop() {
            assert!(self.pending.is_none(), "Previous feed() was failed");
            let chunk_root = path.chunk_root;
            self.pending = Some(path.0);

            Some(chunk_root)
        } else {
            None
        }
    }
}

/// Example:
/// use std::fs::File;
/// use codechain_merkle::snapshot::Snapshot;
///
/// for chunk in Snapshot::from_hashdb(db, root) {
///     let mut file = File::create(format!("{}/{}", block_id, chunk.root))?;
///     let mut compressor = ChunkCompressor::new(&mut file);
///     compressor.compress(chunk);
/// }
pub struct Snapshot<'a> {
    db: &'a dyn HashDB,
    remaining: OrderedHeap<DepthFirst<ChunkPathPrefix>>,
}

impl<'a> Snapshot<'a> {
    pub fn from_hashdb(db: &'a dyn HashDB, chunk_root: H256) -> Self {
        let mut result = Snapshot {
            db,
            remaining: OrderedHeap::new(),
        };
        if chunk_root != BLAKE_NULL_RLP {
            result.remaining.push(ChunkPathPrefix::new(chunk_root).into());
        }
        result
    }
}

impl<'a> Iterator for Snapshot<'a> {
    type Item = Chunk;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(path) = self.remaining.pop() {
            let chunk = Chunk::from_chunk_root(self.db, path.chunk_root);
            for unresolved in chunk.unresolved_chunks() {
                self.remaining.push(path.with_unresolved_chunk(&unresolved).into());
            }
            Some(chunk)
        } else {
            None
        }
    }
}


#[derive(Debug)]
struct ChunkPathPrefix {
    // Absolute path prefix of the chunk root
    path_prefix: DecodedPathSlice,
    depth: usize,
    chunk_root: H256,
}

impl ChunkPathPrefix {
    fn new(chunk_root: H256) -> ChunkPathPrefix {
        ChunkPathPrefix {
            path_prefix: DecodedPathSlice::new(),
            depth: 1,
            chunk_root,
        }
    }

    fn with_unresolved_chunk(&self, unresolved: &UnresolvedChunk) -> ChunkPathPrefix {
        ChunkPathPrefix {
            path_prefix: self.path_prefix.with_path_slice(&unresolved.path_slice),
            depth: self.depth + 1,
            chunk_root: unresolved.chunk_root,
        }
    }
}

impl Ord for DepthFirst<ChunkPathPrefix> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.depth.cmp(&other.0.depth)
    }
}

impl From<ChunkPathPrefix> for DepthFirst<ChunkPathPrefix> {
    fn from(path: ChunkPathPrefix) -> Self {
        DepthFirst(path)
    }
}

/// Encoded value by NibbleSlice::encoded()
pub type PathSlice = Vec<u8>;

/// for item i, i in 0..16
pub(crate) struct DecodedPathSlice(Vec<u8>);

impl DecodedPathSlice {
    fn new() -> DecodedPathSlice {
        DecodedPathSlice(Vec::new())
    }

    fn from_encoded(slice: &[u8]) -> DecodedPathSlice {
        DecodedPathSlice(NibbleSlice::from_encoded(slice).to_vec())
    }

    fn with_slice_and_index(&self, slice: NibbleSlice, i: usize) -> DecodedPathSlice {
        assert!(i < 16);
        let mut v = self.0.clone();
        v.append(&mut slice.to_vec());
        v.push(i as u8);
        DecodedPathSlice(v)
    }

    fn with_slice(&self, slice: NibbleSlice) -> DecodedPathSlice {
        let mut v = self.0.clone();
        v.append(&mut slice.to_vec());
        DecodedPathSlice(v)
    }

    fn with_path_slice(&self, path_slice: &DecodedPathSlice) -> DecodedPathSlice {
        let mut v = self.0.clone();
        v.extend(path_slice.0.as_slice());
        DecodedPathSlice(v)
    }

    fn encode(&self) -> PathSlice {
        let (encoded, _) = NibbleSlice::from_vec(&self.0);
        encoded.to_vec()
    }
}

impl std::fmt::Debug for DecodedPathSlice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let (encoded, _) = NibbleSlice::from_vec(&self.0);
        let nibble_slice = NibbleSlice::from_encoded(&encoded);
        writeln!(f, "{:?}", nibble_slice)
    }
}

#[derive(Debug)]
struct DepthFirst<T>(T);

impl<T> PartialOrd for DepthFirst<T>
where
    Self: Ord,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<T> PartialEq for DepthFirst<T>
where
    Self: Ord,
{
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T> Eq for DepthFirst<T> where Self: Ord {}

impl<T> std::ops::Deref for DepthFirst<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::iter::FromIterator;

    use hashdb::DBValue;
    use memorydb::MemoryDB;
    use primitives::{Bytes, H256};
    use standardmap::{Alphabet, StandardMap, ValueMode};

    use super::chunk::RawChunk;
    use crate::{Trie, TrieDB, TrieDBMut, TrieMut};

    fn random_insert_and_restore_with_count(count: usize) {
        let standard_map = StandardMap {
            alphabet: Alphabet::Custom(b"@QWERTYUIOPASDFGHJKLZXCVBNM[/]^_".to_vec()),
            min_key: 5,
            journal_key: 0,
            value_mode: ValueMode::Index,
            count,
        }
        .make_with(&mut H256::new());
        // Unique standard map
        let unique_map: HashMap<Bytes, Bytes> = HashMap::from_iter(standard_map.into_iter());

        let mut root = H256::new();
        let chunks: HashMap<H256, RawChunk> = {
            // We will throw out `db` after snapshot.
            let mut db = MemoryDB::new();
            let mut trie = TrieDBMut::new(&mut db, &mut root);
            for (key, value) in &unique_map {
                trie.insert(key, value).unwrap();
            }

            Snapshot::from_hashdb(&db, root).map(|chunk| (chunk.root, chunk.into_raw_chunk())).collect()
        };
        dbg!(chunks.len());

        let mut db = MemoryDB::new();
        let mut recover = Restore::new(&mut db, root);
        while let Some(chunk_root) = recover.next_to_feed() {
            let recovered = chunks[&chunk_root].recover(chunk_root).unwrap();
            recover.feed(recovered);
        }

        let trie = TrieDB::try_new(&db, &root).unwrap();
        for (key, value) in &unique_map {
            assert_eq!(trie.get(key).unwrap(), Some(DBValue::from_slice(value)));
        }
    }

    #[test]
    fn random_insert_and_restore_0() {
        random_insert_and_restore_with_count(0);
    }

    #[test]
    fn random_insert_and_restore_1() {
        random_insert_and_restore_with_count(1);
    }

    #[test]
    fn random_insert_and_restore_2() {
        random_insert_and_restore_with_count(2);
    }

    #[test]
    fn random_insert_and_restore_100() {
        random_insert_and_restore_with_count(100);
    }

    #[test]
    fn random_insert_and_restore_10000() {
        random_insert_and_restore_with_count(10_000);
    }

    #[test]
    #[ignore]
    fn random_insert_and_restore_100000() {
        random_insert_and_restore_with_count(100_000);
    }
}
