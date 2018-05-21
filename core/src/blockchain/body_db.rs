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

use std::collections::HashMap;
use std::sync::Arc;

use cbytes::Bytes;
use ctypes::H256;
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;
use rlp::RlpStream;
use rlp_compress::{blocks_swapper, compress, decompress};

use super::super::db;
use super::super::encoded;
use super::super::views::BlockView;

pub struct BodyDB {
    // block cache
    body_cache: RwLock<HashMap<H256, Bytes>>,

    db: Arc<KeyValueDB>,
}

impl BodyDB {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &BlockView, db: Arc<KeyValueDB>) -> Self {
        let bdb = Self {
            body_cache: RwLock::new(HashMap::new()),

            db,
        };

        let genesis_hash = genesis.hash();
        match bdb.block_body(&genesis_hash) {
            None => {
                let mut batch = DBTransaction::new();
                batch.put(db::COL_BODIES, &genesis_hash, &Self::block_to_body(genesis));

                bdb.db.write(batch).expect("Low level database error. Some issue with disk?");
            }
            _ => {}
        };

        bdb
    }

    /// Inserts the block body into backing cache database.
    /// Expects the body to be valid and already verified.
    /// If the body is already known, does nothing.
    pub fn insert_body(&self, batch: &mut DBTransaction, block: &BlockView) {
        let hash = block.hash();

        if self.is_known_body(&hash) {
            return
        }

        let compressed_body = compress(&Self::block_to_body(block), blocks_swapper());

        // store block in db
        batch.put(db::COL_BODIES, &hash, &compressed_body);
    }

    /// Create a block body from a block.
    pub fn block_to_body(block: &BlockView) -> Bytes {
        let mut body = RlpStream::new_list(1);
        body.append_raw(block.rlp().at(1).as_raw(), 1);
        body.out()
    }
}

/// Interface for querying block bodiess by hash and by number.
pub trait BodyProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known_body(&self, hash: &H256) -> bool;

    /// Get the block body (uncles and parcels).
    fn block_body(&self, hash: &H256) -> Option<encoded::Body>;
}

impl BodyProvider for BodyDB {
    fn is_known_body(&self, hash: &H256) -> bool {
        self.block_body(hash).is_some()
    }

    /// Get block body data
    fn block_body(&self, hash: &H256) -> Option<encoded::Body> {
        // Check cache first
        {
            let read = self.body_cache.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Body::new(v.clone()))
            }
        }

        // Read from DB and populate cache
        let compressed_body =
            self.db.get(db::COL_BODIES, hash).expect("Low level database error. Some issue with disk?")?;

        let raw_body = decompress(&compressed_body, blocks_swapper()).into_vec();
        let mut write = self.body_cache.write();
        write.insert(*hash, raw_body.clone());

        Some(encoded::Body::new(raw_body))
    }
}
