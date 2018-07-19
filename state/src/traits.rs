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

use ckey::{Address, Public};
use ctypes::transaction::{Outcome as TransactionOutcome, Transaction};
use primitives::{H256, U256};
use trie::Result as TrieResult;

use super::backend::{ShardBackend, TopBackend};
use super::{Asset, AssetAddress, AssetScheme, AssetSchemeAddress, StateResult};


pub trait TopStateInfo {
    /// Get the nonce of account `a`.
    fn nonce(&self, a: &Address) -> TrieResult<U256>;

    /// Get the balance of account `a`.
    fn balance(&self, a: &Address) -> TrieResult<U256>;

    /// Get the regular key of account `a`.
    fn regular_key(&self, a: &Address) -> TrieResult<Option<Public>>;

    fn number_of_shards(&self) -> TrieResult<u32>;

    fn shard_root(&self, shard_id: u32) -> TrieResult<Option<H256>>;

    /// Get the asset scheme.
    fn asset_scheme(&self, shard_id: u32, a: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>>;
    /// Get the asset.
    fn asset(&self, shard_id: u32, a: &AssetAddress) -> TrieResult<Option<Asset>>;
}

pub trait ShardStateInfo {
    fn root(&self) -> &H256;
    /// Get the asset scheme.
    fn asset_scheme(&self, a: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>>;
    /// Get the asset.
    fn asset(&self, a: &AssetAddress) -> TrieResult<Option<Asset>>;
}

impl TopStateInfo for () {
    fn nonce(&self, _address: &Address) -> TrieResult<U256> {
        unimplemented!()
    }
    fn balance(&self, _address: &Address) -> TrieResult<U256> {
        unimplemented!()
    }
    fn regular_key(&self, _address: &Address) -> TrieResult<Option<Public>> {
        unimplemented!()
    }

    fn number_of_shards(&self) -> TrieResult<u32> {
        unimplemented!()
    }

    fn shard_root(&self, _shard_id: u32) -> TrieResult<Option<H256>> {
        unimplemented!()
    }

    fn asset_scheme(&self, _shard_id: u32, _: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>> {
        unimplemented!()
    }

    fn asset(&self, _shard_id: u32, _: &AssetAddress) -> TrieResult<Option<Asset>> {
        unimplemented!()
    }
}

impl ShardStateInfo for () {
    fn root(&self) -> &H256 {
        unimplemented!()
    }

    fn asset_scheme(&self, _a: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>> {
        unimplemented!()
    }
    fn asset(&self, _a: &AssetAddress) -> TrieResult<Option<Asset>> {
        unimplemented!()
    }
}

pub trait ShardState<B>
where
    B: ShardBackend, {
    fn apply(&mut self, transaction: &Transaction, parcel_network_id: &u64) -> StateResult<TransactionOutcome>;
}

pub trait TopState<B>
where
    B: TopBackend, {
    /// Remove an existing account.
    fn kill_account(&mut self, account: &Address);

    fn account_exists(&self, a: &Address) -> TrieResult<bool>;

    fn account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool>;
    fn account_exists_and_has_nonce(&self, a: &Address) -> TrieResult<bool>;

    /// Add `incr` to the balance of account `a`.
    fn add_balance(&mut self, a: &Address, incr: &U256) -> TrieResult<()>;
    /// Subtract `decr` from the balance of account `a`.
    fn sub_balance(&mut self, a: &Address, decr: &U256) -> TrieResult<()>;
    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) -> StateResult<()>;

    /// Increment the nonce of account `a` by 1.
    fn inc_nonce(&mut self, a: &Address) -> TrieResult<()>;

    /// Set the regular key of account `a`
    fn set_regular_key(&mut self, a: &Address, key: &Public) -> StateResult<()>;

    fn create_shard(&mut self, shard_creation_cost: &U256, fee_payer: &Address) -> StateResult<()>;
    fn set_shard_root(&mut self, shard_id: u32, old_root: &H256, new_root: &H256) -> StateResult<()>;
}

pub trait StateWithCache {
    /// Commits our cached account changes into the trie.
    fn commit(&mut self) -> TrieResult<()>;

    /// Propagate local cache into shared canonical state cache.
    fn propagate_to_global_cache(&mut self);

    /// Clear state cache
    fn clear(&mut self);
}
