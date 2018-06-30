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
use ctypes::{Address, H256, Public, U256};
use error::Error;
use parcel::{Action, SignedParcel};
use trie::{Result as TrieResult, Trie, TrieError, TrieFactory};

use super::super::invoice::Invoice;
use super::super::parcel::ParcelError;
use super::super::state_db::StateDB;
use super::super::transaction::Transaction;
use super::account::Account;
use super::asset::{Asset, AssetAddress};
use super::asset_scheme::{AssetScheme, AssetSchemeAddress};
use super::backend::{Backend, ShardBackend, TopBackend};
use super::cache::{Cache, CacheableItem};
use super::info::{ShardStateInfo, TopStateInfo};
use super::shard::{Shard, ShardAddress};
use super::shard_level::ShardLevelState;
use super::shard_state::{ShardState, TransactionOutcome};
use super::top_state::TopState;
use super::traits::{CheckpointId, StateWithCache, StateWithCheckpoint};

/// Used to return information about an `State::apply` operation.
#[derive(Debug, PartialEq)]
pub enum ParcelOutcome {
    Single {
        invoice: Invoice,
        error: Option<ParcelError>,
    },
    Transactions(Vec<TransactionOutcome>),
}

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
pub struct TopLevelState<B> {
    db: B,
    root: H256,
    account: Cache<Account>,
    shard: Cache<Shard>,
    id_of_checkpoints: Vec<CheckpointId>,
    trie_factory: TrieFactory,
}

impl<B: Backend + TopBackend + ShardBackend + Clone> TopStateInfo for TopLevelState<B> {
    fn nonce(&self, a: &Address) -> TrieResult<U256> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or_else(U256::zero, |account| *account.nonce()))
    }
    fn balance(&self, a: &Address) -> TrieResult<U256> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or_else(U256::zero, |account| *account.balance()))
    }
    fn regular_key(&self, a: &Address) -> TrieResult<Option<Public>> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or(None, |account| account.regular_key()))
    }

    fn shard_root(&self, a: &ShardAddress) -> TrieResult<Option<H256>> {
        let shard = self.db.get_cached_shard(&a).and_then(|s| s).map(|s| s.root().clone());
        if shard.is_some() {
            return Ok(shard)
        }

        // because of lexical borrow of self.db
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        Ok(db.get_with(&a, Shard::from_rlp)?.map(|s| s.root().clone()))
    }

    fn asset_scheme(
        &self,
        shard_id: u32,
        asset_scheme_address: &AssetSchemeAddress,
    ) -> TrieResult<Option<AssetScheme>> {
        // FIXME: Handle the case that shard doesn't exist
        let shard_root = self.shard_root(&ShardAddress::new(shard_id))?.unwrap_or(BLAKE_NULL_RLP);
        // FIXME: Make it mutable borrow db instead of cloning.
        let shard_level_state = ShardLevelState::from_existing(self.db.clone(), shard_root, self.trie_factory)?;
        shard_level_state.asset_scheme(asset_scheme_address)
    }

    fn asset(&self, shard_id: u32, asset_address: &AssetAddress) -> TrieResult<Option<Asset>> {
        // FIXME: Handle the case that shard doesn't exist
        let shard_root = self.shard_root(&ShardAddress::new(shard_id))?.unwrap_or(BLAKE_NULL_RLP);
        // FIXME: Make it mutable borrow db instead of cloning.
        let shard_level_state = ShardLevelState::from_existing(self.db.clone(), shard_root, self.trie_factory)?;
        shard_level_state.asset(asset_address)
    }
}

const PARCEL_FEE_CHECKPOINT: CheckpointId = 123;
const PARCEL_ACTION_CHECKPOINT: CheckpointId = 130;

impl<B: Backend + TopBackend> StateWithCheckpoint for TopLevelState<B> {
    fn create_checkpoint(&mut self, id: CheckpointId) {
        self.id_of_checkpoints.push(id);
        self.account.checkpoint();
        self.shard.checkpoint();
    }

    fn discard_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.account.discard_checkpoint();
        self.shard.discard_checkpoint();
    }

    fn revert_to_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.account.revert_to_checkpoint();
        self.shard.revert_to_checkpoint();
    }
}

