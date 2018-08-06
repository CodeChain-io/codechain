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

//! A mutable state representation suitable to execute parcels.
//! Generic over a `Backend`. Deals with `Account`s.
//! Unconfirmed sub-states are managed with `checkpoint`s which may be canonicalized
//! or rolled back.

use std::cell::RefMut;
use std::fmt;

use ccrypto::BLAKE_NULL_RLP;
use ckey::{public_to_address, Address, Public};
use cmerkle::{Result as TrieResult, Trie, TrieError, TrieFactory};
use ctypes::invoice::Invoice;
use ctypes::parcel::{Action, ChangeShard, Error as ParcelError, Outcome as ParcelOutcome, Parcel};
use ctypes::transaction::{Error as TransactionError, Outcome as TransactionOutcome, Transaction};
use ctypes::ShardId;
use primitives::{Bytes, H256, U256};
use rlp::NULL_RLP;
use unexpected::Mismatch;

use super::super::backend::TopBackend;
use super::super::checkpoint::{CheckpointId, StateWithCheckpoint};
use super::super::item::cache::{Cache, CacheableItem};
use super::super::traits::{ShardState, ShardStateInfo, StateWithCache, TopState, TopStateInfo};
use super::super::{
    Account, Asset, AssetAddress, AssetScheme, AssetSchemeAddress, Metadata, MetadataAddress, RegularAccount,
    RegularAccountAddress, Shard, ShardAddress, ShardLevelState,
};
use super::super::{StateDB, StateError, StateResult};

/// Representation of the entire state of all accounts in the system.
///
/// `State` can work together with `StateDB` to share account cache.
///
/// Local cache contains changes made locally and changes accumulated
/// locally from previous commits. Global cache reflects the database
/// state and never contains any changes.
///
/// Cache items contains account data, or the flag that account does not exist
/// and modification state (see `AccountState`)
///
/// Account data can be in the following cache states:
/// * In global but not local - something that was queried from the database,
/// but never modified
/// * In local but not global - something that was just added (e.g. new account)
/// * In both with the same value - something that was changed to a new value,
/// but changed back to a previous block in the same block (same State instance)
/// * In both with different values - something that was overwritten with a
/// new value.
///
/// All read-only state queries check local cache/modifications first,
/// then global state cache. If data is not found in any of the caches
/// it is loaded from the DB to the local cache.
///
/// **** IMPORTANT *************************************************************
/// All the modifications to the account data must set the `Dirty` state in the
/// `Entry<Item>`. This is done in `require` and `require_or_from`. So just
/// use that.
/// ****************************************************************************
///
/// Upon destruction all the local cache data propagated into the global cache.
/// Propagated items might be rejected if current state is non-canonical.
///
/// State checkpointing.
///
/// A new checkpoint can be created with `checkpoint()`. checkpoints can be
/// created in a hierarchy.
/// When a checkpoint is active all changes are applied directly into
/// `cache` and the original value is copied into an active checkpoint.
/// Reverting a checkpoint with `revert_to_checkpoint` involves copying
/// original values from the latest checkpoint back into `cache`. The code
/// takes care not to overwrite cached storage while doing that.
/// checkpoint can be discarded with `discard_checkpoint`. All of the orignal
/// backed-up values are moved into a parent checkpoint (if any).
pub struct TopLevelState {
    db: StateDB,
    root: H256,
    account: Cache<Account>,
    regular_account: Cache<RegularAccount>,
    metadata: Cache<Metadata>,
    shard: Cache<Shard>,
    action_data: Cache<Bytes>,
    id_of_checkpoints: Vec<CheckpointId>,
}

impl TopStateInfo for TopLevelState {
    fn nonce(&self, a: &Address) -> TrieResult<U256> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or_else(U256::zero, |account| *account.nonce()))
    }
    fn balance(&self, a: &Address) -> TrieResult<U256> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or_else(U256::zero, |account| *account.balance()))
    }
    fn regular_key(&self, a: &Address) -> TrieResult<Option<Public>> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or(None, |account| account.regular_key()))
    }

    fn number_of_shards(&self) -> TrieResult<ShardId> {
        let metadata = self.require_metadata()?;
        Ok(*metadata.number_of_shards())
    }

    fn shard_root(&self, shard_id: ShardId) -> TrieResult<Option<H256>> {
        let shard_address = ShardAddress::new(shard_id);
        let shard = self.db.get_cached_shard(&shard_address).and_then(|s| s).map(|s| s.root().clone());
        if shard.is_some() {
            return Ok(shard)
        }

        // because of lexical borrow of self.db
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        Ok(db.get_with(&shard_address, ::rlp::decode::<Shard>)?.map(|s| s.root().clone()))
    }

    fn shard_owner(&self, shard_id: ShardId) -> TrieResult<Option<Address>> {
        let shard_address = ShardAddress::new(shard_id);
        let owner = self.db.get_cached_shard(&shard_address).and_then(|s| s).map(|s| s.owner().clone());
        if owner.is_some() {
            return Ok(owner)
        }

        // because of lexical borrow of self.db
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        Ok(db.get_with(&shard_address, ::rlp::decode::<Shard>)?.map(|s| s.owner().clone()))
    }

    fn asset_scheme(
        &self,
        shard_id: ShardId,
        asset_scheme_address: &AssetSchemeAddress,
    ) -> TrieResult<Option<AssetScheme>> {
        // FIXME: Handle the case that shard doesn't exist
        let shard_root = self.shard_root(shard_id)?.unwrap_or(BLAKE_NULL_RLP);
        // FIXME: Make it mutable borrow db instead of cloning.
        let shard_level_state = ShardLevelState::from_existing(shard_id, self.db.clone(), shard_root)?;
        shard_level_state.asset_scheme(asset_scheme_address)
    }

    fn asset(&self, shard_id: ShardId, asset_address: &AssetAddress) -> TrieResult<Option<Asset>> {
        // FIXME: Handle the case that shard doesn't exist
        let shard_root = self.shard_root(shard_id)?.unwrap_or(BLAKE_NULL_RLP);
        // FIXME: Make it mutable borrow db instead of cloning.
        let shard_level_state = ShardLevelState::from_existing(shard_id, self.db.clone(), shard_root)?;
        shard_level_state.asset(asset_address)
    }

    fn action_data(&self, key: &H256) -> TrieResult<Bytes> {
        let action_data = self.require_action_data(key)?;
        Ok(action_data.clone())
    }
}

const PARCEL_FEE_CHECKPOINT: CheckpointId = 123;
const PARCEL_ACTION_CHECKPOINT: CheckpointId = 130;

impl StateWithCheckpoint for TopLevelState {
    fn create_checkpoint(&mut self, id: CheckpointId) {
        self.id_of_checkpoints.push(id);
        self.account.checkpoint();
        self.regular_account.checkpoint();
        self.metadata.checkpoint();
        self.shard.checkpoint();
        self.action_data.checkpoint();
    }

