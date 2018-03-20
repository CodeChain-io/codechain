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

use std::path::Path;
use std::str::FromStr;

use kvdb_rocksdb::CompactionProfile;

use super::super::verification::VerifierType;

/// Client state db compaction profile
#[derive(Debug, PartialEq, Clone)]
pub enum DatabaseCompactionProfile {
    /// Try to determine compaction profile automatically
    Auto,
    /// SSD compaction profile
    SSD,
    /// HDD or other slow storage io compaction profile
    HDD,
}

impl Default for DatabaseCompactionProfile {
    fn default() -> Self {
        DatabaseCompactionProfile::Auto
    }
}

impl DatabaseCompactionProfile {
    /// Returns corresponding compaction profile.
    pub fn compaction_profile(&self, db_path: &Path) -> CompactionProfile {
        match *self {
            DatabaseCompactionProfile::Auto => CompactionProfile::auto(db_path),
            DatabaseCompactionProfile::SSD => CompactionProfile::ssd(),
            DatabaseCompactionProfile::HDD => CompactionProfile::hdd(),
        }
    }
}

impl FromStr for DatabaseCompactionProfile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(DatabaseCompactionProfile::Auto),
            "ssd" => Ok(DatabaseCompactionProfile::SSD),
            "hdd" => Ok(DatabaseCompactionProfile::HDD),
            _ => Err("Invalid compaction profile given. Expected default/hdd/ssd.".into()),
        }
    }
}

/// Client configuration. Includes configs for all sub-systems.
#[derive(Debug, PartialEq, Default)]
pub struct ClientConfig {
    /// RocksDB column cache-size if not default
    pub db_cache_size: Option<usize>,
    /// State db compaction profile
    pub db_compaction: DatabaseCompactionProfile,
    /// Should db have WAL enabled?
    pub db_wal: bool,
    /// Type of block verifier used by client.
    pub verifier_type: VerifierType,
}


