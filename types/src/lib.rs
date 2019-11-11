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
#[cfg(test)]
extern crate serde_json;

mod block_hash;
mod common_params;

pub mod errors;
pub mod header;
pub mod transaction;
pub mod util;

pub type BlockNumber = u64;
pub type ShardId = u16;

pub use block_hash::BlockHash;
pub use common_params::CommonParams;
pub use header::Header;