    fn discard_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.account.discard_checkpoint();
        self.regular_account.discard_checkpoint();
        self.metadata.discard_checkpoint();
        self.shard.discard_checkpoint();
        self.action_data.discard_checkpoint();
    }

    fn revert_to_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.account.revert_to_checkpoint();
        self.regular_account.revert_to_checkpoint();
        self.metadata.revert_to_checkpoint();
        self.shard.revert_to_checkpoint();
        self.action_data.revert_to_checkpoint();
    }
}

impl StateWithCache for TopLevelState {
    fn commit(&mut self) -> TrieResult<()> {
        let mut trie = TrieFactory::from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
        self.account.commit(&mut trie)?;
        self.regular_account.commit(&mut trie)?;
        self.metadata.commit(&mut trie)?;
        self.shard.commit(&mut trie)?;
        self.action_data.commit(&mut trie)?;
        Ok(())
    }

    fn propagate_to_global_cache(&mut self) {
        let ref mut db = self.db;
        self.account.propagate_to_global_cache(|address, item, modified| {
            db.add_to_account_cache(address, item, modified);
        });
        self.regular_account.propagate_to_global_cache(|address, item, modified| {
            db.add_to_regular_account_cache(address, item, modified);
        });
        self.metadata.propagate_to_global_cache(|address, item, modified| {
            db.add_to_metadata_cache(address, item, modified);
        });
        self.shard.propagate_to_global_cache(|address, item, modified| {
            db.add_to_shard_cache(address, item, modified);
        });
        self.action_data.propagate_to_global_cache(|address, item, modified| {
            db.add_to_action_data_cache(address, item, modified);
        });
    }

    fn clear(&mut self) {
        self.account.clear();
        self.regular_account.clear();
        self.metadata.clear();
        self.shard.clear();
        self.action_data.clear();
    }
}

