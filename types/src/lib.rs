// Copyright 2018-2019 Kodebox, Inc.
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

extern crate codechain_crypto as ccrypto;
extern crate codechain_json as cjson;
extern crate codechain_key as ckey;
extern crate primitives;
extern crate rlp;
#[macro_use]
extern crate rlp_derive;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rustc_hex;
#[cfg(test)]
extern crate serde_json;

mod block_hash;
mod common_params;
mod tracker;
mod tx_hash;

pub mod errors;
pub mod header;
pub mod transaction;
pub mod util;

pub type BlockNumber = u64;
pub type ShardId = u16;

pub use block_hash::BlockHash;
pub use common_params::CommonParams;
pub use header::Header;
use rustc_hex::ToHex;
pub use tracker::Tracker;
pub use tx_hash::TxHash;

#[derive(Debug, Clone)]
pub struct DebugRead {
    pub time_us: u128,
    pub key: String,
    pub archive_db_overlay_us: u128,
    pub kvrocks_overlay_us: u128,
    pub kvrocks_flushing_us: u128,
    pub kvrocks_get_opt_us: u128,
    pub kvrocks_get_cf_opt_us: u128,
    pub kvrocks_get_cf_opt_us_vec: Vec<u128>,
}

impl DebugRead {
    pub fn empty() -> Self {
        Self {
            time_us: 0,
            key: "".to_string(),
            archive_db_overlay_us: 0,
            kvrocks_overlay_us: 0,
            kvrocks_flushing_us: 0,
            kvrocks_get_opt_us: 0,
            kvrocks_get_cf_opt_us: 0,
            kvrocks_get_cf_opt_us_vec: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub read_count: u32,
    pub db_read_time_us: u128,
    pub reads: Vec<DebugRead>,
}

impl DebugInfo {
    pub fn empty() -> Self {
        Self {
            read_count: 0,
            db_read_time_us: 0,
            reads: Vec::new(),
        }
    }

    pub fn inc_read_count(&mut self) {
        self.read_count += 1;
    }

    // pub fn add_read(&mut self, time_us: u128, key: &[u8]) {
    //     self.reads.push(DebugRead {
    //         time_us,
    //         key: key.to_hex(),
    //     })
    // }
}
