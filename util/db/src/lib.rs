// Copyright 2019 Kodebox, Inc.
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

extern crate kvdb;
extern crate primitives;
extern crate rlp;
extern crate util_error as error;

#[cfg(test)]
extern crate codechain_crypto as crypto;
#[cfg(test)]
extern crate kvdb_memorydb;

mod hashdb;
mod journaldb;
mod memorydb;

pub use crate::hashdb::{AsHashDB, DBValue, HashDB};
pub use crate::journaldb::{new_journaldb, Algorithm, JournalDB};
pub use crate::memorydb::MemoryDB;