impl TopLevelState {
    /// Creates new state with empty state root
    /// Used for tests.
    #[cfg(test)]
    pub fn new(mut db: StateDB) -> Self {
        let mut root = H256::new();

        // init trie and reset root too null
        let _ = TrieFactory::create(db.as_hashdb_mut(), &mut root);

        TopLevelState {
            db,
            root,
            account: Cache::new(),
            regular_account: Cache::new(),
            metadata: Cache::new(),
            shard: Cache::new(),
            action_data: Cache::new(),
            id_of_checkpoints: Default::default(),
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(db: StateDB, root: H256) -> Result<Self, TrieError> {
        if !db.as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        let state = TopLevelState {
            db,
            root,
            account: Cache::new(),
            regular_account: Cache::new(),
            metadata: Cache::new(),
            shard: Cache::new(),
            action_data: Cache::new(),
            id_of_checkpoints: Default::default(),
        };

        Ok(state)
    }

    pub fn root(&self) -> &H256 {
        &self.root
    }

    /// Destroy the current object and return root and database.
    pub fn drop(mut self) -> (H256, StateDB) {
        self.propagate_to_global_cache();
        (self.root, self.db)
    }

    /// Execute a given parcel, charging parcel fee.
    /// This will change the state accordingly.
    pub fn apply(
        &mut self,
        parcel: &Parcel,
        fee_payer: &Address,
        fee_payer_public: &Public,
    ) -> StateResult<ParcelOutcome> {
        // Change the address to a master address if it is a regular key.
        let fee_payer = if self.regular_account_exists_and_not_null(fee_payer)? {
            let regular_account = self.require_regular_account_from_address(fee_payer)?;
            public_to_address(&regular_account.master_account())
        } else {
            fee_payer.clone()
        };

        self.create_checkpoint(PARCEL_FEE_CHECKPOINT);

        match self.apply_internal(parcel, &fee_payer, fee_payer_public) {
            Err(StateError::Transaction(err)) => unreachable!("{:?}", err),
            Err(err) => {
                self.revert_to_checkpoint(PARCEL_FEE_CHECKPOINT);
                Err(err)
            }
            Ok(outcomes) => {
                self.discard_checkpoint(PARCEL_FEE_CHECKPOINT);
                self.commit()?; // FIXME: Remove early commit.
                Ok(outcomes)
            }
        }
    }

    fn apply_internal(
        &mut self,
        parcel: &Parcel,
        fee_payer: &Address,
        fee_payer_public: &Public,
    ) -> StateResult<ParcelOutcome> {
        let nonce = self.nonce(fee_payer)?;

        if parcel.nonce != nonce {
            return Err(ParcelError::InvalidNonce {
                expected: nonce,
                got: parcel.nonce,
            }.into())
        }

        let fee = parcel.fee;
        let balance = self.balance(fee_payer)?;
        if fee > balance {
            return Err(ParcelError::InsufficientBalance {
                address: *fee_payer,
                cost: fee,
                balance,
            }.into())
        }

        self.inc_nonce(fee_payer)?;
        self.sub_balance(fee_payer, &fee)?;

        // The failed parcel also must pay the fee and increase nonce.
        self.create_checkpoint(PARCEL_ACTION_CHECKPOINT);

        match self.apply_action(&parcel.action, &parcel.network_id, fee_payer, fee_payer_public) {
            Ok(outcome) => {
                self.discard_checkpoint(PARCEL_ACTION_CHECKPOINT);
                Ok(outcome)
            }
            Err(err) => {
                self.revert_to_checkpoint(PARCEL_ACTION_CHECKPOINT);
                Err(err)
            }
        }
    }

    fn apply_action(
        &mut self,
        action: &Action,
        network_id: &u64,
        fee_payer: &Address,
        fee_payer_public: &Public,
    ) -> StateResult<ParcelOutcome> {
        match action {
            Action::ChangeShardState {
                transactions,
                changes,
                signatures: _,
            } => {
                if changes.len() == 0 {
                    return Ok(ParcelOutcome::Transactions(vec![]))
                }

                for t in transactions {
                    let transaction_network_id = t.network_id();
                    if &transaction_network_id != network_id {
                        return Err(TransactionError::InvalidNetworkId(Mismatch {
                            expected: *network_id,
                            found: transaction_network_id,
                        }).into())
                    }
                }

                let first_result = self.apply_transactions_with_check(&transactions, &changes[0])?;

                for change in changes.iter().skip(1) {
                    let result = self.apply_transactions_with_check(&transactions, change)?;
                    if result != first_result {
                        return Err(ParcelError::InconsistentShardOutcomes.into())
                    }
                }
                Ok(ParcelOutcome::Transactions(first_result))
            }
            Action::Payment {
                receiver,
                amount,
            } => match self.transfer_balance(fee_payer, receiver, amount) {
                Ok(()) => Ok(ParcelOutcome::Single {
                    invoice: Invoice::Success,
                    error: None,
                }),
                Err(StateError::Parcel(
                    err @ ParcelError::InsufficientBalance {
                        ..
                    },
                )) => Ok(ParcelOutcome::Single {
                    invoice: Invoice::Failed,
                    error: Some(err),
                }),
                Err(err) => Err(err.into()),
            },
            Action::SetRegularKey {
                key,
            } => match self.set_regular_key(fee_payer_public, key) {
                Ok(()) => Ok(ParcelOutcome::Single {
                    invoice: Invoice::Success,
                    error: None,
                }),
                Err(error) => Err(error.into()),
            },
            Action::CreateShard => {
                let shard_creation_cost = 1.into(); // FIXME: Make shard creation cost configurable
                self.create_shard(&shard_creation_cost, fee_payer)?;
                Ok(ParcelOutcome::Single {
                    invoice: Invoice::Success,
                    error: None,
                })
            }
            Action::Custom(bytes) => {
                let handlers = self.db.custom_handlers().to_vec();
                for h in handlers {
                    if let Some(result) = h.execute(bytes, self) {
                        return result
                    }
                }
                panic!("Unknown custom parcel accepted!")
            }
        }
    }

    fn apply_transactions_with_check(
        &mut self,
        transactions: &[Transaction],
        change: &ChangeShard,
    ) -> StateResult<Vec<TransactionOutcome>> {
        let shard_id = change.shard_id;

        let shard_root = self.shard_root(shard_id)?.ok_or_else(|| ParcelError::InvalidShardId(shard_id))?;
        if !change.pre_root.is_zero() && shard_root != change.pre_root {
            return Err(ParcelError::InvalidShardRoot(Mismatch {
                expected: shard_root,
                found: change.pre_root,
            }).into())
        }

        let (new_shard_root, db, results) = self.apply_transactions_internal(transactions, shard_id, shard_root)?;
        if !change.post_root.is_zero() && change.post_root != new_shard_root {
            return Err(ParcelError::InvalidShardRoot(Mismatch {
                expected: new_shard_root,
                found: change.post_root,
            }).into())
        }

        self.db = db;

        self.set_shard_root(shard_id, &shard_root, &new_shard_root)?;
        Ok(results)
    }

    pub fn apply_transactions(&self, transactions: &[Transaction], shard_id: ShardId) -> StateResult<ChangeShard> {
        let pre_root = self.shard_root(shard_id)?.ok_or_else(|| ParcelError::InvalidShardId(shard_id))?;
        let (post_root, ..) = self.apply_transactions_internal(transactions, shard_id, pre_root)?;
        Ok(ChangeShard {
            shard_id,
            pre_root,
            post_root,
        })
    }

    fn apply_transactions_internal(
        &self,
        transactions: &[Transaction],
        shard_id: ShardId,
        shard_root: H256,
    ) -> StateResult<(H256, StateDB, Vec<TransactionOutcome>)> {
        // FIXME: Make it mutable borrow db instead of cloning.
        let mut shard_level_state = ShardLevelState::from_existing(shard_id, self.db.clone(), shard_root)?;

        let mut results = Vec::with_capacity(transactions.len());
        for t in transactions {
            let result = shard_level_state.apply(t)?;
            results.push(result);
        }

        let (new_root, db) = shard_level_state.drop();
        Ok((new_root, db, results))
    }

    fn create_shard_level_state(&mut self, fee_payer: &Address) -> StateResult<()> {
        let (shard_id, shard_root, db) = {
            let mut metadata = self.require_metadata()?;
            let shard_id = metadata.increase_number_of_shards();

            let mut shard_level_state = ShardLevelState::try_new(shard_id, self.db.clone())?;

            let (shard_root, db) = shard_level_state.drop();

            (shard_id, shard_root, db)
        };

        {
            self.db = db;
        }

        ctrace!(STATE, "shard created({}, {:?})", shard_id, shard_root);

        self.set_shard_root(shard_id, &BLAKE_NULL_RLP, &shard_root)?;
        self.set_shard_owner(shard_id, &Address::zero(), *fee_payer)?;
        Ok(())
    }

    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn ensure_account_cached<F, U>(&self, a: &Address, f: F) -> TrieResult<U>
    where
        F: Fn(Option<&Account>) -> U, {
        let a = if self.regular_account_exists_and_not_null(a)? {
            let regular_account = self.require_regular_account_from_address(a)?;
            public_to_address(&regular_account.master_account())
        } else {
            a.clone()
        };

        self.ensure_master_account_cached(&a, f)
    }

    /// Same with ensure_master_account.
    /// But do not pass regular_account redirection
    fn ensure_master_account_cached<F, U>(&self, a: &Address, f: F) -> TrieResult<U>
    where
        F: Fn(Option<&Account>) -> U, {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_global_cache = |a| self.db.get_cached_account_with(a, |acc| f(acc.map(|a| &*a)));
        self.account.ensure_cached(&a, &f, db, from_global_cache)
    }

    fn require_account<'a>(&'a self, a: &Address) -> TrieResult<RefMut<'a, Account>> {
        debug_assert_eq!(Ok(false), self.regular_account_exists_and_not_null(a));

        let default = || Account::new(0u8.into(), 0.into());
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_account(&a);
        self.account.require_item_or_from(&a, default, db, from_db)
    }

    /// Check caches for required data
    /// First searches for regular account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn ensure_regular_account_cached<F, U>(&self, a: &Address, f: F) -> TrieResult<U>
    where
        F: Fn(Option<&RegularAccount>) -> U, {
        let a = RegularAccountAddress::from_address(a);
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_global_cache = |a| self.db.get_cached_regular_account_with(a, |acc| f(acc.map(|a| &*a)));
        self.regular_account.ensure_cached(&a, &f, db, from_global_cache)
    }

    fn require_regular_account<'a>(&'a self, public: &Public) -> TrieResult<RefMut<'a, RegularAccount>> {
        let regular_account_address = RegularAccountAddress::new(public);
        let default = || RegularAccount::new(Public::default());
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_regular_account(&regular_account_address);
        self.regular_account.require_item_or_from(&regular_account_address, default, db, from_db)
    }

    fn require_regular_account_from_address<'a>(&'a self, a: &Address) -> TrieResult<RefMut<'a, RegularAccount>> {
        let regular_account_address = RegularAccountAddress::from_address(a);
        let default = || RegularAccount::new(Public::default());
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_regular_account(&regular_account_address);
        self.regular_account.require_item_or_from(&regular_account_address, default, db, from_db)
    }

    fn require_metadata<'a>(&'a self) -> TrieResult<RefMut<'a, Metadata>> {
        let default = || Metadata::new(0);
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let address = MetadataAddress::new();
        let from_db = || self.db.get_cached_metadata(&address);
        self.metadata.require_item_or_from(&address, default, db, from_db)
    }

    fn require_shard<'a>(&'a self, shard_id: ShardId) -> TrieResult<RefMut<'a, Shard>> {
        let default = || Shard::new(BLAKE_NULL_RLP, Address::zero());
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let shard_address = ShardAddress::new(shard_id);
        let from_db = || self.db.get_cached_shard(&shard_address);
        self.shard.require_item_or_from(&shard_address, default, db, from_db)
    }

    fn require_action_data<'a>(&'a self, key: &H256) -> TrieResult<RefMut<'a, Bytes>> {
        let default = || NULL_RLP.to_vec();
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_action_data(key);
        self.action_data.require_item_or_from(key, default, db, from_db)
    }
}

