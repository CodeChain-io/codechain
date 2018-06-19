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

use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::spawn;

use ccore::{BlockChainClient, BlockId, BlockInfo, ChainInfo, ChainNotify, Client, COL_STATE};
use ctypes::H256;

use kvdb::KeyValueDB;
use rlp::{decode as rlp_decode, RlpStream};
use trie::{Node, OwnedNode};

use super::error::Error;

pub struct Service {
    client: Arc<Client>,
    /// Snapshot root directory
    root_dir: String,
    /// Snapshot creation period in unit of block numbers
    period: u64,
}

impl Service {
    pub fn new(client: Arc<Client>, root_dir: String, period: u64) -> Arc<Self> {
        Arc::new(Self {
            client,
            root_dir,
            period,
        })
    }
}

impl ChainNotify for Service {
    /// fires when chain has new blocks.
    fn new_blocks(
        &self,
        _imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
        let best_number = self.client.chain_info().best_block_number;
        let is_checkpoint = enacted
            .iter()
            .map(|hash| self.client.block_number(BlockId::Hash(*hash)).expect("Enacted block must exist"))
            .any(|number| number % self.period == 0);
        if is_checkpoint && best_number > self.period {
            let number = (best_number / self.period - 1) * self.period;
            let header = self.client.block_header(BlockId::Number(number)).expect("Snapshot target must exist");

            let db = self.client.database();
            let path: PathBuf = [self.root_dir.clone(), format!("{:x}", header.hash())].iter().collect();
            let root = header.state_root();
            spawn(move || match write_snapshot(db, path, root) {
                Ok(_) => {}
                Err(_) => {}
            });
        }
    }
}

fn write_snapshot(db: Arc<KeyValueDB>, path: PathBuf, root: H256) -> Result<(), Error> {
    create_dir_all(&path)?;

    let root_val = get_node(&db, &root)?;
    let children = children_of(&db, &root_val)?;
    let mut grandchildren = Vec::new();
    for (_, value) in &children {
        grandchildren.extend(children_of(&db, value)?);
    }

    {
        let mut file = File::create(path.join("head"))?;

        let mut stream = RlpStream::new();
        stream.begin_unbounded_list();
        for (key, value) in vec![(root, root_val)].iter().chain(&grandchildren).chain(&children) {
            stream.begin_list(2);
            stream.append(key);
            stream.append(value);
        }
        stream.complete_unbounded_list();

        file.write(&stream.drain())?;
    }

    for (grandchild, _) in &grandchildren {
        let nodes = enumerate_subtree(&db, grandchild)?;
        let mut file = File::create(path.join(format!("{:x}", grandchild)))?;
        let mut stream = RlpStream::new();
        stream.begin_unbounded_list();
        for (key, value) in nodes {
            stream.begin_list(2);
            stream.append(&key);
            stream.append(&value);
        }
        stream.complete_unbounded_list();
        file.write(&stream.drain())?;
    }

    Ok(())
}

fn get_node(db: &Arc<KeyValueDB>, key: &H256) -> Result<Vec<u8>, Error> {
    match db.get(COL_STATE, key) {
        Ok(Some(value)) => Ok(value.to_vec()),
        Ok(None) => Err(Error::NodeNotFound(*key)),
        Err(e) => Err(Error::DBError(e)),
    }
}

fn children_of(db: &Arc<KeyValueDB>, node: &[u8]) -> Result<Vec<(H256, Vec<u8>)>, Error> {
    let keys = match OwnedNode::from(Node::decoded(node)) {
        OwnedNode::Empty => Vec::new(),
        OwnedNode::Leaf(..) => Vec::new(),
        OwnedNode::Extension(_, child) => vec![H256::from_slice(&child)],
        OwnedNode::Branch(children, _) => children
            .iter()
            .filter_map(|child| {
                let decoded: Vec<u8> = rlp_decode(child);
                if decoded.len() != 0 {
                    Some(H256::from_slice(&decoded))
                } else {
                    None
                }
            })
            .collect(),
    };
    let mut result = Vec::new();
    for key in keys {
        result.push((key, get_node(db, &key)?));
    }
    Ok(result)
}

fn enumerate_subtree(db: &Arc<KeyValueDB>, root: &H256) -> Result<Vec<(H256, Vec<u8>)>, Error> {
    let node = get_node(db, root)?;
    let children = match OwnedNode::from(Node::decoded(&node)) {
        OwnedNode::Empty => Vec::new(),
        OwnedNode::Leaf(..) => Vec::new(),
        OwnedNode::Extension(_, child) => vec![H256::from_slice(&child)],
        OwnedNode::Branch(children, _) => children
            .iter()
            .filter_map(|child| {
                let decoded: Vec<u8> = rlp_decode(child);
                if decoded.len() != 0 {
                    Some(H256::from_slice(&decoded))
                } else {
                    None
                }
            })
            .collect(),
    };
    let mut result: Vec<_> = vec![(*root, node)];
    for child in children {
        result.extend(enumerate_subtree(db, &child)?);
    }
    Ok(result)
}
