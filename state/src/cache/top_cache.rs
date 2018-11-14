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

use std::cell::RefMut;

use ckey::Address;
use cmerkle::{Result as TrieResult, TrieDB, TrieMut};
use primitives::H256;

use super::super::{
    Account, ActionData, Metadata, MetadataAddress, RegularAccount, RegularAccountAddress, Shard, ShardAddress,
};
use super::WriteBack;

pub struct TopCache {
    account: WriteBack<Account>,
    regular_account: WriteBack<RegularAccount>,
    metadata: WriteBack<Metadata>,
    shard: WriteBack<Shard>,
    action_data: WriteBack<ActionData>,
}

impl TopCache {
    pub fn new(
        accounts: impl Iterator<Item = (Address, Account)>,
        regular_accounts: impl Iterator<Item = (RegularAccountAddress, RegularAccount)>,
        metadata: impl Iterator<Item = (MetadataAddress, Metadata)>,
        shards: impl Iterator<Item = (ShardAddress, Shard)>,
        action_data: impl Iterator<Item = (H256, ActionData)>,
    ) -> Self {
        Self {
            account: WriteBack::new_with_iter(accounts),
            regular_account: WriteBack::new_with_iter(regular_accounts),
            metadata: WriteBack::new_with_iter(metadata),
            shard: WriteBack::new_with_iter(shards),
            action_data: WriteBack::new_with_iter(action_data),
        }
    }

    pub fn checkpoint(&mut self) {
        self.account.checkpoint();
        self.regular_account.checkpoint();
        self.metadata.checkpoint();
        self.shard.checkpoint();
        self.action_data.checkpoint();
    }

    pub fn discard_checkpoint(&mut self) {
        self.account.discard_checkpoint();
        self.regular_account.discard_checkpoint();
        self.metadata.discard_checkpoint();
        self.shard.discard_checkpoint();
        self.action_data.discard_checkpoint();
    }

    pub fn revert_to_checkpoint(&mut self) {
        self.account.revert_to_checkpoint();
        self.regular_account.revert_to_checkpoint();
        self.metadata.revert_to_checkpoint();
        self.shard.revert_to_checkpoint();
        self.action_data.revert_to_checkpoint();
    }

    pub fn commit<'db>(&mut self, trie: &mut Box<TrieMut + 'db>) -> TrieResult<()> {
        self.account.commit(trie)?;
        self.regular_account.commit(trie)?;
        self.metadata.commit(trie)?;
        self.shard.commit(trie)?;
        self.action_data.commit(trie)?;
        Ok(())
    }

    pub fn account(&self, a: &Address, db: TrieDB) -> TrieResult<Option<Account>> {
        self.account.get(a, db)
    }

    pub fn account_mut(&self, a: &Address, db: TrieDB) -> TrieResult<RefMut<Account>> {
        self.account.get_mut(a, db)
    }

    pub fn remove_account(&self, address: &Address) {
        self.account.remove(address)
    }

    pub fn regular_account(&self, a: &RegularAccountAddress, db: TrieDB) -> TrieResult<Option<RegularAccount>> {
        self.regular_account.get(a, db)
    }

    pub fn regular_account_mut(&self, a: &RegularAccountAddress, db: TrieDB) -> TrieResult<RefMut<RegularAccount>> {
        self.regular_account.get_mut(a, db)
    }

    pub fn remove_regular_account(&self, address: &RegularAccountAddress) {
        self.regular_account.remove(address)
    }

    pub fn metadata(&self, a: &MetadataAddress, db: TrieDB) -> TrieResult<Option<Metadata>> {
        self.metadata.get(a, db)
    }

    pub fn metadata_mut(&self, a: &MetadataAddress, db: TrieDB) -> TrieResult<RefMut<Metadata>> {
        self.metadata.get_mut(a, db)
    }

    pub fn shard(&self, a: &ShardAddress, db: TrieDB) -> TrieResult<Option<Shard>> {
        self.shard.get(a, db)
    }

    pub fn shard_mut(&self, a: &ShardAddress, db: TrieDB) -> TrieResult<RefMut<Shard>> {
        self.shard.get_mut(a, db)
    }

    #[allow(dead_code)]
    pub fn remove_shard(&self, address: &ShardAddress) {
        self.shard.remove(address)
    }

    pub fn action_data(&self, a: &H256, db: TrieDB) -> TrieResult<Option<ActionData>> {
        self.action_data.get(a, db)
    }

    pub fn action_data_mut(&self, a: &H256, db: TrieDB) -> TrieResult<RefMut<ActionData>> {
        self.action_data.get_mut(a, db)
    }

    #[allow(dead_code)]
    pub fn remove_action_data(&self, address: &H256) {
        self.action_data.remove(address)
    }

    pub fn cached_accounts(&self) -> Vec<(Address, Option<Account>)> {
        self.account.items()
    }

    pub fn cached_regular_accounts(&self) -> Vec<(RegularAccountAddress, Option<RegularAccount>)> {
        self.regular_account.items()
    }

    pub fn cached_metadata(&self) -> Vec<(MetadataAddress, Option<Metadata>)> {
        self.metadata.items()
    }

    pub fn cached_shards(&self) -> Vec<(ShardAddress, Option<Shard>)> {
        self.shard.items()
    }

    pub fn cached_action_data(&self) -> Vec<(H256, Option<ActionData>)> {
        self.action_data.items()
    }
}

impl Clone for TopCache {
    fn clone(&self) -> Self {
        Self {
            account: self.account.clone(),
            regular_account: self.regular_account.clone(),
            metadata: self.metadata.clone(),
            shard: self.shard.clone(),
            action_data: self.action_data.clone(),
        }
    }
}
