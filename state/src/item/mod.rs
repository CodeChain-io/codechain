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

#[macro_use]
mod address;

pub mod account;
pub mod asset;
pub mod asset_scheme;
pub mod cache;
pub mod metadata;
pub mod regular_account;
pub mod shard;
pub mod shard_metadata;
pub mod world;

const ASSET_PREFIX: u8 = 'A' as u8;
const ADDRESS_PREFIX: u8 = 'C' as u8;
const SHARD_METADATA_PREFIX: u8 = 'E' as u8;
const SHARD_PREFIX: u8 = 'H' as u8;
const METADATA_PREFIX: u8 = 'M' as u8;
const REGULAR_ACCOUNT_PREFIX: u8 = 'R' as u8;
const ASSET_SCHEME_PREFIX: u8 = 'S' as u8;
const WORLD_PREFIX: u8 = 'W' as u8;
