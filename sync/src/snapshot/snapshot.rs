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

use std::convert::AsRef;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::iter::once;
use std::path::{Path, PathBuf};

use ccore::COL_STATE;
use cmerkle::Node;
use kvdb::KeyValueDB;
use primitives::H256;
use rlp::RlpStream;
use snap;

use super::error::Error;

pub trait WriteSnapshot {
    fn write_snapshot(&self, db: &KeyValueDB, root: &H256) -> Result<(), Error>;
}

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
    fn write_nodes<'a, I>(&self, root: &H256, iter: I) -> Result<(), Error>
    where
        I: IntoIterator<Item = &'a (H256, Vec<u8>)>, {
        let file = File::create(self.path.join(format!("{:x}", root)))?;
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