impl fmt::Debug for TopLevelState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "account: {:?}", self.account)?;
        writeln!(f, "regular_account: {:?}", self.regular_account)?;
        writeln!(f, "metadata: {:?}", self.metadata)?;
        writeln!(f, "shard: {:?}", self.shard)?;
        writeln!(f, "action_data: {:?}", self.action_data)?;
        Ok(())
    }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for TopLevelState {
    fn clone(&self) -> TopLevelState {
        TopLevelState {
            db: self.db.clone(),
            root: self.root.clone(),
            id_of_checkpoints: self.id_of_checkpoints.clone(),
            account: self.account.clone(),
            regular_account: self.regular_account.clone(),
            metadata: self.metadata.clone(),
            shard: self.shard.clone(),
            action_data: self.action_data.clone(),
        }
    }
}

impl TopState<StateDB> for TopLevelState {
    fn kill_account(&mut self, account: &Address) {
        self.account.remove(account);
    }

    fn kill_regular_account(&mut self, account: &Public) {
        self.regular_account.remove(&RegularAccountAddress::new(account));
    }

    fn account_exists(&self, a: &Address) -> TrieResult<bool> {
        // Bloom filter does not contain empty accounts, so it is important here to
        // check if account exists in the database directly before EIP-161 is in effect.
        self.ensure_account_cached(a, |a| a.is_some())
    }

    fn account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool> {
        self.ensure_account_cached(a, |a| a.map_or(false, |a| !a.is_null()))
    }

    fn account_exists_and_has_nonce(&self, a: &Address) -> TrieResult<bool> {
        self.ensure_account_cached(a, |a| a.map_or(false, |a| !a.nonce().is_zero()))
    }

    fn master_account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool> {
        self.ensure_master_account_cached(a, |a| a.map_or(false, |a| !a.is_null()))
    }

    fn regular_account_exists_and_not_null(&self, a: &Address) -> TrieResult<bool> {
        self.ensure_regular_account_cached(a, |a| a.map_or(false, |a| !a.is_null()))
    }

    fn add_balance(&mut self, a: &Address, incr: &U256) -> TrieResult<()> {
        ctrace!(STATE, "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        let is_value_transfer = !incr.is_zero();
        if is_value_transfer {
            self.require_account(a)?.add_balance(incr);
        }
        Ok(())
    }

    fn sub_balance(&mut self, a: &Address, decr: &U256) -> TrieResult<()> {
        ctrace!(STATE, "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
        if !decr.is_zero() || !self.account_exists(a)? {
            self.require_account(a)?.sub_balance(decr);
        }
        Ok(())
    }

    fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) -> StateResult<()> {
        let balance = self.balance(from)?;
        if &balance < by {
            return Err(ParcelError::InsufficientBalance {
                address: *from,
                cost: *by,
                balance,
            }.into())
        }
        if self.regular_account_exists_and_not_null(to)? {
            return Err(ParcelError::InvalidTransferDestination.into())
        }
        self.sub_balance(from, by)?;
        self.add_balance(to, by)?;
        Ok(())
    }

    fn inc_nonce(&mut self, a: &Address) -> TrieResult<()> {
        self.require_account(a)?.inc_nonce();
        Ok(())
    }

    fn set_regular_key(&mut self, master_public: &Public, regular_key: &Public) -> StateResult<()> {
        let master_address = public_to_address(master_public);

        let (master_public, master_address) = if self.regular_account_exists_and_not_null(&master_address)? {
            let regular_account = self.require_regular_account_from_address(&master_address)?;
            let master_public = regular_account.master_account().clone();
            let master_address = public_to_address(&master_public);
            (master_public, master_address)
        } else {
            (*master_public, public_to_address(&master_public))
        };

        let regular_address = public_to_address(regular_key);
        if self.regular_account_exists_and_not_null(&regular_address)? {
            return Err(ParcelError::RegularKeyAlreadyInUse.into())
        }

        if self.master_account_exists_and_not_null(&regular_address)? {
            return Err(ParcelError::RegularKeyAlreadyInUseAsMaster.into())
        }

        let prev_regular_key = self.require_account(&master_address)?.regular_key();

        if let Some(prev_regular_key) = prev_regular_key {
            self.kill_regular_account(&prev_regular_key);
        }

        let mut master_account = self.require_account(&master_address)?;
        master_account.set_regular_key(regular_key);
        self.require_regular_account(&regular_key)?.set_master_account(&master_public);
        Ok(())
    }

    fn create_shard(&mut self, shard_creation_cost: &U256, fee_payer: &Address) -> StateResult<()> {
        let balance = self.balance(fee_payer)?;
        if &balance < shard_creation_cost {
            return Err(ParcelError::InsufficientBalance {
                address: *fee_payer,
                cost: *shard_creation_cost,
                balance,
            }.into())
        }
        self.sub_balance(fee_payer, shard_creation_cost)?;

        self.create_shard_level_state(fee_payer)?;

        Ok(())
    }

    fn set_shard_root(&mut self, shard_id: ShardId, old_root: &H256, new_root: &H256) -> StateResult<()> {
        let mut shard = self.require_shard(shard_id)?;
        assert_eq!(old_root, shard.root());
        shard.set_root(*new_root);
        Ok(())
    }

    fn set_shard_owner(&mut self, shard_id: ShardId, old_owner: &Address, new_owner: Address) -> StateResult<()> {
        let mut shard = self.require_shard(shard_id)?;
        assert_eq!(old_owner, shard.owner());
        shard.set_owner(new_owner);
        Ok(())
    }

    fn update_action_data(&mut self, key: &H256, data: Bytes) -> StateResult<()> {
        let mut action_data = self.require_action_data(key)?;
        *action_data = data;
        Ok(())
    }
}

#[cfg(test)]
mod tests_state {
    use ccrypto::BLAKE_NULL_RLP;
    use ckey::Address;
    use primitives::U256;

    use super::super::super::tests::helpers::{get_temp_state, get_temp_state_db};
    use super::*;

