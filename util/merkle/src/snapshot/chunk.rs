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

use std::collections::VecDeque;
use std::convert::From;

use ccrypto::BLAKE_NULL_RLP;
use hashdb::{DBValue, HashDB};
use primitives::H256;

use super::error::{ChunkError, Error};
use super::{DecodedPathSlice, PathSlice, CHUNK_HEIGHT};
use crate::nibbleslice::NibbleSlice;
use crate::{Node, TrieDBMut};

#[derive(RlpEncodable, RlpDecodable, Eq, PartialEq)]
pub struct TerminalNode {
    // Relative path from the chunk root.
    pub path_slice: PathSlice,
    pub node_rlp: Vec<u8>,
}

impl std::fmt::Debug for TerminalNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let path_slice = NibbleSlice::from_encoded(&self.path_slice);
        f.debug_struct("TerminalNode")
            .field("path_slice", &path_slice)
            .field("node_rlp", &NodeDebugAdaptor {
                rlp: &self.node_rlp,
            })
            .finish()
    }
}

struct NodeDebugAdaptor<'a> {
    rlp: &'a [u8],
}

impl<'a> std::fmt::Debug for NodeDebugAdaptor<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match Node::decoded(&self.rlp) {
            Some(node) => write!(f, "{:?}", &node),
            None => write!(f, "{:?}", self.rlp),
        }
    }
}

/// An unverified chunk from the network
#[derive(Debug)]
pub struct RawChunk {
    pub nodes: Vec<TerminalNode>,
}

/// Fully recovered, and re-hydrated chunk.
pub struct RecoveredChunk {
    pub(crate) root: H256,
    /// contains all nodes including non-terminal nodes and terminal nodes.
    /// You can blindly pour all items in `nodes` into `HashDB`.
    pub(crate) nodes: Vec<(H256, DBValue)>,
    /// Their path slices are relative to this chunk root.
    pub(crate) unresolved_chunks: Vec<UnresolvedChunk>,
}

impl RawChunk {
    /// Verify and recover the chunk
    pub fn recover(&self, expected_chunk_root: H256) -> Result<RecoveredChunk, Error> {
        let mut memorydb = memorydb::MemoryDB::new();
        let mut chunk_root = H256::new();

        {
            let mut trie = TrieDBMut::new(&mut memorydb, &mut chunk_root);
            for node in self.nodes.iter() {
                let old_val = match Node::decoded(&node.node_rlp) {
                    Some(Node::Branch(slice, child)) => {
                        let encoded = DecodedPathSlice::from_encoded(&node.path_slice).with_slice(slice).encode();
                        trie.insert_raw(Node::Branch(NibbleSlice::from_encoded(&encoded), child))?
                    }
                    Some(Node::Leaf(slice, data)) => {
                        let encoded = DecodedPathSlice::from_encoded(&node.path_slice).with_slice(slice).encode();
                        trie.insert_raw(Node::Leaf(NibbleSlice::from_encoded(&encoded), data))?
                    }
                    None => return Err(ChunkError::InvalidContent.into()),
                };

                if let Some(old_val) = old_val {
                    if old_val.as_ref() != node.node_rlp.as_slice() {
                        return Err(ChunkError::InvalidContent.into())
                    }
                }
            }
        }

        // Some nodes in the chunk is different from the expected.
        if chunk_root != expected_chunk_root {
            return Err(ChunkError::ChunkRootMismatch {
                expected: expected_chunk_root,
                actual: chunk_root,
            }
            .into())
        }

        let mut nodes = Vec::new();
        let mut unresolved_chunks = Vec::new();
        let mut queue: VecDeque<NodePath> = VecDeque::from(vec![NodePath::new(chunk_root)]);
        while let Some(path) = queue.pop_front() {
            let node = match memorydb.get(&path.key) {
                Some(x) => x,
                None => {
                    // all unresolved should depth == CHUNK_HEIGHT + 1
                    if path.depth != CHUNK_HEIGHT + 1 {
                        return Err(ChunkError::InvalidHeight.into())
                    }

                    unresolved_chunks.push(UnresolvedChunk::from(path));
                    continue
                }
            };

            if path.depth > CHUNK_HEIGHT {
                return Err(ChunkError::InvalidHeight.into())
            }
            nodes.push((path.key, node.clone()));

            let node = Node::decoded(&node).expect("Chunk root was verified; Node can't be wrong");
            if let Node::Branch(slice, children) = node {
                for (index, child) in children.iter().enumerate() {
                    if let Some(child) = child {
                        queue.push_back(path.with_slice_and_index(slice, index, *child));
                    }
                }
            }
        }

        Ok(RecoveredChunk {
            root: expected_chunk_root,
            nodes,
            unresolved_chunks,
        })
    }
}

