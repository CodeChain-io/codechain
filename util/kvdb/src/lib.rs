// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Key-Value store abstraction with `RocksDB` backend.

#[macro_use]
extern crate error_chain;
extern crate elastic_array;
extern crate primitives;

use elastic_array::{ElasticArray128, ElasticArray32};
use primitives::Bytes;

/// Required length of prefixes.
pub const PREFIX_LEN: usize = 12;

/// Database value.
pub type DBValue = ElasticArray128<u8>;

#[allow(deprecated)]
mod errors {
    use std::io;

    error_chain! {
        types {
            Error, ErrorKind, ResultExt, Result;
        }

        foreign_links {
            Io(io::Error);
        }
    }
}

pub use self::errors::{Error, ErrorKind, Result, ResultExt};

/// Write parcel. Batches a sequence of put/delete operations for efficiency.
#[derive(Default, Clone, PartialEq)]
pub struct DBTransaction {
    /// Database operations.
    pub ops: Vec<DBOp>,
}

/// Database operation.
#[derive(Clone, PartialEq)]
pub enum DBOp {
    Insert {
        col: Option<u32>,
        key: ElasticArray32<u8>,
        value: DBValue,
    },
    Delete {
        col: Option<u32>,
        key: ElasticArray32<u8>,
    },
}

impl DBTransaction {
    /// Create new parcel.
    pub fn new() -> DBTransaction {
        DBTransaction::with_capacity(256)
    }

    /// Create new parcel with capacity.
    pub fn with_capacity(cap: usize) -> DBTransaction {
        DBTransaction {
            ops: Vec::with_capacity(cap),
        }
    }

    /// Insert a key-value pair in the parcel. Any existing value will be overwritten upon write.
    pub fn put(&mut self, col: Option<u32>, key: &[u8], value: &[u8]) {
        let mut ekey = ElasticArray32::new();
        ekey.append_slice(key);
        self.ops.push(DBOp::Insert {
            col,
            key: ekey,
            value: DBValue::from_slice(value),
        });
    }

    /// Insert a key-value pair in the parcel. Any existing value will be overwritten upon write.
    pub fn put_vec(&mut self, col: Option<u32>, key: &[u8], value: Bytes) {
        let mut ekey = ElasticArray32::new();
        ekey.append_slice(key);
        self.ops.push(DBOp::Insert {
            col,
            key: ekey,
            value: DBValue::from_vec(value),
        });
    }

    /// Delete value by key.
    pub fn delete(&mut self, col: Option<u32>, key: &[u8]) {
        let mut ekey = ElasticArray32::new();
        ekey.append_slice(key);
        self.ops.push(DBOp::Delete {
            col,
            key: ekey,
        });
    }
}

/// Generic key-value database.
///
/// This makes a distinction between "buffered" and "flushed" values. Values which have been
/// written can always be read, but may be present in an in-memory buffer. Values which have
/// been flushed have been moved to backing storage, like a RocksDB instance. There are certain
/// operations which are only guaranteed to operate on flushed data and not buffered,
/// although implementations may differ in this regard.
///
/// The contents of an interior buffer may be explicitly flushed using the `flush` method.
///
/// The `KeyValueDB` also deals in "column families", which can be thought of as distinct
/// stores within a database. Keys written in one column family will not be accessible from
/// any other. The number of column families must be specified at initialization, with a
/// differing interface for each database. The `None` argument in place of a column index
/// is always supported.
///
/// The API laid out here, along with the `Sync` bound implies interior synchronization for
/// implementation.
pub trait KeyValueDB: Sync + Send {
    /// Helper to create a new parcel.
    fn transaction(&self) -> DBTransaction {
        DBTransaction::new()
    }

    /// Get a value by key.
    fn get(&self, col: Option<u32>, key: &[u8]) -> Result<Option<DBValue>>;

    /// Get a value by partial key. Only works for flushed data.
    fn get_by_prefix(&self, col: Option<u32>, prefix: &[u8]) -> Option<Box<[u8]>>;

    /// Write a parcel of changes to the buffer.
    fn write_buffered(&self, transaction: DBTransaction);

    /// Write a parcel of changes to the backing store.
    fn write(&self, transaction: DBTransaction) -> Result<()> {
        self.write_buffered(transaction);
        self.flush()
    }

    /// Flush all buffered data.
    fn flush(&self) -> Result<()>;

    /// Iterate over flushed data for a given column.
    fn iter(&self, col: Option<u32>) -> KeyValueDBIterator;

    /// Iterate over flushed data for a given column, starting from a given prefix.
    fn iter_from_prefix<'a>(&'a self, col: Option<u32>, prefix: &'a [u8]) -> KeyValueDBIterator;

    /// Attempt to replace this database with a new one located at the given path.
    fn restore(&self, new_db: &str) -> Result<()>;
}

pub type KeyValueDBIterator<'a> = Box<dyn Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a>;