impl<B: Backend + TopBackend + ShardBackend> StateWithCache for TopLevelState<B> {
    fn commit(&mut self) -> TrieResult<()> {
        let mut trie = self.trie_factory.from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
        self.account.commit(&mut trie)?;
        self.shard.commit(&mut trie)?;
        Ok(())
    }

    fn propagate_to_global_cache(&mut self) {
        let ref mut db = self.db;
        self.account.propagate_to_global_cache(|address, item, modified| {
            db.add_to_account_cache(address, item, modified);
        });
        self.shard.propagate_to_global_cache(|address, item, modified| {
            db.add_to_shard_cache(address, item, modified);
        });
    }

    fn clear(&mut self) {
        self.account.clear();
        self.shard.clear();
    }
}

impl<B: Backend + TopBackend + ShardBackend + Clone> TopLevelState<B> {
    /// Creates new state with empty state root
    /// Used for tests.
    #[cfg(test)]
    pub fn new(mut db: B, trie_factory: TrieFactory) -> TopLevelState<B> {
        let mut root = H256::new();

        // init trie and reset root too null
        let _ = trie_factory.create(db.as_hashdb_mut(), &mut root);

        TopLevelState {
            db,
            root,
            account: Cache::new(),
            shard: Cache::new(),
            id_of_checkpoints: Default::default(),
            trie_factory,
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(db: B, root: H256, trie_factory: TrieFactory) -> Result<TopLevelState<B>, TrieError> {
        if !db.as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        let state = TopLevelState {
            db,
            root,
            account: Cache::new(),
            shard: Cache::new(),
            id_of_checkpoints: Default::default(),
            trie_factory,
        };

        Ok(state)
    }

    pub fn root(&self) -> &H256 {
        &self.root
    }

    /// Destroy the current object and return root and database.
    pub fn drop(mut self) -> (H256, B) {
        self.propagate_to_global_cache();
        (self.root, self.db)
    }

    /// Execute a given parcel, charging parcel fee.
    /// This will change the state accordingly.
    pub fn apply(&mut self, parcel: &SignedParcel) -> Result<ParcelOutcome, Error> {
        self.create_checkpoint(PARCEL_FEE_CHECKPOINT);

        match self.apply_internal(parcel) {
            Err(Error::Transaction(_)) => unreachable!(),
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

    fn apply_internal(&mut self, parcel: &SignedParcel) -> Result<ParcelOutcome, Error> {
        let fee_payer = parcel.sender();
        let nonce = self.nonce(&fee_payer)?;

        if parcel.nonce != nonce {
            return Err(ParcelError::InvalidNonce {
                expected: nonce,
                got: parcel.nonce,
            }.into())
        }

        let fee = parcel.as_unsigned().fee;
        let balance = self.balance(&fee_payer)?;
        if fee > balance {
            return Err(ParcelError::InsufficientBalance {
                address: fee_payer,
                cost: fee,
                balance,
            }.into())
        }

        self.inc_nonce(&fee_payer)?;
        self.sub_balance(&fee_payer, &fee)?;

        // The failed parcel also must pay the fee and increase nonce.
        self.create_checkpoint(PARCEL_ACTION_CHECKPOINT);

        match self.apply_action(&parcel.action, &parcel.network_id, &fee_payer) {
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

    fn apply_action(&mut self, action: &Action, network_id: &u64, fee_payer: &Address) -> Result<ParcelOutcome, Error> {
        match action {
            Action::ChangeShardState {
                transactions,
            } => {
                // FIXME: Use shard id when introducing mutli-shard
                let shard_id = 0;
                let result = self.apply_transactions(&transactions, network_id, shard_id)?;
                Ok(ParcelOutcome::Transactions(result))
            }
            Action::Payment {
                receiver,
                value,
            } => match self.transfer_balance(fee_payer, receiver, value) {
                Ok(()) => Ok(ParcelOutcome::Single {
                    invoice: Invoice::Success,
                    error: None,
                }),
                Err(Error::Parcel(
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
            } => match self.set_regular_key(fee_payer, key) {
                Ok(()) => Ok(ParcelOutcome::Single {
                    invoice: Invoice::Success,
                    error: None,
                }),
                Err(error) => Err(error.into()),
            },
        }
    }

    fn apply_transactions(
        &mut self,
        transactions: &[Transaction],
        network_id: &u64,
        shard_id: u32,
    ) -> Result<Vec<TransactionOutcome>, Error> {
        // FIXME: Handle the case that shard doesn't exist
        let shard_root = self.shard_root(&ShardAddress::new(shard_id))?.unwrap_or(BLAKE_NULL_RLP);

        // FIXME: Make it mutable borrow db instead of cloning.
        let mut shard_level_state = ShardLevelState::from_existing(self.db.clone(), shard_root, self.trie_factory)?;

        let mut results = Vec::with_capacity(transactions.len());
        for t in transactions {
            let result = shard_level_state.apply(t, network_id)?;
            results.push(result);
        }
        let (new_shard_root, db) = shard_level_state.drop();
        self.db = db;

        self.set_shard_root(shard_id, &shard_root, &new_shard_root)?;
        Ok(results)
    }
}

trait TopStateInternal<B: Backend + TopBackend> {
    fn ensure_account_cached<F, U>(&self, a: &Address, f: F) -> TrieResult<U>
    where
        F: Fn(Option<&Account>) -> U;

    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn require_account<'a>(&'a self, a: &Address) -> TrieResult<RefMut<'a, Account>>;

    fn require_shard<'a>(&'a self, shard_id: u32) -> TrieResult<RefMut<'a, Shard>>;
}

impl<B: Backend + TopBackend> TopStateInternal<B> for TopLevelState<B> {
    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn ensure_account_cached<F, U>(&self, a: &Address, f: F) -> TrieResult<U>
    where
        F: Fn(Option<&Account>) -> U, {
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_global_cache = |a| self.db.get_cached_account_with(a, |acc| f(acc.map(|a| &*a)));
        self.account.ensure_cached(a, &f, db, from_global_cache)
    }

    fn require_account<'a>(&'a self, a: &Address) -> TrieResult<RefMut<'a, Account>> {
        let default = || Account::new(0u8.into(), 0.into());
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_account(a);
        self.account.require_item_or_from(a, default, db, from_db)
    }

    fn require_shard<'a>(&'a self, shard_id: u32) -> TrieResult<RefMut<'a, Shard>> {
        let default = || Shard::new(BLAKE_NULL_RLP);
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let shard_address = ShardAddress::new(shard_id);
        let from_db = || self.db.get_cached_shard(&shard_address);
        self.shard.require_item_or_from(&shard_address, default, db, from_db)
    }
}

impl<B: TopBackend + ShardBackend> fmt::Debug for TopLevelState<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "account: {:?} shard: {:?}", self.account, self.shard)
    }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for TopLevelState<StateDB> {
    fn clone(&self) -> TopLevelState<StateDB> {
        TopLevelState {
            db: self.db.clone(),
            root: self.root.clone(),
            id_of_checkpoints: self.id_of_checkpoints.clone(),
            account: self.account.clone(),
            shard: self.shard.clone(),
            trie_factory: self.trie_factory.clone(),
        }
    }
}