    #[test]
    fn should_work_when_cloned() {
        let a = Address::zero();

        let mut state = {
            let mut state = get_temp_state();
            assert_eq!(Ok(false), state.account_exists(&a));
            assert_eq!(Ok(()), state.inc_nonce(&a));
            assert_eq!(Ok(()), state.commit());
            state.clone()
        };

        assert_eq!(Ok(()), state.inc_nonce(&a));
        assert_eq!(Ok(()), state.commit());
    }


    #[test]
    fn state_is_not_synchronized_when_cloned() {
        let a = Address::random();

        let original_state = get_temp_state();

        assert_eq!(Ok(false), original_state.account_exists(&a));

        let mut cloned_state = original_state.clone();

        assert_eq!(Ok(()), cloned_state.inc_nonce(&a));
        assert_eq!(Ok(()), cloned_state.commit());

        assert_ne!(original_state.nonce(&a), cloned_state.nonce(&a));
    }

    #[test]
    fn get_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state();
            assert_eq!(Ok(()), state.inc_nonce(&a));
            assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
            assert_eq!(Ok(()), state.commit());
            assert_eq!(Ok(69.into()), state.balance(&a));
            state.drop()
        };

        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(69.into()), state.balance(&a));
        assert_eq!(Ok(1.into()), state.nonce(&a));
    }

    #[test]
    fn remove() {
        let a = Address::zero();
        let mut state = get_temp_state();
        assert_eq!(Ok(false), state.account_exists(&a));
        assert_eq!(Ok(false), state.account_exists_and_not_null(&a));
        assert_eq!(Ok(()), state.inc_nonce(&a));
        assert_eq!(Ok(true), state.account_exists(&a));
        assert_eq!(Ok(true), state.account_exists_and_not_null(&a));
        assert_eq!(Ok(1.into()), state.nonce(&a));
        state.kill_account(&a);
        assert_eq!(Ok(false), state.account_exists(&a));
        assert_eq!(Ok(false), state.account_exists_and_not_null(&a));
        assert_eq!(Ok(0.into()), state.nonce(&a));
    }

    #[test]
    fn empty_account_is_not_created() {
        let a = Address::zero();
        let db = get_temp_state_db();
        let (root, db) = {
            let mut state = TopLevelState::new(db);
            assert_eq!(Ok(()), state.add_balance(&a, &U256::default())); // create an empty account
            assert_eq!(Ok(()), state.commit());
            state.drop()
        };
        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(false), state.account_exists(&a));
        assert_eq!(Ok(false), state.account_exists_and_not_null(&a));
    }

    #[test]
    fn remove_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state();
            assert_eq!(Ok(()), state.inc_nonce(&a));
            assert_eq!(Ok(()), state.commit());
            assert_eq!(Ok(true), state.account_exists(&a));
            assert_eq!(Ok(1.into()), state.nonce(&a));
            state.drop()
        };

        let (root, db) = {
            let mut state = TopLevelState::from_existing(db, root).unwrap();
            assert_eq!(Ok(true), state.account_exists(&a));
            assert_eq!(Ok(1.into()), state.nonce(&a));
            state.kill_account(&a);
            assert_eq!(Ok(()), state.commit());
            assert_eq!(Ok(false), state.account_exists(&a));
            assert_eq!(Ok(0.into()), state.nonce(&a));
            state.drop()
        };

        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(false), state.account_exists(&a));
        assert_eq!(Ok(0.into()), state.nonce(&a));
    }

    #[test]
    fn alter_balance() {
        let mut state = get_temp_state();
        let a = Address::zero();
        let b = 1u64.into();
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        assert_eq!(Ok(69.into()), state.balance(&a));
        assert_eq!(Ok(()), state.commit());
        assert_eq!(Ok(69.into()), state.balance(&a));
        assert_eq!(Ok(()), state.sub_balance(&a, &U256::from(42u64)));
        assert_eq!(Ok(27.into()), state.balance(&a));
        assert_eq!(Ok(()), state.commit());
        assert_eq!(Ok(27.into()), state.balance(&a));
        assert_eq!(Ok(()), state.transfer_balance(&a, &b, &U256::from(18u64)));
        assert_eq!(Ok(9.into()), state.balance(&a));
        assert_eq!(Ok(18.into()), state.balance(&b));
        assert_eq!(Ok(()), state.commit());
        assert_eq!(Ok(9.into()), state.balance(&a));
        assert_eq!(Ok(18.into()), state.balance(&b));
    }

    #[test]
    fn alter_nonce() {
        let mut state = get_temp_state();
        let a = Address::zero();
        assert_eq!(Ok(()), state.inc_nonce(&a));
        assert_eq!(Ok(1.into()), state.nonce(&a));
        assert_eq!(Ok(()), state.inc_nonce(&a));
        assert_eq!(Ok(2.into()), state.nonce(&a));
        assert_eq!(Ok(()), state.commit());
        assert_eq!(Ok(2.into()), state.nonce(&a));
        assert_eq!(Ok(()), state.inc_nonce(&a));
        assert_eq!(Ok(3.into()), state.nonce(&a));
        assert_eq!(Ok(()), state.commit());
        assert_eq!(Ok(3.into()), state.nonce(&a));
    }

    #[test]
    fn balance_nonce() {
        let mut state = get_temp_state();
        let a = Address::zero();
        assert_eq!(Ok(0.into()), state.balance(&a));
        assert_eq!(Ok(0.into()), state.nonce(&a));
        assert_eq!(Ok(()), state.commit());
        assert_eq!(Ok(0.into()), state.balance(&a));
        assert_eq!(Ok(0.into()), state.nonce(&a));
    }

    #[test]
    fn ensure_cached() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.require_account(&a).unwrap();
        assert_eq!(Ok(()), state.commit());
        assert_eq!(*state.root(), "db4046bb91a12a37cbfb0f09631aad96a97248423163eca791e19b430cc7fe4a".into());
    }

    #[test]
    fn checkpoint_basic() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.create_checkpoint(0);
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        assert_eq!(Ok(69.into()), state.balance(&a));
        state.discard_checkpoint(0);
        assert_eq!(Ok(69.into()), state.balance(&a));
        state.create_checkpoint(1);
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(1u64)));
        assert_eq!(Ok(70.into()), state.balance(&a));
        state.revert_to_checkpoint(1);
        assert_eq!(Ok(69.into()), state.balance(&a));
    }

    #[test]
    fn checkpoint_nested() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.create_checkpoint(0);
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        state.create_checkpoint(1);
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        assert_eq!(Ok((69 + 69).into()), state.balance(&a));
        state.revert_to_checkpoint(1);
        assert_eq!(Ok(69.into()), state.balance(&a));
        state.revert_to_checkpoint(0);
        assert_eq!(Ok(0.into()), state.balance(&a));
    }

    #[test]
    fn checkpoint_discard() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.create_checkpoint(0);
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        state.create_checkpoint(1);
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        assert_eq!(Ok(()), state.inc_nonce(&a));
        assert_eq!(Ok((69 + 69).into()), state.balance(&a));
        assert_eq!(Ok(1.into()), state.nonce(&a));
        state.discard_checkpoint(1);
        assert_eq!(Ok((69 + 69).into()), state.balance(&a));
        assert_eq!(Ok(1.into()), state.nonce(&a));
        state.revert_to_checkpoint(0);
        assert_eq!(Ok(0.into()), state.balance(&a));
        assert_eq!(Ok(0.into()), state.nonce(&a));
    }

    #[test]
    fn create_empty() {
        let mut state = get_temp_state();
        state.commit().unwrap();
        assert_eq!(state.root(), &BLAKE_NULL_RLP);
    }
}

