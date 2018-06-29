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
use trie::{self, Result as TrieResult, Trie, TrieError, TrieFactory};

use super::invoice::Invoice;
use super::parcel::ParcelError;
use super::state_db::StateDB;
use super::{Transaction, TransactionError};

use self::cache::Cache;
use self::shard_level::ShardLevelState;
use self::traits::{CheckpointId, StateWithCheckpoint};

#[macro_use]
mod address;

mod account;
mod asset;
mod asset_scheme;
mod backend;
mod cache;
mod info;
mod shard;
mod shard_level;
mod shard_state;
mod top_state;
mod traits;

pub use self::account::Account;
pub use self::asset::{Asset, AssetAddress};
pub use self::asset_scheme::{AssetScheme, AssetSchemeAddress};
pub use self::backend::{Backend, Basic as BasicBackend, ShardBackend, TopBackend};
pub use self::cache::CacheableItem;
pub use self::info::{ShardStateInfo, TopStateInfo};
pub use self::shard::{Shard, ShardAddress};
pub use self::shard_state::ShardState;
pub use self::top_state::TopState;
pub use self::traits::StateWithCache;

/// Used to return information about an `State::apply` operation.
pub enum ParcelOutcome {
    Single {
        invoice: Invoice,
        error: Option<ParcelError>,
    },
    Transactions(Vec<TransactionOutcome>),
}

pub struct TransactionOutcome {
    /// The invoice for the applied parcel.
    pub invoice: Invoice,
    /// The output of the applied parcel.
    pub error: Option<TransactionError>,
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
///
pub struct State<B> {
    db: B,
    root: H256,
    account: Cache<Account>,
    shard: Cache<Shard>,
    id_of_checkpoints: Vec<CheckpointId>,
    trie_factory: TrieFactory,
}

impl<B> TopStateInfo for State<B>
where
    B: Backend + TopBackend + ShardBackend + Clone,
{
    fn nonce(&self, a: &Address) -> trie::Result<U256> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or_else(U256::zero, |account| *account.nonce()))
    }
    fn balance(&self, a: &Address) -> trie::Result<U256> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or(U256::zero(), |account| *account.balance()))
    }
    fn regular_key(&self, a: &Address) -> trie::Result<Option<Public>> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or(None, |account| account.regular_key()))
    }

    fn shard_root(&self, a: &ShardAddress) -> trie::Result<Option<H256>> {
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
    ) -> trie::Result<Option<AssetScheme>> {
        // FIXME: Handle the case that shard doesn't exist
        let shard_root = self.shard_root(&ShardAddress::new(shard_id))?.unwrap_or(BLAKE_NULL_RLP);
        // FIXME: Make it mutable borrow db instead of cloning.
        let shard_level_state = ShardLevelState::from_existing(self.db.clone(), shard_root, self.trie_factory)?;
        shard_level_state.asset_scheme(asset_scheme_address)
    }

    fn asset(&self, shard_id: u32, asset_address: &AssetAddress) -> trie::Result<Option<Asset>> {
        // FIXME: Handle the case that shard doesn't exist
        let shard_root = self.shard_root(&ShardAddress::new(shard_id))?.unwrap_or(BLAKE_NULL_RLP);
        // FIXME: Make it mutable borrow db instead of cloning.
        let shard_level_state = ShardLevelState::from_existing(self.db.clone(), shard_root, self.trie_factory)?;
        shard_level_state.asset(asset_address)
    }
}

const PARCEL_CHECKPOINT: CheckpointId = 123;
const PARCEL_BODY_CHECKPOINT: CheckpointId = 130;
const TRANSACTION_CHECKPOINT: CheckpointId = 456;
const TRANSACTIONS_CHECKPOINT: CheckpointId = 789;

