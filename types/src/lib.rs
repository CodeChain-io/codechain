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

extern crate byteorder;
extern crate codechain_crypto as ccrypto;
extern crate codechain_key as ckey;
extern crate heapsize;
extern crate primitives;
#[cfg_attr(test, macro_use)]
extern crate rlp;
#[macro_use]
extern crate rlp_derive;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate unexpected;

pub mod invoice;
pub mod machine;
pub mod parcel;
pub mod transaction;

pub type BlockNumber = u64;
pub type ShardId = u32;