impl std::fmt::Debug for RecoveredChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        struct Adapter<'a>(&'a [(H256, DBValue)]);
        impl<'a> std::fmt::Debug for Adapter<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                f.debug_list()
                    .entries(self.0.iter().map(|(hash, rlp)| {
                        (hash, NodeDebugAdaptor {
                            rlp,
                        })
                    }))
                    .finish()
            }
        }

        f.debug_struct("RecoveredChunk")
            .field("root", &self.root)
            .field("nodes", &Adapter(&self.nodes))
            .field("unresolved_chunks", &self.unresolved_chunks)
            .finish()
    }
}

/// Chunk obtained from the state db.
#[derive(Debug)]
pub struct Chunk {
    pub root: H256,
    pub terminal_nodes: Vec<TerminalNode>,
}

impl Chunk {
    pub(crate) fn from_chunk_root(db: &dyn HashDB, chunk_root: H256) -> Chunk {
        let mut unresolved: VecDeque<NodePath> = VecDeque::from(vec![NodePath::new(chunk_root)]);
        let mut terminal_nodes: Vec<TerminalNode> = Vec::new();
        while let Some(path) = unresolved.pop_front() {
            assert!(path.key != BLAKE_NULL_RLP, "Empty DB");
            assert!(path.depth <= CHUNK_HEIGHT);
            let node = db.get(&path.key).expect("Can't find the node in a db. DB is inconsistent");
            let node_decoded = Node::decoded(&node).expect("Node cannot be decoded. DB is inconsistent");

            match node_decoded {
                // Continue to BFS
                Node::Branch(slice, ref children) if path.depth < CHUNK_HEIGHT => {
                    for (i, hash) in children.iter().enumerate() {
                        if let Some(hash) = hash {
                            unresolved.push_back(path.with_slice_and_index(slice, i, *hash));
                        }
                    }
                }
                // Reached the terminal node. Branch at path.depth == CHUNK_HEIGHT || Leaf
                _ => terminal_nodes.push(TerminalNode {
                    path_slice: path.path_slice.encode(),
                    node_rlp: node.to_vec(),
                }),
            };
        }
        Chunk {
            root: chunk_root,
            terminal_nodes,
        }
    }

    // Returns path slices to unresolved chunk roots relative to this chunk root
    pub(crate) fn unresolved_chunks(&self) -> Vec<UnresolvedChunk> {
        let mut result = Vec::new();
        for node in self.terminal_nodes.iter() {
            let decoded = Node::decoded(&node.node_rlp).expect("All terminal nodes should be valid");
            if let Node::Branch(slice, children) = decoded {
                for (i, child) in children.iter().enumerate() {
                    if let Some(child) = child {
                        result.push(UnresolvedChunk {
                            path_slice: DecodedPathSlice::from_encoded(&node.path_slice).with_slice_and_index(slice, i),
                            chunk_root: *child,
                        })
                    }
                }
            }
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn into_raw_chunk(self) -> RawChunk {
        RawChunk {
            nodes: self.terminal_nodes,
        }
    }
}

/// path slice to `chunk_root` is relative to the root of originating chunk.
#[derive(Debug)]
pub(crate) struct UnresolvedChunk {
    pub path_slice: DecodedPathSlice,
    pub chunk_root: H256,
}

impl From<NodePath> for UnresolvedChunk {
    fn from(path: NodePath) -> Self {
        Self {
            path_slice: path.path_slice,
            chunk_root: path.key,
        }
    }
}

#[derive(Debug)]
struct NodePath {
    // path slice to the node relative to chunk_root
    path_slice: DecodedPathSlice,
    depth: usize,
    key: H256,
}

impl NodePath {
    fn new(key: H256) -> NodePath {
        NodePath {
            path_slice: DecodedPathSlice::new(),
            depth: 1,
            key,
        }
    }

    fn with_slice_and_index(&self, slice: NibbleSlice, index: usize, key: H256) -> NodePath {
        NodePath {
            path_slice: self.path_slice.with_slice_and_index(slice, index),
            depth: self.depth + 1,
            key,
        }
    }
}