impl<B> StateWithCheckpoint for State<B>
where
    B: Backend + TopBackend,
{
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

impl<B> StateWithCache for State<B>
where
    B: Backend + TopBackend + ShardBackend,
{
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

impl<B> State<B>
where
    B: Backend + TopBackend + ShardBackend + Clone,
{
    /// Creates new state with empty state root
    /// Used for tests.
    #[cfg(test)]
    pub fn new(mut db: B, trie_factory: TrieFactory) -> State<B> {
        let mut root = H256::new();
        {
            // init trie and reset root too null
            let _ = trie_factory.create(db.as_hashdb_mut(), &mut root);
        }

        State {
            db,
            root,
            account: Cache::new(),
            shard: Cache::new(),
            id_of_checkpoints: Default::default(),
            trie_factory,
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(db: B, root: H256, trie_factory: TrieFactory) -> Result<State<B>, TrieError> {
        if !db.as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        let state = State {
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
        self.create_checkpoint(PARCEL_CHECKPOINT);

        match self.execute(parcel) {
            Err(Error::Transaction(_)) => unreachable!(),
            Err(err) => {
                self.revert_to_checkpoint(PARCEL_CHECKPOINT);
                Err(err)
            }
            Ok(outcomes) => {
                self.discard_checkpoint(PARCEL_CHECKPOINT);
                self.commit()?; // FIXME: Remove early commit.
                Ok(outcomes)
            }
        }
    }

    fn execute(&mut self, parcel: &SignedParcel) -> Result<ParcelOutcome, Error> {
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

        self.create_checkpoint(PARCEL_BODY_CHECKPOINT);
        let transactions = match &parcel.action {
            Action::ChangeShardState {
                transactions,
            } => transactions,
            Action::Payment {
                receiver,
                value,
            } => match self.transfer_balance(&fee_payer, receiver, value) {
                Ok(()) => {
                    self.discard_checkpoint(PARCEL_BODY_CHECKPOINT);
                    return Ok(ParcelOutcome::Single {
                        invoice: Invoice::Success,
                        error: None,
                    })
                }
                Err(Error::Parcel(
                    err @ ParcelError::InsufficientBalance {
                        ..
                    },
                )) => {
                    self.discard_checkpoint(PARCEL_BODY_CHECKPOINT);
                    return Ok(ParcelOutcome::Single {
                        invoice: Invoice::Failed,
                        error: Some(err),
                    })
                }
                Err(err) => {
                    self.revert_to_checkpoint(PARCEL_BODY_CHECKPOINT);
                    return Err(err)
                }
            },
            Action::SetRegularKey {
                key,
            } => match self.set_regular_key(&fee_payer, key) {
                Ok(()) => {
                    self.discard_checkpoint(PARCEL_BODY_CHECKPOINT);
                    return Ok(ParcelOutcome::Single {
                        invoice: Invoice::Success,
                        error: None,
                    })
                }
                Err(error) => {
                    self.revert_to_checkpoint(PARCEL_BODY_CHECKPOINT);
                    return Err(error)
                }
            },
        };
        self.discard_checkpoint(PARCEL_BODY_CHECKPOINT);

        let shard_id = 0;
        // FIXME: Use shard id when introducing mutli-shard
        // FIXME: Handle the case that shard doesn't exist
        let shard_root = self.shard_root(&ShardAddress::new(shard_id))?.unwrap_or(BLAKE_NULL_RLP);
        // FIXME: Make it mutable borrow db instead of cloning.
        let mut shard_level_state = ShardLevelState::from_existing(self.db.clone(), shard_root, self.trie_factory)?;

        self.create_checkpoint(TRANSACTIONS_CHECKPOINT);

        let mut results = Vec::with_capacity(transactions.len());
        for t in transactions {
            shard_level_state.create_checkpoint(TRANSACTION_CHECKPOINT);
            results.push(match shard_level_state.execute_transaction(t, &parcel.network_id) {
                Ok(_) => {
                    cinfo!(TX, "Tx({}) is applied", t.hash());
                    shard_level_state.discard_checkpoint(TRANSACTION_CHECKPOINT);
                    let invoice = Invoice::Success;
                    let error = None;
                    TransactionOutcome {
                        invoice,
                        error,
                    }
                }
                Err(Error::Transaction(err)) => {
                    cinfo!(TX, "Cannot apply Tx({}): {:?}", t.hash(), err);
                    shard_level_state.revert_to_checkpoint(TRANSACTION_CHECKPOINT);
                    let invoice = Invoice::Failed;
                    let error = Some(err);
                    TransactionOutcome {
                        invoice,
                        error,
                    }
                }
                Err(err) => {
                    cinfo!(TX, "Tx({}) is invalid: {:?}", t.hash(), err);
                    shard_level_state.discard_checkpoint(TRANSACTION_CHECKPOINT);
                    self.revert_to_checkpoint(TRANSACTIONS_CHECKPOINT);
                    return Err(err)
                }
            });
        }
        shard_level_state.commit()?;
        let (new_shard_root, db) = shard_level_state.drop();
        self.db = db;
        // FIXME: Use shard id when introducing multi-shards
        match self.set_shard_root(0, &shard_root, &new_shard_root) {
            Err(err) => {
                self.revert_to_checkpoint(TRANSACTIONS_CHECKPOINT);
                Err(err.into())
            }
            _ => {
                self.discard_checkpoint(TRANSACTIONS_CHECKPOINT);
                Ok(ParcelOutcome::Transactions(results))
            }
        }
    }
}

trait TopStateInternal<B>
where
    B: Backend + TopBackend, {
    fn ensure_account_cached<F, U>(&self, a: &Address, f: F) -> trie::Result<U>
    where
        F: Fn(Option<&Account>) -> U;

    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn require_account<'a>(&'a self, a: &Address) -> trie::Result<RefMut<'a, Account>>;

    fn require_shard<'a>(&'a self, shard_id: u32) -> trie::Result<RefMut<'a, Shard>>;
}

impl<B> TopStateInternal<B> for State<B>
where
    B: Backend + TopBackend,
{
    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn ensure_account_cached<F, U>(&self, a: &Address, f: F) -> trie::Result<U>
    where
        F: Fn(Option<&Account>) -> U, {
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_global_cache = |a| self.db.get_cached_account_with(a, |acc| f(acc.map(|a| &*a)));
        self.account.ensure_cached(a, &f, db, from_global_cache)
    }

    fn require_account<'a>(&'a self, a: &Address) -> trie::Result<RefMut<'a, Account>> {
        let default = || Account::new(0u8.into(), 0.into());
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_account(a);
        self.account.require_item_or_from(a, default, db, from_db)
    }

    fn require_shard<'a>(&'a self, shard_id: u32) -> trie::Result<RefMut<'a, Shard>> {
        let default = || Shard::new(BLAKE_NULL_RLP);
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let shard_address = ShardAddress::new(shard_id);
        let from_db = || self.db.get_cached_shard(&shard_address);
        self.shard.require_item_or_from(&shard_address, default, db, from_db)
    }
}