impl<B: Backend + TopBackend + ShardBackend + Clone> TopState<B> for TopLevelState<B> {
    fn kill_account(&mut self, account: &Address) {
        self.account.remove(account);
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

    fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) -> Result<(), Error> {
        let balance = self.balance(from)?;
        if &balance < by {
            return Err(ParcelError::InsufficientBalance {
                address: *from,
                cost: *by,
                balance,
            }.into())
        }
        self.sub_balance(from, by)?;
        self.add_balance(to, by)?;
        Ok(())
    }

    fn inc_nonce(&mut self, a: &Address) -> TrieResult<()> {
        self.require_account(a)?.inc_nonce();
        Ok(())
    }

    fn set_regular_key(&mut self, a: &Address, key: &Public) -> Result<(), Error> {
        self.require_account(a)?.set_regular_key(key);
        Ok(())
    }

    fn set_shard_root(&mut self, shard_id: u32, old_root: &H256, new_root: &H256) -> Result<(), Error> {
        let mut shard = self.require_shard(shard_id)?;
        assert_eq!(old_root, shard.root());
        shard.set_root(*new_root);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ccrypto::Blake;
    use ckey::{Generator, Random};
    use ctypes::{Address, Secret, U256};

    use super::super::super::parcel::{AssetOutPoint, AssetTransferInput, AssetTransferOutput, Parcel};
    use super::super::super::tests::helpers::{get_temp_state, get_temp_state_db};
    use super::super::super::transaction::Transaction;
    use super::*;

    fn secret() -> Secret {
        Secret::blake("")
    }

    #[test]
    fn apply_empty_parcel() {
        let mut state = get_temp_state();

        let signed_parcel = Parcel {
            fee: 5.into(),
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = signed_parcel.sender();
        state.add_balance(&sender, &20.into()).unwrap();

        let res = state.apply(&signed_parcel).unwrap();
        match res {
            ParcelOutcome::Transactions(res) => {
                assert!(res.is_empty());
            }
            _ => unreachable!(),
        }
        assert_eq!(state.balance(&sender).unwrap(), 15.into());
        assert_eq!(state.nonce(&sender).unwrap(), 1.into());
    }

    #[test]
    fn should_apply_error_for_invalid_nonce() {
        let mut state = get_temp_state();

        let signed_parcel = Parcel {
            nonce: 2.into(),
            fee: 5.into(),
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = signed_parcel.sender();
        state.add_balance(&sender, &20.into()).unwrap();

        match state.apply(&signed_parcel) {
            Err(Error::Parcel(err)) => {
                assert_eq!(
                    ParcelError::InvalidNonce {
                        expected: 0.into(),
                        got: 2.into()
                    },
                    err
                );
            }
            _ => unreachable!(),
        }

        assert_eq!(state.balance(&sender).unwrap(), 20.into());
        assert_eq!(state.nonce(&sender).unwrap(), 0.into());
    }

    #[test]
    fn should_apply_error_for_not_enough_cash() {
        let mut state = get_temp_state();
        let signed_parcel = Parcel {
            fee: 5.into(),
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = signed_parcel.sender();
        state.add_balance(&sender, &4.into()).unwrap();

        match state.apply(&signed_parcel) {
            Err(Error::Parcel(err)) => {
                assert_eq!(
                    ParcelError::InsufficientBalance {
                        address: sender,
                        balance: 4.into(),
                        cost: 5.into(),
                    },
                    err
                );
            }
            _ => unreachable!(),
        }
        assert_eq!(state.balance(&sender).unwrap(), 4.into());
        assert_eq!(state.nonce(&sender).unwrap(), 0.into());
    }

    #[test]
    fn should_apply_payment() {
        let mut state = get_temp_state();
        let receiver = 1u64.into();

        let keypair = Random.generate().unwrap();

        let signed_parcel = Parcel {
            fee: 5.into(),
            action: Action::Payment {
                receiver,
                value: 10.into(),
            },
            ..Parcel::default()
        }.sign(keypair.private());
        let sender = signed_parcel.sender();
        assert_eq!(keypair.address(), sender);
        state.add_balance(&sender, &20.into()).unwrap();

        let res = state.apply(&signed_parcel).unwrap();
        match res {
            ParcelOutcome::Single {
                invoice,
                error,
            } => {
                assert_eq!(invoice, Invoice::Success);
                assert!(error.is_none());
            }
            _ => unreachable!(),
        }
        assert_eq!(state.balance(&receiver).unwrap(), 10.into());
        assert_eq!(state.balance(&sender).unwrap(), 5.into());
        assert_eq!(state.nonce(&sender).unwrap(), 1.into());
    }

    #[test]
    fn should_apply_set_regular_key() {
        let mut state = get_temp_state();
        let key = 1u64.into();

        let keypair = Random.generate().unwrap();

        let signed_parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key,
            },
            ..Parcel::default()
        }.sign(keypair.private());
        let sender = signed_parcel.sender();
        assert_eq!(sender, keypair.address());
        state.add_balance(&sender, &5.into()).unwrap();

        assert_eq!(state.regular_key(&sender), Ok(None));
        let res = state.apply(&signed_parcel).unwrap();
        assert_eq!(
            ParcelOutcome::Single {
                invoice: Invoice::Success,
                error: None
            },
            res
        );
        assert_eq!(state.regular_key(&sender), Ok(Some(key)));
    }

    #[test]
    fn should_apply_error_for_action_failure() {
        let mut state = get_temp_state();
        let receiver = 1u64.into();
        let keypair = Random.generate().unwrap();

        let signed_parcel = Parcel {
            fee: 5.into(),
            action: Action::Payment {
                receiver,
                value: 30.into(),
            },
            ..Parcel::default()
        }.sign(keypair.private());
        let sender = signed_parcel.sender();
        assert_eq!(keypair.address(), sender);
        state.add_balance(&sender, &20.into()).unwrap();

        let res = state.apply(&signed_parcel).unwrap();
        assert_eq!(
            ParcelOutcome::Single {
                invoice: Invoice::Failed,
                error: Some(ParcelError::InsufficientBalance {
                    address: sender,
                    balance: 15.into(),
                    cost: 30.into(),
                })
            },
            res
        );

        assert_eq!(state.balance(&receiver).unwrap(), 0.into());
        assert_eq!(state.balance(&sender).unwrap(), 15.into());
        assert_eq!(state.nonce(&sender).unwrap(), 1.into());
    }

    #[test]
    fn should_work_when_cloned() {
        let a = Address::zero();

        let mut state = {
            let mut state = get_temp_state();
            assert_eq!(state.account_exists(&a).unwrap(), false);
            state.inc_nonce(&a).unwrap();
            state.commit().unwrap();
            state.clone()
        };

        state.inc_nonce(&a).unwrap();
        state.commit().unwrap();
    }


    #[test]
    fn state_is_not_synchronized_when_cloned() {
        let a = Address::random();

        let original_state = get_temp_state();

        assert_eq!(original_state.account_exists(&a).unwrap(), false);

        let mut cloned_state = original_state.clone();

        cloned_state.inc_nonce(&a).unwrap();
        cloned_state.commit().unwrap();

        assert_ne!(original_state.nonce(&a), cloned_state.nonce(&a));
    }

    #[test]
    fn get_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state();
            state.inc_nonce(&a).unwrap();
            state.add_balance(&a, &U256::from(69u64)).unwrap();
            state.commit().unwrap();
            assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
            state.drop()
        };

        let state = TopLevelState::from_existing(db, root, Default::default()).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
    }

