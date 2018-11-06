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
use ctypes::invoice::Invoice;
use ctypes::transaction::Transaction;
use ctypes::ShardId;
use cvm::ChainTimeInfo;
use primitives::{Bytes, H256, U256};

use super::{AssetScheme, AssetSchemeAddress, OwnedAsset, OwnedAssetAddress, StateResult};


pub trait TopStateInfo {
    /// Get the seq of account `a`.
    fn seq(&self, a: &Address) -> TrieResult<U256>;

    /// Get the balance of account `a`.
    fn balance(&self, a: &Address) -> TrieResult<U256>;

    /// Get the regular key of account `a`.
    fn regular_key(&self, a: &Address) -> TrieResult<Option<Public>>;

    fn regular_key_owner(&self, address: &Address) -> TrieResult<Option<Address>>;

    fn number_of_shards(&self) -> TrieResult<ShardId>;

    fn shard_root(&self, shard_id: ShardId) -> TrieResult<Option<H256>>;
    fn shard_owners(&self, shard_id: ShardId) -> TrieResult<Option<Vec<Address>>>;
    fn shard_users(&self, shard_id: ShardId) -> TrieResult<Option<Vec<Address>>>;

    /// Get the asset scheme.
    fn asset_scheme(&self, shard_id: ShardId, a: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>>;
    /// Get the asset.
    fn asset(&self, shard_id: ShardId, a: &OwnedAssetAddress) -> TrieResult<Option<OwnedAsset>>;

    fn action_data(&self, key: &H256) -> TrieResult<Bytes>;
}

pub trait ShardStateInfo {
    fn root(&self) -> &H256;

    /// Get the asset scheme.
    fn asset_scheme(&self, a: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>>;
    /// Get the asset.
    fn asset(&self, a: &OwnedAssetAddress) -> TrieResult<Option<OwnedAsset>>;
}

pub trait ShardState {
    fn apply<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        sender: &Address,
        shard_owners: &[Address],
        client: &C,
    ) -> StateResult<Invoice>;
}

pub trait TopState {
    /// Remove an existing account.
    fn kill_account(&mut self, account: &Address);
    fn kill_regular_account(&mut self, account: &Public);

    fn account_exists(&self, a: &Address) -> TrieResult<bool>;

    fn account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool>;
    fn account_exists_and_has_seq(&self, a: &Address) -> TrieResult<bool>;

    fn regular_account_exists_and_not_null(&self, p: &Public) -> TrieResult<bool>;
    fn regular_account_exists_and_not_null_by_address(&self, a: &Address) -> TrieResult<bool>;

    /// Add `incr` to the balance of account `a`.
    fn add_balance(&mut self, a: &Address, incr: &U256) -> TrieResult<()>;
    /// Subtract `decr` from the balance of account `a`.
    fn sub_balance(&mut self, a: &Address, decr: &U256) -> TrieResult<()>;
    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) -> StateResult<()>;

    /// Increment the seq of account `a` by 1.
    fn inc_seq(&mut self, a: &Address) -> TrieResult<()>;

    /// Set the regular key of account `owner_public`
    fn set_regular_key(&mut self, owner_public: &Public, key: &Public) -> StateResult<()>;

    fn create_shard(&mut self, shard_creation_cost: &U256, fee_payer: &Address) -> StateResult<()>;
    fn change_shard_owners(&mut self, shard_id: ShardId, owners: &[Address], sender: &Address) -> StateResult<()>;
    fn change_shard_users(&mut self, shard_id: ShardId, users: &[Address], sender: &Address) -> StateResult<()>;

    fn set_shard_root(&mut self, shard_id: ShardId, old_root: &H256, new_root: &H256) -> StateResult<()>;
    fn set_shard_owners(&mut self, shard_id: ShardId, new_owners: Vec<Address>) -> StateResult<()>;
    fn set_shard_users(&mut self, shard_id: ShardId, new_users: Vec<Address>) -> StateResult<()>;

    fn update_action_data(&mut self, key: &H256, data: Bytes) -> StateResult<()>;
}

pub trait StateWithCache {
    /// Commits our cached account changes into the trie.
    fn commit(&mut self) -> TrieResult<()>;

    /// Clear state cache
    fn clear(&mut self);
}