impl<B> fmt::Debug for State<B>
where
    B: TopBackend + ShardBackend,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "account: {:?} shard: {:?}", self.account, self.shard)
    }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for State<StateDB> {
    fn clone(&self) -> State<StateDB> {
        State {
            db: self.db.clone(),
            root: self.root.clone(),
            id_of_checkpoints: self.id_of_checkpoints.clone(),
            account: self.account.clone(),
            shard: self.shard.clone(),
            trie_factory: self.trie_factory.clone(),
        }
    }
}

impl<B> TopState<B> for State<B>
where
    B: Backend + TopBackend + ShardBackend + Clone,
{
    fn kill_account(&mut self, account: &Address) {
        self.account.remove(account);
    }

    fn account_exists(&self, a: &Address) -> trie::Result<bool> {
        // Bloom filter does not contain empty accounts, so it is important here to
        // check if account exists in the database directly before EIP-161 is in effect.
        self.ensure_account_cached(a, |a| a.is_some())
    }

    fn account_exists_and_not_null(&self, a: &Address) -> trie::Result<bool> {
        self.ensure_account_cached(a, |a| a.map_or(false, |a| !a.is_null()))
    }

    fn account_exists_and_has_nonce(&self, a: &Address) -> trie::Result<bool> {
        self.ensure_account_cached(a, |a| a.map_or(false, |a| !a.nonce().is_zero()))
    }

    fn add_balance(&mut self, a: &Address, incr: &U256) -> trie::Result<()> {
        ctrace!(STATE, "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        let is_value_transfer = !incr.is_zero();
        if is_value_transfer {
            self.require_account(a)?.add_balance(incr);
        }
        Ok(())
    }

    fn sub_balance(&mut self, a: &Address, decr: &U256) -> trie::Result<()> {
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

    fn inc_nonce(&mut self, a: &Address) -> trie::Result<()> {
        self.require_account(a).map(|mut x| x.inc_nonce())
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
    use ccrypto::Blake;
    use ckeys::{Generator, Random};
    use ctypes::{Address, Bytes, Secret, U256};

    use super::super::parcel::Parcel;
    use super::super::tests::helpers::{get_temp_state, get_temp_state_db};
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
            ParcelOutcome::Single {
                ..
            } => unreachable!(),
            ParcelOutcome::Transactions(res) => {
                assert!(res.is_empty());
            }
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
            Ok(_) => unreachable!(),
            Err(_) => unreachable!(),
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
            Ok(_) => unreachable!(),
            Err(_) => unreachable!(),
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
            ParcelOutcome::Transactions(_) => {
                unreachable!();
            }
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

        assert_eq!(state.regular_key(&sender).unwrap(), None);
        let res = state.apply(&signed_parcel).unwrap();
        match res {
            ParcelOutcome::Single {
                invoice,
                error,
            } => {
                assert_eq!(invoice, Invoice::Success);
                assert!(error.is_none());
            }
            ParcelOutcome::Transactions(_) => {
                unreachable!();
            }
        }
        assert_eq!(state.regular_key(&sender).unwrap(), Some(key));
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
        match res {
            ParcelOutcome::Single {
                invoice,
                error,
            } => {
                assert_eq!(invoice, Invoice::Failed);
                assert_eq!(
                    error.as_ref().unwrap(),
                    &ParcelError::InsufficientBalance {
                        address: sender,
                        balance: 15.into(),
                        cost: 30.into(),
                    }
                );
            }
            ParcelOutcome::Transactions(_) => {
                unreachable!();
            }
        }
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

        let state = State::from_existing(db, root, Default::default()).unwrap();
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
            let mut state = State::new(db, Default::default());
            state.add_balance(&a, &U256::default()).unwrap(); // create an empty account
            state.commit().unwrap();
            state.drop()
        };
        let state = State::from_existing(db, root, Default::default()).unwrap();
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
            let mut state = State::from_existing(db, root, Default::default()).unwrap();
            assert_eq!(state.account_exists(&a).unwrap(), true);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
            state.kill_account(&a);
            state.commit().unwrap();
            assert_eq!(state.account_exists(&a).unwrap(), false);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
            state.drop()
        };

        let state = State::from_existing(db, root, Default::default()).unwrap();
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
            State::new(state_db, Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let registrar = Some(Address::random());
        let amount = 30;
        let transaction = Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters: vec![],
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

        match state.apply(&signed_parcel).unwrap() {
            ParcelOutcome::Transactions(res) => match res.as_slice() {
                [TransactionOutcome {
                    invoice,
                    error,
                }] => {
                    assert_eq!(&None, error);
                    assert_eq!(&Invoice::Success, invoice);
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        assert_eq!(state.balance(&sender).unwrap(), 58.into());
        assert_eq!(state.nonce(&sender).unwrap(), 1.into());

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash);
        let asset_scheme = state.asset_scheme(0, &asset_scheme_address).unwrap();
        let asset_scheme = asset_scheme.unwrap();
        assert_eq!(&metadata, asset_scheme.metadata());
        assert_eq!(&amount, asset_scheme.amount());
        assert_eq!(&registrar, asset_scheme.registrar());
        assert!(asset_scheme.is_permissioned());

        let asset_address = AssetAddress::new(transaction_hash, 0);
        let asset = state.asset(0, &asset_address).unwrap();
        let asset = asset.unwrap();
        let asset_type: H256 = asset_scheme_address.into();
        assert_eq!(&asset_type, asset.asset_type());
        assert_eq!(&lock_script_hash, asset.lock_script_hash());
        assert_eq!(&Vec::<Bytes>::new(), asset.parameters());
        assert_eq!(&amount, asset.amount());
    }

    #[test]
    fn mint_infinite_permissioned_asset() {
        let mut state = {
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.clone_canon(&root_parent);
            State::new(state_db, Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters: vec![],
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

        match state.apply(&signed_parcel).unwrap() {
            ParcelOutcome::Transactions(res) => match res.as_slice() {
                [TransactionOutcome {
                    invoice,
                    error,
                }] => {
                    assert_eq!(&None, error);
                    assert_eq!(&Invoice::Success, invoice);
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        assert_eq!(state.balance(&sender).unwrap(), 64.into());
        assert_eq!(state.nonce(&sender).unwrap(), 1.into());

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash);
        let asset_scheme = state.asset_scheme(0, &asset_scheme_address).unwrap();
        let asset_scheme = asset_scheme.unwrap();
        assert_eq!(&metadata, asset_scheme.metadata());
        assert_eq!(&::std::u64::MAX, asset_scheme.amount());
        assert_eq!(&registrar, asset_scheme.registrar());
        assert!(asset_scheme.is_permissioned());

        let asset_address = AssetAddress::new(transaction_hash, 0);
        let asset = state.asset(0, &asset_address).unwrap();
        let asset = asset.unwrap();
        let asset_type: H256 = asset_scheme_address.into();
        assert_eq!(&asset_type, asset.asset_type());
        assert_eq!(&lock_script_hash, asset.lock_script_hash());
        assert_eq!(&Vec::<Bytes>::new(), asset.parameters());
        assert_eq!(&::std::u64::MAX, asset.amount());
    }
}