    #[test]
    fn remove() {
        let a = Address::zero();
        let mut state = get_temp_state();
        assert_eq!(state.account_exists(&a).unwrap(), false);
        assert_eq!(state.account_exists_and_not_null(&a).unwrap(), false);
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.account_exists(&a).unwrap(), true);
        assert_eq!(state.account_exists_and_not_null(&a).unwrap(), true);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.kill_account(&a);
        assert_eq!(state.account_exists(&a).unwrap(), false);
        assert_eq!(state.account_exists_and_not_null(&a).unwrap(), false);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn empty_account_is_not_created() {
        let a = Address::zero();
        let db = get_temp_state_db();
        let (root, db) = {
            let mut state = TopLevelState::new(db, Default::default());
            state.add_balance(&a, &U256::default()).unwrap(); // create an empty account
            state.commit().unwrap();
            state.drop()
        };
        let state = TopLevelState::from_existing(db, root, Default::default()).unwrap();
        assert!(!state.account_exists(&a).unwrap());
        assert!(!state.account_exists_and_not_null(&a).unwrap());
    }

    #[test]
    fn remove_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state();
            state.inc_nonce(&a).unwrap();
            state.commit().unwrap();
            assert_eq!(state.account_exists(&a).unwrap(), true);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
            state.drop()
        };

        let (root, db) = {
            let mut state = TopLevelState::from_existing(db, root, Default::default()).unwrap();
            assert_eq!(state.account_exists(&a).unwrap(), true);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
            state.kill_account(&a);
            state.commit().unwrap();
            assert_eq!(state.account_exists(&a).unwrap(), false);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
            state.drop()
        };

        let state = TopLevelState::from_existing(db, root, Default::default()).unwrap();
        assert_eq!(state.account_exists(&a).unwrap(), false);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn alter_balance() {
        let mut state = get_temp_state();
        let a = Address::zero();
        let b = 1u64.into();
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.sub_balance(&a, &U256::from(42u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(27u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(27u64));
        state.transfer_balance(&a, &b, &U256::from(18u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(9u64));
        assert_eq!(state.balance(&b).unwrap(), U256::from(18u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(9u64));
        assert_eq!(state.balance(&b).unwrap(), U256::from(18u64));
    }

    #[test]
    fn alter_nonce() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(2u64));
        state.commit().unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(2u64));
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(3u64));
        state.commit().unwrap();
        assert_eq!(state.nonce(&a).unwrap(), U256::from(3u64));
    }

    #[test]
    fn balance_nonce() {
        let mut state = get_temp_state();
        let a = Address::zero();
        assert_eq!(state.balance(&a).unwrap(), U256::from(0u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(0u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn ensure_cached() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.require_account(&a).unwrap();
        state.commit().unwrap();
        assert_eq!(*state.root(), "27a2e0676e24a2d55dd6bc3ad8ec876108a47e70534ea49718a1f76d5c05479e".into());
    }

    #[test]
    fn checkpoint_basic() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.create_checkpoint(0);
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.discard_checkpoint(0);
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.create_checkpoint(1);
        state.add_balance(&a, &U256::from(1u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(70u64));
        state.revert_to_checkpoint(1);
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    }

    #[test]
    fn checkpoint_nested() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.create_checkpoint(0);
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        state.create_checkpoint(1);
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64 + 69u64));
        state.revert_to_checkpoint(1);
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.revert_to_checkpoint(0);
        assert_eq!(state.balance(&a).unwrap(), U256::from(0));
    }

    #[test]
    fn checkpoint_discard() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.create_checkpoint(0);
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        state.create_checkpoint(1);
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64 + 69u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.discard_checkpoint(1);
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64 + 69u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.revert_to_checkpoint(0);
        assert_eq!(state.balance(&a).unwrap(), U256::from(0u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn create_empty() {
        let mut state = get_temp_state();
        state.commit().unwrap();
        assert_eq!(*state.root(), "45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0".into());
    }

    #[test]
    fn mint_permissioned_asset() {
        let mut state = {
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.clone_canon(&root_parent);
            TopLevelState::new(state_db, Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let amount = 30;
        let transaction = Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters: parameters.clone(),
            amount: Some(amount),
            registrar,
            nonce: 0,
        };
        let transaction_hash = transaction.hash();
        let transactions = vec![transaction];
        let signed_parcel = Parcel {
            fee: 11.into(),
            action: Action::ChangeShardState {
                transactions,
            },
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = signed_parcel.sender();

        state.add_balance(&sender, &U256::from(69u64)).unwrap();

        assert_eq!(
            ParcelOutcome::Transactions(vec![TransactionOutcome {
                invoice: Invoice::Success,
                error: None,
            }]),
            state.apply(&signed_parcel).unwrap()
        );

        assert_eq!(state.balance(&sender), Ok(58.into()));
        assert_eq!(state.nonce(&sender), Ok(1.into()));

        let shard_id = 0;
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash);
        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset_address = AssetAddress::new(transaction_hash, 0);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(Some(Asset::new(asset_scheme_address.into(), lock_script_hash, parameters, amount))), asset);
    }

    #[test]
    fn mint_infinite_permissioned_asset() {
        let mut state = {
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.clone_canon(&root_parent);
            TopLevelState::new(state_db, Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters: parameters.clone(),
            amount: None,
            registrar,
            nonce: 0,
        };
        let transaction_hash = transaction.hash();
        let transactions = vec![transaction];
        let signed_parcel = Parcel {
            fee: 5.into(),
            action: Action::ChangeShardState {
                transactions,
            },
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = signed_parcel.sender();

        state.add_balance(&sender, &U256::from(69u64)).unwrap();

        assert_eq!(
            ParcelOutcome::Transactions(vec![TransactionOutcome {
                invoice: Invoice::Success,
                error: None,
            }]),
            state.apply(&signed_parcel).unwrap()
        );

        assert_eq!(state.balance(&sender), Ok(64.into()));
        assert_eq!(state.nonce(&sender), Ok(1.into()));

        let shard_id = 0;

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash);
        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, registrar))), asset_scheme);

        let asset_address = AssetAddress::new(transaction_hash, 0);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(
            Ok(Some(Asset::new(asset_scheme_address.into(), lock_script_hash, parameters, ::std::u64::MAX))),
            asset
        );
    }

    #[test]
    fn mint_and_transfer_in_the_same_parcel() {
        let mut state = {
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.clone_canon(&root_parent);
            TopLevelState::new(state_db, Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash =
            H256::from_str("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050").unwrap();
        let registrar = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters: vec![],
            amount: Some(amount),
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();

        let network_id = 0xBeef;

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash);
        let asset_type = asset_scheme_address.clone().into();
        let asset_address = AssetAddress::new(mint_hash, 0);

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
        let signed_parcel = Parcel {
            fee: 20.into(),
            network_id,
            action: Action::ChangeShardState {
                transactions,
            },
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = signed_parcel.sender();

        state.add_balance(&sender, &U256::from(120)).unwrap();

        let shard_id = 0x00;

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
            state.apply(&signed_parcel).unwrap()
        );

        assert_eq!(state.balance(&sender), Ok(100.into()));
        assert_eq!(state.nonce(&sender), Ok(1.into()));

        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(None), asset);

        let asset0_address = AssetAddress::new(transfer_hash, 0);
        let asset0 = state.asset(shard_id, &asset0_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = AssetAddress::new(transfer_hash, 1);
        let asset1 = state.asset(shard_id, &asset1_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], 5))), asset1);

        let asset2_address = AssetAddress::new(transfer_hash, 2);
        let asset2 = state.asset(shard_id, &asset2_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);
    }

    #[test]
    fn mint_and_transfer_in_different_parcel() {
        let mut state = {
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.clone_canon(&root_parent);
            TopLevelState::new(state_db, Default::default())
        };


        let metadata = "metadata".to_string();
        let lock_script_hash =
            H256::from_str("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050").unwrap();
        let registrar = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters: vec![],
            amount: Some(amount),
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();

        let network_id = 0xBeef;

        let mint_parcel = Parcel {
            fee: 20.into(),
            network_id,
            nonce: 0.into(),
            action: Action::ChangeShardState {
                transactions: vec![mint],
            },
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = mint_parcel.sender();

        state.add_balance(&sender, &U256::from(120)).unwrap();

        assert_eq!(
            ParcelOutcome::Transactions(vec![TransactionOutcome {
                invoice: Invoice::Success,
                error: None,
            }]),
            state.apply(&mint_parcel).unwrap()
        );

        assert_eq!(state.balance(&sender), Ok(100.into()));
        assert_eq!(state.nonce(&sender), Ok(1.into()));

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash);
        let asset_type = asset_scheme_address.clone().into();
        let asset_address = AssetAddress::new(mint_hash, 0);

        let shard_id = 0x00;

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
            },
            ..Parcel::default()
        }.sign(&secret().into());

        assert_eq!(
            ParcelOutcome::Transactions(vec![TransactionOutcome {
                invoice: Invoice::Success,
                error: None,
            }]),
            state.apply(&transfer_parcel).unwrap()
        );

        assert_eq!(state.balance(&sender), Ok(70.into()));
        assert_eq!(state.nonce(&sender), Ok(2.into()));

        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(None), asset);

        let asset0_address = AssetAddress::new(transfer_hash, 0);
        let asset0 = state.asset(shard_id, &asset0_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = AssetAddress::new(transfer_hash, 1);
        let asset1 = state.asset(shard_id, &asset1_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], 5))), asset1);

        let asset2_address = AssetAddress::new(transfer_hash, 2);
        let asset2 = state.asset(shard_id, &asset2_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);
    }
}
