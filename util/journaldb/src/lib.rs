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

//! `JournalDB` interface and implementation.

extern crate heapsize;
#[macro_use]
extern crate log;

extern crate codechain_bytes as bytes;
extern crate codechain_types;
extern crate hashdb;
extern crate kvdb;
extern crate memorydb;
extern crate parking_lot;
extern crate plain_hasher;
extern crate rlp;
extern crate util_error as error;

#[cfg(test)]
extern crate codechain_crypto as crypto;
#[cfg(test)]
extern crate codechain_logger;
#[cfg(test)]
extern crate kvdb_memorydb;

use std::{fmt, str};
use std::sync::Arc;

/// Export the journaldb module.
mod traits;
mod archivedb;

/// Export the `JournalDB` trait.
pub use self::traits::JournalDB;

/// Journal database operating strategy.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Algorithm {
	/// Keep all keys forever.
	Archive,
}

impl Default for Algorithm {
	fn default() -> Algorithm { Algorithm::Archive }
}

impl str::FromStr for Algorithm {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"archive" => Ok(Algorithm::Archive),
			e => Err(format!("Invalid algorithm: {}", e)),
		}
	}
}

impl Algorithm {
	/// Returns static str describing journal database algorithm.
	pub fn as_str(&self) -> &'static str {
		match *self {
			Algorithm::Archive => "archive",
		}
	}

	/// Returns static str describing journal database algorithm.
	pub fn as_internal_name_str(&self) -> &'static str {
		match *self {
			Algorithm::Archive => "archive",
		}
	}

	/// Returns true if pruning strategy is stable
	pub fn is_stable(&self) -> bool {
		match *self {
			Algorithm::Archive => true,
		}
	}

	/// Returns all algorithm types.
	pub fn all_types() -> Vec<Algorithm> {
		vec![Algorithm::Archive]
	}
}

impl fmt::Display for Algorithm {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

/// Create a new `JournalDB` trait object over a generic key-value database.
pub fn new(backing: Arc<::kvdb::KeyValueDB>, algorithm: Algorithm, col: Option<u32>) -> Box<JournalDB> {
	match algorithm {
		Algorithm::Archive => Box::new(archivedb::ArchiveDB::new(backing, col)),
	}
}

// all keys must be at least 12 bytes
const DB_PREFIX_LEN : usize = ::kvdb::PREFIX_LEN;
const LATEST_ERA_KEY : [u8; ::kvdb::PREFIX_LEN] = [ b'l', b'a', b's', b't', 0, 0, 0, 0, 0, 0, 0, 0 ];

#[cfg(test)]
mod tests {
	use super::Algorithm;

	#[test]
	fn test_journal_algorithm_parsing() {
		assert_eq!(Algorithm::Archive, "archive".parse().unwrap());
	}

	#[test]
	fn test_journal_algorithm_printing() {
		assert_eq!(Algorithm::Archive.to_string(), "archive".to_owned());
	}

	#[test]
	fn test_journal_algorithm_is_stable() {
		assert!(Algorithm::Archive.is_stable());
	}

	#[test]
	fn test_journal_algorithm_default() {
		assert_eq!(Algorithm::default(), Algorithm::Archive);
	}

	#[test]
	fn test_journal_algorithm_all_types() {
		// compiling should fail if some cases are not covered
		let mut archive = 0;

		for a in &Algorithm::all_types() {
			match *a {
				Algorithm::Archive => archive += 1,
			}
		}

		assert_eq!(archive, 1);
	}
}
