// Copyright 2019 Kodebox, Inc.
// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of CodeChain.
//
// This is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! `JournalDB` interface and implementation.

mod algorithm;
mod archivedb;
mod traits;

use std::sync::Arc;

pub use self::algorithm::Algorithm;
pub use self::traits::JournalDB;

/// Create a new `JournalDB` trait object over a generic key-value database.
pub fn new_journaldb(backing: Arc<dyn kvdb::KeyValueDB>, algorithm: Algorithm, col: Option<u32>) -> Box<dyn JournalDB> {
    match algorithm {
        Algorithm::Archive => Box::new(archivedb::ArchiveDB::new(backing, col)),
    }
}

// all keys must be at least 12 bytes
const DB_PREFIX_LEN: usize = ::kvdb::PREFIX_LEN;
const LATEST_ERA_KEY: [u8; ::kvdb::PREFIX_LEN] = [b'l', b'a', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0];
