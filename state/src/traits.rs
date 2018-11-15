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

use ckey::{public_to_address, Address, Public};
use cmerkle::Result as TrieResult;
use ctypes::invoice::Invoice;
use ctypes::transaction::InnerTransaction;
use ctypes::ShardId;
use cvm::ChainTimeInfo;
use primitives::{Bytes, H256, U256};

use super::{
    Account, ActionData, AssetScheme, AssetSchemeAddress, CacheableItem, Metadata, OwnedAsset, OwnedAssetAddress,
    RegularAccount, Shard, StateResult,
};


pub trait TopStateView {
    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn account(&self, a: &Address) -> TrieResult<Option<Account>>;

    /// Get the seq of account `a`.
    fn seq(&self, a: &Address) -> TrieResult<u64> {
        Ok(self.account(a)?.map_or(0, |account| account.seq()))
    }

    /// Get the balance of account `a`.
    fn balance(&self, a: &Address) -> TrieResult<U256> {
        Ok(self.account(a)?.map_or_else(U256::zero, |account| *account.balance()))
    }

    fn account_exists(&self, a: &Address) -> TrieResult<bool> {
        // Bloom filter does not contain empty accounts, so it is important here to
        // check if account exists in the database directly before EIP-161 is in effect.
        Ok(self.account(a)?.is_some())
    }

    fn account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool> {
        Ok(self.account(a)?.map(|a| !a.is_null()).unwrap_or(false))
    }

    fn account_exists_and_has_seq(&self, a: &Address) -> TrieResult<bool> {
        Ok(self.account(a)?.map(|a| a.seq() != 0).unwrap_or(false))
    }

    fn regular_account_by_address(&self, a: &Address) -> TrieResult<Option<RegularAccount>>;

    fn regular_account(&self, p: &Public) -> TrieResult<Option<RegularAccount>> {
        self.regular_account_by_address(&public_to_address(p))
    }

    /// Get the regular key of account `a`.
    fn regular_key(&self, a: &Address) -> TrieResult<Option<Public>> {
        Ok(self.account(a)?.and_then(|account| account.regular_key()))
    }

    fn regular_key_owner(&self, address: &Address) -> TrieResult<Option<Address>> {
        Ok(self
            .regular_account_by_address(&address)?
            .map(|regular_account| public_to_address(regular_account.owner_public())))
    }

    fn regular_account_exists_and_not_null(&self, p: &Public) -> TrieResult<bool> {
        Ok(self.regular_account(p)?.map_or(false, |a| !a.is_null()))
    }

    fn regular_account_exists_and_not_null_by_address(&self, a: &Address) -> TrieResult<bool> {
        Ok(self.regular_account_by_address(a)?.map_or(false, |a| !a.is_null()))
    }

    fn metadata(&self) -> TrieResult<Option<Metadata>>;

    fn number_of_shards(&self) -> TrieResult<ShardId> {
        Ok(*self.metadata()?.expect("Metadata must exist").number_of_shards())
    }

    fn shard(&self, shard_id: ShardId) -> TrieResult<Option<Shard>>;
    fn shard_state<'db>(&'db self, shard_id: ShardId) -> TrieResult<Option<Box<ShardStateView + 'db>>>;

    fn shard_root(&self, shard_id: ShardId) -> TrieResult<Option<H256>> {
        Ok(self.shard(shard_id)?.map(|shard| *shard.root()))
    }

    fn shard_owners(&self, shard_id: ShardId) -> TrieResult<Option<Vec<Address>>> {
        Ok(self.shard(shard_id)?.map(|shard| shard.owners().to_vec()))
    }

    fn shard_users(&self, shard_id: ShardId) -> TrieResult<Option<Vec<Address>>> {
        Ok(self.shard(shard_id)?.map(|shard| shard.users().to_vec()))
    }

    /// Get the asset scheme.
    fn asset_scheme(
        &self,
        shard_id: ShardId,
        asset_scheme_address: &AssetSchemeAddress,
    ) -> TrieResult<Option<AssetScheme>> {
        match self.shard_state(shard_id)? {
            None => Ok(None),
            Some(state) => state.asset_scheme(asset_scheme_address),
        }
    }

    /// Get the asset.
    fn asset(&self, shard_id: ShardId, asset_address: &OwnedAssetAddress) -> TrieResult<Option<OwnedAsset>> {
        match self.shard_state(shard_id)? {
            None => Ok(None),
            Some(state) => state.asset(asset_address),
        }
    }

    fn action_data(&self, key: &H256) -> TrieResult<Option<ActionData>>;
}

pub trait ShardStateView {
    /// Get the asset scheme.
    fn asset_scheme(&self, a: &AssetSchemeAddress) -> TrieResult<Option<AssetScheme>>;
    /// Get the asset.
    fn asset(&self, a: &OwnedAssetAddress) -> TrieResult<Option<OwnedAsset>>;
}

pub trait ShardState {
    fn apply<C: ChainTimeInfo>(
        &mut self,
        transaction: &InnerTransaction,
        sender: &Address,
        shard_owners: &[Address],
        client: &C,
    ) -> StateResult<Invoice>;
}

pub trait TopState {
    /// Remove an existing account.
    fn kill_account(&mut self, account: &Address);
    fn kill_regular_account(&mut self, account: &Public);

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

    fn set_shard_root(&mut self, shard_id: ShardId, new_root: H256) -> StateResult<()>;
    fn set_shard_owners(&mut self, shard_id: ShardId, new_owners: Vec<Address>) -> StateResult<()>;
    fn set_shard_users(&mut self, shard_id: ShardId, new_users: Vec<Address>) -> StateResult<()>;

    fn update_action_data(&mut self, key: &H256, data: Bytes) -> StateResult<()>;
}

pub trait StateWithCache {
    /// Commits our cached account changes into the trie.
    fn commit(&mut self) -> StateResult<H256>;
}
