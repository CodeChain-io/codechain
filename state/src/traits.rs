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
use cmerkle::Result as TrieResult;
use ctypes::transaction::{Outcome as TransactionOutcome, Transaction};
use ctypes::{ShardId, WorldId};
use primitives::{Bytes, H256, U256};

use super::backend::{ShardBackend, TopBackend};
use super::{Asset, AssetAddress, AssetScheme, AssetSchemeAddress, ShardMetadata, StateResult, World};


pub trait TopStateInfo {
    /// Get the nonce of account `a`.
    fn nonce(&self, a: &Address) -> TrieResult<U256>;

    /// Get the balance of account `a`.
    fn balance(&self, a: &Address) -> TrieResult<U256>;

    /// Get the regular key of account `a`.
    fn regular_key(&self, a: &Address) -> TrieResult<Option<Public>>;

    fn number_of_shards(&self) -> TrieResult<ShardId>;

    fn shard_root(&self, shard_id: ShardId) -> TrieResult<Option<H256>>;
    fn shard_owner(&self, shard_id: ShardId) -> TrieResult<Option<Address>>;

    fn shard_metadata(&self, shard_id: ShardId) -> TrieResult<Option<ShardMetadata>>;
    fn world(&self, shard_id: ShardId, world_id: WorldId) -> TrieResult<Option<World>>;

    /// Get the asset scheme.
    fn asset_scheme(&self, shard_id: ShardId, a: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>>;
    /// Get the asset.
    fn asset(&self, shard_id: ShardId, a: &AssetAddress) -> TrieResult<Option<Asset>>;

    fn action_data(&self, key: &H256) -> TrieResult<Bytes>;
}

pub trait ShardStateInfo {
    fn root(&self) -> &H256;

    fn metadata(&self) -> TrieResult<Option<ShardMetadata>>;
    fn world(&self, world_id: WorldId) -> TrieResult<Option<World>>;

    /// Get the asset scheme.
    fn asset_scheme(&self, a: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>>;
    /// Get the asset.
    fn asset(&self, a: &AssetAddress) -> TrieResult<Option<Asset>>;
}

pub trait ShardState<B>
where
    B: ShardBackend, {
    fn apply(
        &mut self,
        shard_id: ShardId,
        transaction: &Transaction,
        sender: &Address,
    ) -> StateResult<TransactionOutcome>;
}

pub trait TopState<B>
where
    B: TopBackend, {
    /// Remove an existing account.
    fn kill_account(&mut self, account: &Address);
    fn kill_regular_account(&mut self, account: &Public);

    fn account_exists(&self, a: &Address) -> TrieResult<bool>;

    fn account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool>;
    fn account_exists_and_has_nonce(&self, a: &Address) -> TrieResult<bool>;

    fn master_account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool>;
    fn regular_account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool>;

    /// Add `incr` to the balance of account `a`.
    fn add_balance(&mut self, a: &Address, incr: &U256) -> TrieResult<()>;
    /// Subtract `decr` from the balance of account `a`.
    fn sub_balance(&mut self, a: &Address, decr: &U256) -> TrieResult<()>;
    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) -> StateResult<()>;

    /// Increment the nonce of account `a` by 1.
    fn inc_nonce(&mut self, a: &Address) -> TrieResult<()>;

    /// Set the regular key of account `master_public`
    fn set_regular_key(&mut self, master_public: &Public, key: &Public) -> StateResult<()>;

    fn create_shard(&mut self, shard_creation_cost: &U256, fee_payer: &Address) -> StateResult<()>;

    fn set_shard_root(&mut self, shard_id: ShardId, old_root: &H256, new_root: &H256) -> StateResult<()>;
    fn set_shard_owner(&mut self, shard_id: ShardId, old_owner: &Address, new_owner: Address) -> StateResult<()>;

    fn update_action_data(&mut self, key: &H256, data: Bytes) -> StateResult<()>;
}

pub trait StateWithCache {
    /// Commits our cached account changes into the trie.
    fn commit(&mut self) -> TrieResult<()>;

    /// Propagate local cache into shared canonical state cache.
    fn propagate_to_global_cache(&mut self);

    /// Clear state cache
    fn clear(&mut self);
}
