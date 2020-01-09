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
use crate::db;
use kvdb::{DBTransaction, KeyValueDB};

pub const VERSION_KEY_PREFIX: &[u8] = b"version_";
/// Save the version of Tendermint backup where the key below is pointing
pub const VERSION_KEY_TENDERMINT_BACKUP: &[u8] = b"version_tendermint-backup";

/// To support data values that are saved before the version scheme return 0 if the version does not exist
pub fn get_version(db: &dyn KeyValueDB, key: &[u8]) -> u32 {
    let value = db.get(db::COL_EXTRA, key).expect("Low level database error. Some issue with disk?");
    if let Some(bytes) = value {
        rlp::decode(&bytes).unwrap()
    } else {
        0
    }
}

pub fn set_version(batch: &mut DBTransaction, key: &[u8], value: u32) {
    assert!(
        key.starts_with(VERSION_KEY_PREFIX),
        "Version keys should be prefixed with".to_owned() + std::str::from_utf8(VERSION_KEY_PREFIX).unwrap()
    );
    batch.put(db::COL_EXTRA, key, &rlp::encode(&value));
}
