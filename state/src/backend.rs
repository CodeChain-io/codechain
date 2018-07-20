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

// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.
//
// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! A minimal "state backend" trait: an abstraction over the sources of data
//! a blockchain state may draw upon.
//!
//! Currently assumes a very specific DB + cache structure, but
//! should become general over time to the point where not even a
//! merkle trie is strictly necessary.

use ckey::Address;
use hashdb::HashDB;
use primitives::{Bytes, H256};

use super::{
    Account, Asset, AssetAddress, AssetScheme, AssetSchemeAddress, Metadata, MetadataAddress, Shard, ShardAddress,
};


/// State backend. See module docs for more details.
pub trait Backend: Send {
    /// Treat the backend as a read-only hashdb.
    fn as_hashdb(&self) -> &HashDB;

    /// Treat the backend as a writeable hashdb.
    fn as_hashdb_mut(&mut self) -> &mut HashDB;
}

pub trait TopBackend: Send {
    /// Add an account entry to the cache.
    fn add_to_account_cache(&mut self, addr: Address, data: Option<Account>, modified: bool);
    fn add_to_metadata_cache(&mut self, address: MetadataAddress, item: Option<Metadata>, modified: bool);
    fn add_to_shard_cache(&mut self, address: ShardAddress, item: Option<Shard>, modified: bool);
    fn add_to_action_data_cache(&mut self, address: H256, item: Option<Bytes>, modified: bool);

    /// Get basic copy of the cached account. Not required to include storage.
    /// Returns 'None' if cache is disabled or if the account is not cached.
    fn get_cached_account(&self, addr: &Address) -> Option<Option<Account>>;
    fn get_cached_metadata(&self, addr: &MetadataAddress) -> Option<Option<Metadata>>;
    fn get_cached_shard(&self, addr: &ShardAddress) -> Option<Option<Shard>>;
    fn get_cached_action_data(&self, key: &H256) -> Option<Option<Bytes>>;

    /// Get value from a cached account.
    /// `None` is passed to the closure if the account entry cached
    /// is known not to exist.
    /// `None` is returned if the entry is not cached.
    fn get_cached_account_with<F, U>(&self, a: &Address, f: F) -> Option<U>
    where
        F: FnOnce(Option<&mut Account>) -> U;
}

pub trait ShardBackend: Send {
    /// Add an asset entry to the cache.
    fn add_to_asset_scheme_cache(&mut self, addr: AssetSchemeAddress, asset: Option<AssetScheme>, modified: bool);
    /// Add an asset entry to the cache.
    fn add_to_asset_cache(&mut self, addr: AssetAddress, asset: Option<Asset>, modified: bool);

    /// Get basic copy of the cached account. Not required to include storage.
    /// Returns 'None' if cache is disabled or if the the asset/asset scheme is not cached.
    fn get_cached_asset_scheme(&self, hash: &AssetSchemeAddress) -> Option<Option<AssetScheme>>;

    fn get_cached_asset(&self, hash: &AssetAddress) -> Option<Option<Asset>>;
}
