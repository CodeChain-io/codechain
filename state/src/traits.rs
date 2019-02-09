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

use ckey::{public_to_address, Address, Public, Signature};
use cmerkle::Result as TrieResult;
use ctypes::invoice::Invoice;
use ctypes::transaction::ShardTransaction;
use ctypes::ShardId;
use cvm::ChainTimeInfo;
use primitives::{Bytes, H160, H256};

use crate::{
    Account, ActionData, AssetScheme, CacheableItem, Metadata, OwnedAsset, RegularAccount, Shard, StateDB, StateResult,
    Text,
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
    fn balance(&self, a: &Address) -> TrieResult<u64> {
        Ok(self.account(a)?.map_or(0, |account| account.balance()))
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

    fn shard_id_by_hash(&self, tx_hash: &H256) -> TrieResult<Option<ShardId>> {
        Ok(self.metadata()?.and_then(|metadata| metadata.shard_id_by_hash(tx_hash)))
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
    fn asset_scheme(&self, shard_id: ShardId, asset_type: H160) -> TrieResult<Option<AssetScheme>> {
        match self.shard_state(shard_id)? {
            None => Ok(None),
            Some(state) => state.asset_scheme(asset_type),
        }
    }

    /// Get the asset.
    fn asset(&self, shard_id: ShardId, tracker: H256, index: usize) -> TrieResult<Option<OwnedAsset>> {
        match self.shard_state(shard_id)? {
            None => Ok(None),
            Some(state) => state.asset(tracker, index),
        }
    }

    fn text(&self, key: &H256) -> TrieResult<Option<Text>>;

    fn action_data(&self, key: &H256) -> TrieResult<Option<ActionData>>;
}

pub trait ShardStateView {
    /// Get the asset scheme.
    fn asset_scheme(&self, asset_type: H160) -> TrieResult<Option<AssetScheme>>;
    /// Get the asset.
    fn asset(&self, tracker: H256, index: usize) -> TrieResult<Option<OwnedAsset>>;
}

pub trait ShardState {
    fn apply<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        sender: &Address,
        shard_owners: &[Address],
        approvers: &[Address],
        client: &C,
    ) -> StateResult<Invoice>;
}

pub trait TopState {
    /// Remove an existing account.
    fn kill_account(&mut self, account: &Address);
    fn kill_regular_account(&mut self, account: &Public);

    /// Add `incr` to the balance of account `a`.
    fn add_balance(&mut self, a: &Address, incr: u64) -> TrieResult<()>;
    /// Subtract `decr` from the balance of account `a`.
    fn sub_balance(&mut self, a: &Address, decr: u64) -> StateResult<()>;
    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    fn transfer_balance(&mut self, from: &Address, to: &Address, by: u64) -> StateResult<()>;

    /// Increment the seq of account `a` by 1.
    fn inc_seq(&mut self, a: &Address) -> TrieResult<()>;

    /// Set the regular key of account `owner_public`
    fn set_regular_key(&mut self, owner_public: &Public, key: &Public) -> StateResult<()>;

    fn create_shard(&mut self, fee_payer: &Address, tx_hash: H256) -> StateResult<()>;
    fn change_shard_owners(&mut self, shard_id: ShardId, owners: &[Address], sender: &Address) -> StateResult<()>;
    fn change_shard_users(&mut self, shard_id: ShardId, users: &[Address], sender: &Address) -> StateResult<()>;

    fn set_shard_root(&mut self, shard_id: ShardId, new_root: H256) -> StateResult<()>;
    fn set_shard_owners(&mut self, shard_id: ShardId, new_owners: Vec<Address>) -> StateResult<()>;
    fn set_shard_users(&mut self, shard_id: ShardId, new_users: Vec<Address>) -> StateResult<()>;

    fn store_text(&mut self, key: &H256, text: Text, sig: &Signature) -> StateResult<()>;
    fn remove_text(&mut self, key: &H256, sig: &Signature) -> StateResult<()>;

    fn update_action_data(&mut self, key: &H256, data: Bytes) -> StateResult<()>;
}

pub trait StateWithCache {
    /// Commits our cached account changes into the trie.
    fn commit(&mut self) -> StateResult<H256>;
    fn commit_and_into_db(self) -> StateResult<(StateDB, H256)>;
}