#[cfg(test)]
mod tests_parcel {
    use ckey::{Address, Generator, Random};
    use ctypes::parcel::Parcel;
    use ctypes::transaction::{AssetMintOutput, AssetOutPoint, AssetTransferInput, AssetTransferOutput, Transaction};
    use primitives::U256;

    use super::super::super::tests::helpers::get_temp_state;
    use super::*;

    fn address() -> (Address, Public) {
        let keypair = Random.generate().unwrap();
        (keypair.address(), keypair.public().clone())
    }

    #[test]
    fn apply_empty_parcel() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(&sender));
        assert_eq!(Ok(()), state.commit());

        let parcel = Parcel {
            fee: 5.into(),
            nonce: 0.into(),
            network_id: 0xCA,
            action: Action::ChangeShardState {
                transactions: vec![],
                changes: vec![],
                signatures: vec![],
            },
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));

        let result = state.apply(&parcel, &sender, &sender_public);

        assert_eq!(Ok(ParcelOutcome::Transactions(vec![])), result);
        assert_eq!(Ok(15.into()), state.balance(&sender));
        assert_eq!(Ok(1.into()), state.nonce(&sender));
    }

    #[test]
    fn should_apply_error_for_invalid_nonce() {
        let mut state = get_temp_state();

        let parcel = Parcel {
            nonce: 2.into(),
            fee: 5.into(),
            network_id: 0xCA,
            action: Action::ChangeShardState {
                transactions: vec![],
                changes: vec![],
                signatures: vec![],
            },
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));

        let result = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(
            Err(StateError::Parcel(ParcelError::InvalidNonce {
                expected: 0.into(),
                got: 2.into()
            })),
            result
        );

        assert_eq!(Ok(20.into()), state.balance(&sender));
        assert_eq!(Ok(0.into()), state.nonce(&sender));
    }

    #[test]
    fn should_apply_error_for_not_enough_cash() {
        let mut state = get_temp_state();
        let parcel = Parcel {
            fee: 5.into(),
            nonce: 0.into(),
            network_id: 0xCA,
            action: Action::ChangeShardState {
                transactions: vec![],
                changes: vec![],
                signatures: vec![],
            },
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &4.into()));

        let result = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(
            Err(StateError::Parcel(ParcelError::InsufficientBalance {
                address: sender,
                balance: 4.into(),
                cost: 5.into(),
            })),
            result
        );
        assert_eq!(Ok(4.into()), state.balance(&sender));
        assert_eq!(Ok(0.into()), state.nonce(&sender));
    }

    #[test]
    fn should_apply_payment() {
        let mut state = get_temp_state();
        let receiver = 1u64.into();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::Payment {
                receiver,
                amount: 10.into(),
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));

        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None,
            }),
            state.apply(&parcel, &sender, &sender_public)
        );

        assert_eq!(Ok(10.into()), state.balance(&receiver));
        assert_eq!(Ok(5.into()), state.balance(&sender));
        assert_eq!(Ok(1.into()), state.nonce(&sender));
    }

    #[test]
    fn should_apply_set_regular_key() {
        let mut state = get_temp_state();
        let key = 1u64.into();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key,
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &5.into()));

        assert_eq!(state.regular_key(&sender), Ok(None));
        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None
            }),
            state.apply(&parcel, &sender, &sender_public)
        );
        assert_eq!(Ok(Some(key)), state.regular_key(&sender));
    }

    #[test]
    fn should_use_master_balance_when_signed_with_regular_key() {
        let mut state = get_temp_state();
        let regular_keypair = Random.generate().unwrap();
        let key = regular_keypair.public();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: key.clone(),
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &15.into()));

        assert_eq!(state.regular_key(&sender), Ok(None));
        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None
            }),
            state.apply(&parcel, &sender, &sender_public)
        );
        assert_eq!(Ok(Some(*key)), state.regular_key(&sender));

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            nonce: 1.into(),
            network_id: 0xCA,
        };

        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None
            }),
            state.apply(&parcel, &regular_keypair.address(), regular_keypair.public())
        );
        assert_eq!(Ok(4.into()), state.balance(&sender));
        assert_eq!(Ok(Some(sender)), state.shard_owner(0));
    }

    #[test]
    fn should_fail_when_two_accounts_used_the_same_regular_key() {
        let mut state = get_temp_state();
        let regular_keypair = Random.generate().unwrap();
        let key = regular_keypair.public();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: key.clone(),
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &15.into()));

        assert_eq!(state.regular_key(&sender), Ok(None));
        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None
            }),
            state.apply(&parcel, &sender, &sender_public)
        );
        assert_eq!(Ok(Some(*key)), state.regular_key(&sender));

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: key.clone(),
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender2, sender_public2) = address();
        assert_eq!(Ok(()), state.add_balance(&sender2, &15.into()));

        let result = state.apply(&parcel, &sender2, &sender_public2);
        assert_eq!(Err(StateError::Parcel(ParcelError::RegularKeyAlreadyInUse)), result);
        assert_eq!(Ok(None), state.regular_key(&sender2));
    }

    #[test]
    fn should_fail_when_regular_key_is_already_registered_as_master_key() {
        let (sender, sender_public) = address();
        let (sender2, sender_public2) = address();

        let mut state = get_temp_state();

        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        assert_eq!(Ok(()), state.add_balance(&sender2, &20.into()));

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: sender_public2.clone(),
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };

        let result = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(Err(StateError::Parcel(ParcelError::RegularKeyAlreadyInUseAsMaster)), result);
    }

    #[test]
    fn should_be_able_to_change_regular_key() {
        let (sender, sender_public) = address();
        let (regular_address, regular_public) = address();
        let (_, regular_public2) = address();

        let mut state = get_temp_state();

        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        assert_eq!(Ok(()), state.set_regular_key(&sender_public, &regular_public));

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: regular_public2,
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };

        assert_eq!(Some(regular_public), state.regular_key(&sender).unwrap());
        assert_eq!(Ok(true), state.regular_account_exists_and_not_null(&regular_address));
        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None
            }),
            state.apply(&parcel, &regular_address, &regular_public)
        );
        assert_eq!(Ok(false), state.regular_account_exists_and_not_null(&regular_address));
        assert_eq!(Some(regular_public2), state.regular_key(&sender).unwrap());
    }

    #[test]
    fn should_be_able_to_use_deleted_regular_key_as_master_key() {
        let (sender, sender_public) = address();
        let (regular_address, regular_public) = address();
        let (_, regular_public2) = address();

        let mut state = get_temp_state();

        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        assert_eq!(Ok(()), state.set_regular_key(&sender_public, &regular_public));
        assert_eq!(Ok(()), state.set_regular_key(&sender_public, &regular_public2));

        assert_eq!(Ok(false), state.regular_account_exists_and_not_null(&regular_address));
        assert_eq!(Ok(()), state.add_balance(&regular_address, &20.into()));

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            nonce: 0.into(),
            network_id: 0xCA,
        };
        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None
            }),
            state.apply(&parcel, &regular_address, &regular_public)
        );
        assert_eq!(Ok(14.into()), state.balance(&regular_address));
        assert_eq!(Ok(20.into()), state.balance(&sender));
        assert_eq!(Ok(Some(regular_address)), state.shard_owner(0));
    }

    #[test]
    fn should_fail_when_someone_sends_some_ccc_to_an_address_which_used_as_a_regular_key() {
        let (sender, sender_public) = address();
        let (regular_address, regular_public) = address();

        let mut state = get_temp_state();

        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        assert_eq!(Ok(()), state.set_regular_key(&sender_public, &regular_public));

        let parcel = Parcel {
            action: Action::Payment {
                receiver: regular_address,
                amount: 5.into(),
            },
            fee: 5.into(),
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let result = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(Err(StateError::Parcel(ParcelError::InvalidTransferDestination)), result);
        assert_eq!(Ok(20.into()), state.balance(&sender));
    }

    #[test]
    fn should_apply_error_for_action_failure() {
        let mut state = get_temp_state();
        let receiver = 1u64.into();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::Payment {
                receiver,
                amount: 30.into(),
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));

        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Failed,
                error: Some(ParcelError::InsufficientBalance {
                    address: sender,
                    balance: 15.into(),
                    cost: 30.into(),
                })
            }),
            state.apply(&parcel, &sender, &sender_public)
        );

        assert_eq!(Ok(0.into()), state.balance(&receiver));
        assert_eq!(Ok(15.into()), state.balance(&sender));
        assert_eq!(Ok(1.into()), state.nonce(&sender));
    }

    #[test]
    fn mint_permissioned_asset() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(&sender));
        assert_eq!(Ok(()), state.commit());

        let shard_id = 0x0;

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let amount = 30;
        let transaction = Transaction::AssetMint {
            network_id: 0xCA,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: Some(amount),
            },
            registrar,
            nonce: 0,
        };
        let transaction_hash = transaction.hash();
        let transactions = vec![transaction];
        let parcel = Parcel {
            fee: 11.into(),
            action: Action::ChangeShardState {
                transactions,
                changes: vec![ChangeShard {
                    shard_id,
                    pre_root: H256::from("0xa8ed01b49cd63c6a547ac3ce357539aa634fb44331a351e3e98b9f1c3a8e3edf"),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));

        assert_eq!(
            Ok(ParcelOutcome::Transactions(vec![TransactionOutcome {
                invoice: Invoice::Success,
                error: None,
            }])),
            state.apply(&parcel, &sender, &sender_public)
        );

        assert_eq!(state.balance(&sender), Ok(58.into()));
        assert_eq!(state.nonce(&sender), Ok(1.into()));

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset_address = AssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(Some(Asset::new(asset_scheme_address.into(), lock_script_hash, parameters, amount))), asset);
    }

    #[test]
    fn mint_infinite_permissioned_asset() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(&sender));
        assert_eq!(Ok(()), state.commit());

        let shard_id = 0;

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: 0xCA,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            registrar,
            nonce: 0,
        };
        let transaction_hash = transaction.hash();
        let transactions = vec![transaction];
        let parcel = Parcel {
            fee: 5.into(),
            action: Action::ChangeShardState {
                transactions,
                changes: vec![ChangeShard {
                    shard_id,
                    pre_root: H256::from("0xa8ed01b49cd63c6a547ac3ce357539aa634fb44331a351e3e98b9f1c3a8e3edf"),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
            nonce: 0.into(),
            network_id: 0xCA,
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));

        assert_eq!(
            Ok(ParcelOutcome::Transactions(vec![TransactionOutcome {
                invoice: Invoice::Success,
                error: None,
            }])),
            state.apply(&parcel, &sender, &sender_public)
        );

        assert_eq!(state.balance(&sender), Ok(64.into()));
        assert_eq!(state.nonce(&sender), Ok(1.into()));

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, registrar))), asset_scheme);

        let asset_address = AssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(
            Ok(Some(Asset::new(asset_scheme_address.into(), lock_script_hash, parameters, ::std::u64::MAX))),
            asset
        );
    }

    #[test]
    fn mint_and_transfer_in_the_same_parcel() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(&sender));
        assert_eq!(Ok(()), state.commit());

        let shard_id = 0x00;
        let network_id = 0xBeef;

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050");
        let registrar = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.clone().into();
        let asset_address = AssetAddress::new(mint_hash, 0, shard_id);

        let random_lock_script_hash = H256::random();
        let transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 30,
                },
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![vec![1]],
                    asset_type,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash: random_lock_script_hash,
                    parameters: vec![],
                    asset_type,
                    amount: 15,
                },
            ],
            nonce: 0,
        };
        let transfer_hash = transfer.hash();


        let transactions = vec![mint, transfer];
        let parcel = Parcel {
            fee: 20.into(),
            nonce: 0.into(),
            network_id,
            action: Action::ChangeShardState {
                transactions,
                changes: vec![ChangeShard {
                    shard_id,
                    pre_root: H256::from("0xa8ed01b49cd63c6a547ac3ce357539aa634fb44331a351e3e98b9f1c3a8e3edf"),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(120)));

        assert_eq!(
            ParcelOutcome::Transactions(vec![
                TransactionOutcome {
                    invoice: Invoice::Success,
                    error: None,
                },
                TransactionOutcome {
                    invoice: Invoice::Success,
                    error: None,
                },
            ]),
            state.apply(&parcel, &sender, &sender_public).unwrap()
        );

        assert_eq!(state.balance(&sender), Ok(100.into()));
        assert_eq!(state.nonce(&sender), Ok(1.into()));

        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(None), asset);

        let asset0_address = AssetAddress::new(transfer_hash, 0, shard_id);
        let asset0 = state.asset(shard_id, &asset0_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = AssetAddress::new(transfer_hash, 1, shard_id);
        let asset1 = state.asset(shard_id, &asset1_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], 5))), asset1);

        let asset2_address = AssetAddress::new(transfer_hash, 2, shard_id);
        let asset2 = state.asset(shard_id, &asset2_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);
    }

    #[test]
    fn mint_and_transfer_in_different_parcel() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(&sender));
        assert_eq!(Ok(()), state.commit());

        let network_id = 0xBeef;
        let shard_id = 0x00;

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050");
        let registrar = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();

        let mint_parcel = Parcel {
            fee: 20.into(),
            network_id,
            nonce: 0.into(),
            action: Action::ChangeShardState {
                transactions: vec![mint],
                changes: vec![ChangeShard {
                    shard_id,
                    pre_root: H256::from("0xa8ed01b49cd63c6a547ac3ce357539aa634fb44331a351e3e98b9f1c3a8e3edf"),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(120)));

        assert_eq!(
            Ok(ParcelOutcome::Transactions(vec![TransactionOutcome {
                invoice: Invoice::Success,
                error: None,
            }])),
            state.apply(&mint_parcel, &sender, &sender_public)
        );
        assert_eq!(state.balance(&sender), Ok(100.into()));
        assert_eq!(state.nonce(&sender), Ok(1.into()));

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.clone().into();
        let asset_address = AssetAddress::new(mint_hash, 0, shard_id);

        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], 30))), asset);

        let random_lock_script_hash = H256::random();
        let transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 30,
                },
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![vec![1]],
                    asset_type,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash: random_lock_script_hash,
                    parameters: vec![],
                    asset_type,
                    amount: 15,
                },
            ],
            nonce: 0,
        };
        let transfer_hash = transfer.hash();

        let transfer_parcel = Parcel {
            fee: 30.into(),
            network_id,
            nonce: 1.into(),
            action: Action::ChangeShardState {
                transactions: vec![transfer],
                changes: vec![ChangeShard {
                    shard_id,
                    pre_root: H256::from("0x3e56ec350ed779120d7b27472aef08fefa0e72165206275efa0b4c8419ba26db"),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
        };

        assert_eq!(
            Ok(ParcelOutcome::Transactions(vec![TransactionOutcome {
                invoice: Invoice::Success,
                error: None,
            }])),
            state.apply(&transfer_parcel, &sender, &sender_public)
        );

        assert_eq!(state.balance(&sender), Ok(70.into()));
        assert_eq!(state.nonce(&sender), Ok(2.into()));

        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(None), asset);

        let asset0_address = AssetAddress::new(transfer_hash, 0, shard_id);
        let asset0 = state.asset(shard_id, &asset0_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = AssetAddress::new(transfer_hash, 1, shard_id);
        let asset1 = state.asset(shard_id, &asset1_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], 5))), asset1);

        let asset2_address = AssetAddress::new(transfer_hash, 2, shard_id);
        let asset2 = state.asset(shard_id, &asset2_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);
    }

    #[test]
    fn get_invalid_shard_root() {
        let state = get_temp_state();

        let shard_id = 3;
        assert_eq!(Ok(None), state.shard_root(shard_id));
    }

    #[test]
    fn get_asset_in_invalid_shard() {
        let state = get_temp_state();

        let shard_id = 3;
        assert_eq!(Ok(None), state.asset(shard_id, &AssetAddress::new(H256::random(), 0, shard_id)));
    }


    #[test]
    fn get_asset_scheme_in_invalid_shard() {
        let state = get_temp_state();

        let shard_id = 3;
        assert_eq!(Ok(None), state.asset_scheme(shard_id, &AssetSchemeAddress::new(H256::random(), shard_id)));
    }

    #[test]
    fn apply_create_shard() {
        let mut state = get_temp_state();

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        let res = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None,
            }),
            res
        );
        assert_eq!(Ok(14.into()), state.balance(&sender));
        assert_eq!(Ok(1.into()), state.nonce(&sender));
        assert_ne!(Ok(None), state.shard_root(0));
        assert_ne!(Ok(None), state.shard_root(0));
        assert_eq!(Ok(Some(sender)), state.shard_owner(0));
    }

    #[test]
    fn get_asset_in_invalid_shard2() {
        let mut state = get_temp_state();

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        let res = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None,
            }),
            res
        );
        assert_eq!(Ok(14.into()), state.balance(&sender));
        assert_eq!(Ok(1.into()), state.nonce(&sender));
        assert_eq!(Ok(Some(sender)), state.shard_owner(0));

        let shard_id = 3;
        assert_eq!(Ok(None), state.asset(shard_id, &AssetAddress::new(H256::random(), 0, shard_id)));
    }

    #[test]
    fn get_asset_scheme_in_invalid_shard2() {
        let mut state = get_temp_state();

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            nonce: 0.into(),
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        let res = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(
            Ok(ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None,
            }),
            res
        );
        assert_eq!(Ok(14.into()), state.balance(&sender));
        assert_eq!(Ok(1.into()), state.nonce(&sender));
        assert_eq!(Ok(Some(sender)), state.shard_owner(0));

        let shard_id = 3;
        assert_eq!(Ok(None), state.asset_scheme(shard_id, &AssetSchemeAddress::new(H256::random(), shard_id)));
    }

    #[test]
    fn mint_asset_on_invalid_parcel_must_fail() {
        let mut state = get_temp_state();

        let shard_id = 0;
        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let amount = 30;
        let transaction = Transaction::AssetMint {
            network_id: 0xCA,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: Some(amount),
            },
            registrar,
            nonce: 0,
        };
        let transactions = vec![transaction];
        let parcel = Parcel {
            fee: 11.into(),
            nonce: 0.into(),
            action: Action::ChangeShardState {
                transactions,
                changes: vec![ChangeShard {
                    shard_id,
                    pre_root: H256::zero(),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
            network_id: 0xCA,
        };
        let (sender, sender_public) = address();

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));

        let res = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(Err(StateError::Parcel(ParcelError::InvalidShardId(0))), res);
    }

    #[test]
    fn transfer_on_invalid_parcel_must_fail() {
        let mut state = get_temp_state();

        let network_id = 0xBeef;
        let shard_id = 100;

        let asset_type = AssetSchemeAddress::new(H256::zero(), shard_id).into();
        let transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount: 30,
                },
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![vec![1]],
                    asset_type,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type,
                    amount: 15,
                },
            ],
            nonce: 0,
        };

        let parcel = Parcel {
            fee: 30.into(),
            network_id,
            nonce: 0.into(),
            action: Action::ChangeShardState {
                transactions: vec![transfer],
                changes: vec![ChangeShard {
                    shard_id,
                    pre_root: H256::zero(),
                    post_root: H256::zero(),
                }],
                signatures: vec![],
            },
        };

        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(120)));

        let res = state.apply(&parcel, &sender, &sender_public);
        assert_eq!(Err(StateError::Parcel(ParcelError::InvalidShardId(100))), res);
    }
}
