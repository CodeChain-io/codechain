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

use std::collections::HashMap;

use ckey::Public;
use kvdb::{DBTransaction, KeyValueDB};
use primitives::H256;
use rlp::Encodable;

use super::mem_pool_types::{MemPoolItem, PoolingInstant};
use crate::db as dblib;

const PREFIX_SIZE: usize = 5;
const PREFIX_ITEM: &[u8; PREFIX_SIZE] = b"item_";
const PREFIX_FIRST_SEQS: &[u8; PREFIX_SIZE] = b"seqs1";
const PREFIX_NEXT_SEQS: &[u8; PREFIX_SIZE] = b"seqs2";
const FIELD_COUNT: usize = 6;

pub struct RecoveredData {
    pub by_hash: HashMap<H256, MemPoolItem>,
    pub first_seqs: HashMap<Public, u64>,
    pub next_seqs: HashMap<Public, u64>,
    pub last_time: PoolingInstant,
    pub last_timestamp: u64,
    pub next_transaction_id: u64,
}

pub fn backup_batch_with_capacity(length : usize) -> DBTransaction {
    DBTransaction::with_capacity(length * FIELD_COUNT)
}

pub fn backup_item(batch: &mut DBTransaction, key: H256, item: &MemPoolItem) {
    let mut db_key = PREFIX_ITEM.to_vec();
    db_key.extend_from_slice(key.as_ref());
    batch.put(dblib::COL_MEMPOOL, db_key.as_ref(), item.rlp_bytes().as_ref());
}

/// Backup first sequence
pub fn backup_seqs(batch: &mut DBTransaction, key: Public, value: u64, is_first: bool) {
    let mut db_key = if is_first {
        PREFIX_FIRST_SEQS.to_vec()
    } else {
        PREFIX_NEXT_SEQS.to_vec()
    };
    db_key.extend_from_slice(key.as_ref());
    batch.put(dblib::COL_MEMPOOL, db_key.as_ref(), value.rlp_bytes().as_ref());
}

pub fn backup_extra(batch: &mut DBTransaction, key: &[u8], val: u64) {
    batch.put(dblib::COL_MEMPOOL, key, val.rlp_bytes().as_ref());
}

pub fn remove_item(batch: &mut DBTransaction, key: &H256) {
    let mut db_key = PREFIX_ITEM.to_vec();
    db_key.extend_from_slice(key.as_ref());
    batch.delete(dblib::COL_MEMPOOL, db_key.as_ref());
}

pub fn recover_to_data(db: &KeyValueDB) -> RecoveredData {
    let mut by_hash = HashMap::new();
    let mut first_seqs = HashMap::new();
    let mut next_seqs = HashMap::new();

    for (key, value) in db.iter(dblib::COL_MEMPOOL) {
        let key_prefix: &[u8] = &key.as_ref()[0..PREFIX_SIZE];
        let bytes = (*value).to_vec();
        let rlp = rlp::Rlp::new(&bytes);

        if PREFIX_ITEM == key_prefix {
            let decoded_key = (key.as_ref()[PREFIX_SIZE..]).into();
            let decoded_item = rlp.as_val();
            by_hash.insert(decoded_key, decoded_item);
        } else if PREFIX_FIRST_SEQS == key_prefix {
            let decoded_key = (key.as_ref()[PREFIX_SIZE..]).into();
            let decoded_item = rlp.as_val();
            first_seqs.insert(decoded_key, decoded_item);
        } else if PREFIX_NEXT_SEQS == key_prefix {
            let decoded_key = (key.as_ref()[PREFIX_SIZE..]).into();
            let decoded_item = rlp.as_val();
            next_seqs.insert(decoded_key, decoded_item);
        }
    }

    let last_time = recover_extra_or_default(db, dblib::COL_MEMPOOL, b"last_time");
    let last_timestamp = recover_extra_or_default(db, dblib::COL_MEMPOOL, b"last_timestamp");
    let next_transaction_id = recover_extra_or_default(db, dblib::COL_MEMPOOL, b"next_transaction_id");

    RecoveredData {
        by_hash,
        first_seqs,
        next_seqs,
        last_time,
        last_timestamp,
        next_transaction_id,
    }
}

pub fn recover_extra_or_default(db: &KeyValueDB, col: Option<u32>, key: &[u8]) -> u64 {
    db.get(col, key).expect("Low Level Database Error").map(|dbval| rlp::decode(&dbval)).unwrap_or_default()
}
