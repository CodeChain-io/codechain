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
use std::collections::HashMap;
use std::fmt;

use ccrypto::Blake;
use ctypes::{Address, Bytes, H256, Public, U128, U256};
use cvm::{decode, execute, ScriptResult, VMConfig};
use error::Error;
use parcel::{Action, AssetTransferInput, AssetTransferOutput, SignedParcel};
use trie::{self, Trie, TrieError, TrieFactory};
use unexpected::Mismatch;

use self::cache::Cache;
use super::invoice::Invoice;
use super::parcel::ParcelError;
use super::state_db::StateDB;
use super::{Transaction, TransactionError};

#[macro_use]
mod address;

mod account;
mod asset;
mod asset_scheme;
mod backend;
mod cache;
mod info;

pub use self::account::Account;
pub use self::asset::{Asset, AssetAddress};
pub use self::asset_scheme::{AssetScheme, AssetSchemeAddress};
pub use self::backend::{Backend, Basic as BasicBackend, ShardBackend, TopBackend};
pub use self::cache::CacheableItem;
pub use self::info::{ShardStateInfo, TopStateInfo};

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

type CheckpointId = usize;

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
pub struct State<B>
where
    B: TopBackend + ShardBackend, {
    db: B,
    root: H256,
    account: Cache<Account>,
    asset_scheme: Cache<AssetScheme>,
    asset: Cache<Asset>,
    id_of_checkpoints: Vec<CheckpointId>,
    account_start_nonce: U256,
    trie_factory: TrieFactory,
}

impl<B> TopStateInfo for State<B>
where
    B: Backend + TopBackend + ShardBackend,
{
    fn nonce(&self, a: &Address) -> trie::Result<U256> {
        State::nonce(self, a)
    }
    fn balance(&self, a: &Address) -> trie::Result<U256> {
        State::balance(self, a)
    }
    fn regular_key(&self, a: &Address) -> trie::Result<Option<Public>> {
        State::regular_key(self, a)
    }
}

impl<B> ShardStateInfo for State<B>
where
    B: Backend + TopBackend + ShardBackend,
{
    fn asset_scheme(&self, a: &AssetSchemeAddress) -> trie::Result<Option<AssetScheme>> {
        State::asset_scheme(self, a)
    }

    fn asset(&self, a: &AssetAddress) -> trie::Result<Option<Asset>> {
        State::asset(self, a)
    }
}

const PARCEL_CHECKPOINT: CheckpointId = 123;
const PARCEL_BODY_CHECKPOINT: CheckpointId = 130;
const TRANSACTION_CHECKPOINT: CheckpointId = 456;
const TRANSACTIONS_CHECKPOINT: CheckpointId = 789;

impl<B> State<B>
where
    B: Backend + TopBackend + ShardBackend,
{
    /// Creates new state with empty state root
    /// Used for tests.
    #[cfg(test)]
    pub fn new(mut db: B, account_start_nonce: U256, trie_factory: TrieFactory) -> State<B> {
        let mut root = H256::new();
        {
            // init trie and reset root too null
            let _ = trie_factory.create(db.as_hashdb_mut(), &mut root);
        }

        State {
            db,
            root,
            account: Cache::new(),
            asset_scheme: Cache::new(),
            asset: Cache::new(),
            id_of_checkpoints: Default::default(),
            account_start_nonce,
            trie_factory,
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(
        db: B,
        root: H256,
        account_start_nonce: U256,
        trie_factory: TrieFactory,
    ) -> Result<State<B>, TrieError> {
        if !db.as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        let state = State {
            db,
            root,
            account: Cache::new(),
            asset_scheme: Cache::new(),
            asset: Cache::new(),
            id_of_checkpoints: Default::default(),
            account_start_nonce,
            trie_factory,
        };

        Ok(state)
    }

    /// Create a recoverable checkpoint of this state.
    pub fn checkpoint(&mut self, id: CheckpointId) {
        self.id_of_checkpoints.push(id);
        self.account.checkpoint();
        self.asset_scheme.checkpoint();
        self.asset.checkpoint();
    }

    /// Merge last checkpoint with previous.
    pub fn discard_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.account.discard_checkpoint();
        self.asset_scheme.discard_checkpoint();
        self.asset.discard_checkpoint();
    }

    /// Revert to the last checkpoint and discard it.
    pub fn revert_to_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.account.revert_to_checkpoint();
        self.asset_scheme.revert_to_checkpoint();
        self.asset.revert_to_checkpoint();
    }

    /// Destroy the current object and return root and database.
    pub fn drop(mut self) -> (H256, B) {
        self.propagate_to_global_cache();
        (self.root, self.db)
    }

    /// Return reference to root
    pub fn root(&self) -> &H256 {
        &self.root
    }

    /// Remove an existing account.
    pub fn kill_account(&mut self, account: &Address) {
        self.account.remove(account);
    }

    pub fn kill_asset(&mut self, account: &AssetAddress) {
        self.asset.remove(account);
    }

    /// Determine whether an account exists.
    pub fn account_exists(&self, a: &Address) -> trie::Result<bool> {
        // Bloom filter does not contain empty accounts, so it is important here to
        // check if account exists in the database directly before EIP-161 is in effect.
        self.ensure_account_cached(a, |a| a.is_some())
    }

    /// Determine whether an account exists and if not empty.
    pub fn account_exists_and_not_null(&self, a: &Address) -> trie::Result<bool> {
        self.ensure_account_cached(a, |a| a.map_or(false, |a| !a.is_null()))
    }

    /// Determine whether an account exists and has code or non-zero nonce.
    pub fn account_exists_and_has_nonce(&self, a: &Address) -> trie::Result<bool> {
        self.ensure_account_cached(a, |a| a.map_or(false, |a| *a.nonce() != self.account_start_nonce))
    }

    /// Get the balance of account `a`.
    pub fn balance(&self, a: &Address) -> trie::Result<U256> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or(U256::zero(), |account| *account.balance()))
    }

    /// Get the nonce of account `a`.
    pub fn nonce(&self, a: &Address) -> trie::Result<U256> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or(self.account_start_nonce, |account| *account.nonce()))
    }

    /// Get the regular key of account `a`.
    pub fn regular_key(&self, a: &Address) -> trie::Result<Option<Public>> {
        self.ensure_account_cached(a, |a| a.as_ref().map_or(None, |account| account.regular_key()))
    }

    pub fn asset_scheme(&self, a: &AssetSchemeAddress) -> trie::Result<Option<AssetScheme>> {
        let cached_asset = self.db.get_cached_asset_scheme(&a).and_then(|asset| asset);
        if cached_asset.is_some() {
            return Ok(cached_asset)
        }

        // because of lexical borrow of self.db
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        if let Some(r) = db.get_with(a.as_ref(), AssetScheme::from_rlp)? {
            Ok(Some(r))
        } else {
            return Ok(None)
        }
    }

    pub fn asset(&self, a: &AssetAddress) -> trie::Result<Option<Asset>> {
        let cached_asset = self.db.get_cached_asset(&a).and_then(|asset| asset);
        if cached_asset.is_some() {
            return Ok(cached_asset)
        }

        // because of lexical borrow of self.db
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        if let Some(r) = db.get_with(a.as_ref(), Asset::from_rlp)? {
            Ok(Some(r))
        } else {
            return Ok(None)
        }
    }

    /// Add `incr` to the balance of account `a`.
    pub fn add_balance(&mut self, a: &Address, incr: &U256) -> trie::Result<()> {
        ctrace!(STATE, "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        let is_value_transfer = !incr.is_zero();
        if is_value_transfer {
            self.require_account(a)?.add_balance(incr);
        }
        Ok(())
    }

    /// Subtract `decr` from the balance of account `a`.
    pub fn sub_balance(&mut self, a: &Address, decr: &U256) -> trie::Result<()> {
        ctrace!(STATE, "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
        if !decr.is_zero() || !self.account_exists(a)? {
            self.require_account(a)?.sub_balance(decr);
        }
        Ok(())
    }

    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    pub fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) -> Result<(), Error> {
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

    /// Increment the nonce of account `a` by 1.
    pub fn inc_nonce(&mut self, a: &Address) -> trie::Result<()> {
        self.require_account(a).map(|mut x| x.inc_nonce())
    }

    /// Set the regular key of account `a`
    pub fn set_regular_key(&mut self, a: &Address, key: &Public) -> Result<(), Error> {
        self.require_account(a)?.set_regular_key(key);
        Ok(())
    }

    fn mint_asset(
        &mut self,
        transaction_hash: H256,
        metadata: &String,
        lock_script_hash: &H256,
        parameters: &Vec<Bytes>,
        amount: &Option<u64>,
        registrar: &Option<Address>,
    ) -> Result<(), Error> {
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash);
        let amount = amount.unwrap_or(::std::u64::MAX);
        let asset_scheme = self.require_asset_scheme(&asset_scheme_address, || {
            AssetScheme::new(metadata.clone(), amount, registrar.clone())
        })?;
        ctrace!(TX, "{:?} is minted on {:?}", asset_scheme, asset_scheme_address);

        let asset_address = AssetAddress::new(transaction_hash, 0);
        let asset = self.require_asset(&asset_address, || {
            Asset::new(asset_scheme_address.into(), *lock_script_hash, parameters.clone(), amount)
        });
        ctrace!(TX, "{:?} is generated on {:?}", asset, asset_address);
        Ok(())
    }

    fn transfer_asset(
        &mut self,
        transaction: &Transaction,
        burns: &[AssetTransferInput],
        inputs: &[AssetTransferInput],
        outputs: &[AssetTransferOutput],
    ) -> Result<(), Error> {
        debug_assert!(is_input_and_output_consistent(inputs, outputs));

        for (input, burn) in inputs.iter().map(|input| (input, false)).chain(burns.iter().map(|input| (input, true))) {
            let (address_hash, asset) = {
                let index = input.prev_out.index;
                let address = AssetAddress::new(input.prev_out.transaction_hash, index);
                match self.asset(&address)? {
                    Some(asset) => (address.into(), asset),
                    None => return Err(TransactionError::AssetNotFound(address.into()).into()),
                }
            };

            if *asset.lock_script_hash() != Blake::blake(&input.lock_script) {
                let mismatch = Mismatch {
                    expected: *asset.lock_script_hash(),
                    found: Blake::blake(&input.lock_script),
                };
                return Err(TransactionError::ScriptHashMismatch(mismatch).into())
            }

            let script_result = match (decode(&input.lock_script), decode(&input.unlock_script)) {
                (Ok(lock_script), Ok(unlock_script)) => {
                    // FIXME : apply parameters to vm
                    execute(
                        &unlock_script,
                        &asset.parameters(),
                        &lock_script,
                        transaction.hash_without_script(),
                        VMConfig::default(),
                    )
                }
                // FIXME : Deliver full decode error
                _ => return Err(TransactionError::InvalidScript.into()),
            };

            match script_result {
                Ok(result) => match (result, burn) {
                    (ScriptResult::Unlocked, false) => {}
                    (ScriptResult::Burnt, true) => {}
                    _ => return Err(TransactionError::FailedToUnlock(address_hash).into()),
                },
                Err(err) => {
                    ctrace!(TX, "Cannot run unlock/lock script {:?}", err);
                    return Err(TransactionError::FailedToUnlock(address_hash).into())
                }
            }
        }

        let mut deleted_asset = Vec::with_capacity(inputs.len());
        for input in inputs {
            let index = input.prev_out.index;
            let amount = input.prev_out.amount;
            let address = AssetAddress::new(input.prev_out.transaction_hash, index);

            let asset_type = input.prev_out.asset_type.clone();
            let asset_scheme_address = AssetSchemeAddress::from_hash(asset_type)
                .ok_or(TransactionError::AssetSchemeNotFound(asset_type.into()))?;
            let _asset_scheme = self.asset_scheme((&asset_scheme_address).into())?
                .ok_or(TransactionError::AssetSchemeNotFound(asset_scheme_address.into()))?;

            match self.asset(&address)? {
                Some(asset) => {
                    if asset.amount() != &amount {
                        let address = address.into();
                        let expected = *asset.amount();
                        let got = amount;
                        return Err(TransactionError::InvalidAssetAmount {
                            address,
                            expected,
                            got,
                        }.into())
                    }
                }
                None => return Err(TransactionError::AssetNotFound(address.into()).into()),
            }

            self.kill_asset(&address);
            let hash: H256 = address.into();
            deleted_asset.push((hash, amount));
        }
        let mut created_asset = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.iter().enumerate() {
            let asset_address = AssetAddress::new(transaction.hash(), index);
            let asset =
                Asset::new(output.asset_type, output.lock_script_hash, output.parameters.clone(), output.amount);
            self.require_asset(&asset_address, || asset)?;
            created_asset.push((asset_address, output.amount));
        }
        ctrace!(TX, "Deleted assets {:?}", deleted_asset);
        ctrace!(TX, "Created assets {:?}", created_asset);
        Ok(())
    }

    /// Execute a given parcel, charging parcel fee.
    /// This will change the state accordingly.
    pub fn apply(&mut self, parcel: &SignedParcel) -> Result<ParcelOutcome, Error> {
        self.checkpoint(PARCEL_CHECKPOINT);

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

        self.checkpoint(PARCEL_BODY_CHECKPOINT);
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

        self.checkpoint(TRANSACTIONS_CHECKPOINT);
        let mut results = Vec::with_capacity(transactions.len());
        for t in transactions {
            self.checkpoint(TRANSACTION_CHECKPOINT);
            results.push(match self.execute_transaction(t, &parcel.network_id) {
                Ok(_) => {
                    cinfo!(TX, "Tx({}) is applied", t.hash());
                    self.discard_checkpoint(TRANSACTION_CHECKPOINT);
                    let invoice = Invoice::Success;
                    let error = None;
                    TransactionOutcome {
                        invoice,
                        error,
                    }
                }
                Err(Error::Transaction(err)) => {
                    cinfo!(TX, "Cannot apply Tx({}): {:?}", t.hash(), err);
                    self.revert_to_checkpoint(TRANSACTION_CHECKPOINT);
                    let invoice = Invoice::Failed;
                    let error = Some(err);
                    TransactionOutcome {
                        invoice,
                        error,
                    }
                }
                Err(err) => {
                    cinfo!(TX, "Tx({}) is invalid: {:?}", t.hash(), err);
                    self.discard_checkpoint(TRANSACTION_CHECKPOINT);
                    self.revert_to_checkpoint(TRANSACTIONS_CHECKPOINT);
                    return Err(err)
                }
            });
        }
        self.discard_checkpoint(TRANSACTIONS_CHECKPOINT);
        Ok(ParcelOutcome::Transactions(results))
    }

    fn execute_transaction(&mut self, transaction: &Transaction, parcel_network_id: &u64) -> Result<(), Error> {
        ctrace!(TX, "Execute {:?}(TxHash:{:?})", transaction, transaction.hash());
        match transaction {
            Transaction::AssetMint {
                metadata,
                lock_script_hash,
                amount,
                parameters,
                registrar,
                ..
            } => Ok(self.mint_asset(transaction.hash(), metadata, lock_script_hash, parameters, amount, registrar)?),
            Transaction::AssetTransfer {
                burns,
                inputs,
                outputs,
                network_id,
                ..
            } => {
                if parcel_network_id != network_id {
                    return Err(TransactionError::InvalidNetworkId(Mismatch {
                        expected: *parcel_network_id,
                        found: *network_id,
                    }).into())
                }
                self.transfer_asset(&transaction, burns, inputs, outputs)
            }
        }
    }

    /// Commits our cached account changes into the trie.
    pub fn commit(&mut self) -> Result<(), Error> {
        let mut trie = self.trie_factory.from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
        self.account.commit(&mut trie)?;
        self.asset_scheme.commit(&mut trie)?;
        self.asset.commit(&mut trie)?;
        Ok(())
    }

    /// Propagate local cache into shared canonical state cache.
    fn propagate_to_global_cache(&mut self) {
        let ref mut db = self.db;
        self.account.propagate_to_global_cache(|address, item, modified| {
            db.add_to_account_cache(address, item, modified);
        });
        self.asset_scheme.propagate_to_global_cache(|address, item, modified| {
            db.add_to_asset_scheme_cache(address, item, modified);
        });
        self.asset.propagate_to_global_cache(|address, item, modified| {
            db.add_to_asset_cache(address, item, modified);
        });
    }

    /// Clear state cache
    pub fn clear(&mut self) {
        self.account.clear();
        self.asset_scheme.clear();
        self.asset.clear();
    }

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
        let default = || Account::new(0u8.into(), self.account_start_nonce);
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_account(a);
        self.account.require_item_or_from(a, default, db, from_db)
    }

    fn require_asset_scheme<'a, F>(
        &'a self,
        a: &AssetSchemeAddress,
        default: F,
    ) -> trie::Result<RefMut<'a, AssetScheme>>
    where
        F: FnOnce() -> AssetScheme, {
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_asset_scheme(a);
        self.asset_scheme.require_item_or_from(a, default, db, from_db)
    }

    fn require_asset<'a, F>(&'a self, a: &AssetAddress, default: F) -> trie::Result<RefMut<'a, Asset>>
    where
        F: FnOnce() -> Asset, {
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_asset(a);
        self.asset.require_item_or_from(a, default, db, from_db)
    }
}

impl<B> fmt::Debug for State<B>
where
    B: TopBackend + ShardBackend,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "account: {:?} asset_scheme: {:?} asset: {:?}", self.account, self.asset_scheme, self.asset)
    }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for State<StateDB> {
    fn clone(&self) -> State<StateDB> {
        State {
            db: self.db.boxed_clone(),
            root: self.root.clone(),
            id_of_checkpoints: self.id_of_checkpoints.clone(),
            account: self.account.clone(),
            asset_scheme: self.asset_scheme.clone(),
            asset: self.asset.clone(),
            account_start_nonce: self.account_start_nonce.clone(),
            trie_factory: self.trie_factory.clone(),
        }
    }
}

fn is_input_and_output_consistent(inputs: &[AssetTransferInput], outputs: &[AssetTransferOutput]) -> bool {
    let mut sum: HashMap<H256, U128> = HashMap::new();

    for input in inputs {
        let ref asset_type = input.prev_out.asset_type;
        let ref amount = input.prev_out.amount;
        let current_amount = sum.get(&asset_type).cloned().unwrap_or(U128::zero());
        sum.insert(asset_type.clone(), current_amount + U128::from(*amount));
    }
    for output in outputs {
        let ref asset_type = output.asset_type;
        let ref amount = output.amount;
        let current_amount = if let Some(current_amount) = sum.get(&asset_type) {
            if current_amount < &U128::from(*amount) {
                return false
            }
            current_amount.clone()
        } else {
            return false
        };
        let t = sum.insert(asset_type.clone(), current_amount - From::from(*amount));
        debug_assert!(t.is_some());
    }

    sum.iter().all(|(_, sum)| sum.is_zero())
}

#[cfg(test)]
mod tests {
    use ccrypto::Blake;
    use ckeys::{Generator, Random};
    use ctypes::{Address, Secret, U256};

    use super::super::parcel::{AssetOutPoint, Parcel};
    use super::super::tests::helpers::{get_temp_state, get_temp_state_db};
    use super::*;

    fn secret() -> Secret {
        Secret::blake("")
    }

    #[test]
    fn apply_empty_parcel() {
        // account_start_nonce is 0
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
        // account_start_nonce is 0
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
        // account_start_nonce is 0
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
        // account_start_nonce is 0
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
        // account_start_nonce is 0
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

        let state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
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
            let mut state = State::new(db, U256::from(0), Default::default());
            state.add_balance(&a, &U256::default()).unwrap(); // create an empty account
            state.commit().unwrap();
            state.drop()
        };
        let state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
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
            let mut state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
            assert_eq!(state.account_exists(&a).unwrap(), true);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
            state.kill_account(&a);
            state.commit().unwrap();
            assert_eq!(state.account_exists(&a).unwrap(), false);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
            state.drop()
        };

        let state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
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
        state.checkpoint(0);
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.discard_checkpoint(0);
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.checkpoint(1);
        state.add_balance(&a, &U256::from(1u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(70u64));
        state.revert_to_checkpoint(1);
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    }

    #[test]
    fn checkpoint_nested() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.checkpoint(0);
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        state.checkpoint(1);
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
        state.checkpoint(0);
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        state.checkpoint(1);
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
            // account_start_nonce is 0
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.boxed_clone_canon(&root_parent);
            State::new(state_db, U256::from(0), Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let amount = 100;
        let registrar = Some(Address::random());
        let transactions = vec![Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters,
            amount: Some(amount),
            registrar,
            nonce: 0,
        }];
        let signed_parcel = Parcel {
            fee: 5.into(),
            action: Action::ChangeShardState {
                transactions,
            },
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = signed_parcel.sender();
        let parcel_hash = signed_parcel.hash();

        let added_result = state.add_balance(&sender, &U256::from(69u64));
        assert!(added_result.is_ok());

        let minted_result =
            state.mint_asset(parcel_hash.clone(), &metadata, &lock_script_hash, &vec![], &Some(amount), &registrar);
        assert!(minted_result.is_ok());

        let commit = state.commit();
        assert!(commit.is_ok());

        let asset_scheme_address = AssetSchemeAddress::new(parcel_hash.clone());
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert!(asset_scheme.is_ok());
        let asset_scheme = asset_scheme.unwrap();
        assert!(asset_scheme.is_some());
        let asset_scheme = asset_scheme.unwrap();

        assert_eq!(&metadata, asset_scheme.metadata());
        assert_eq!(&amount, asset_scheme.amount());
        assert_eq!(&registrar, asset_scheme.registrar());
        assert!(asset_scheme.is_permissioned());
    }

    #[test]
    fn mint_infinite_asset() {
        let mut state = {
            // account_start_nonce is 0
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.boxed_clone_canon(&root_parent);
            State::new(state_db, U256::from(0), Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transactions = vec![Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters: vec![],
            amount: None,
            registrar,
            nonce: 0,
        }];
        let signed_parcel = Parcel {
            fee: 5.into(),
            action: Action::ChangeShardState {
                transactions,
            },
            ..Parcel::default()
        }.sign(&secret().into());
        let sender = signed_parcel.sender();
        let parcel_hash = signed_parcel.hash();

        let added_result = state.add_balance(&sender, &U256::from(69u64));
        assert!(added_result.is_ok());

        let minted_result =
            state.mint_asset(parcel_hash.clone(), &metadata, &lock_script_hash, &parameters, &None, &registrar);
        assert!(minted_result.is_ok());

        let commit = state.commit();
        assert!(commit.is_ok());

        let asset_scheme_address = AssetSchemeAddress::new(parcel_hash.clone());
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert!(asset_scheme.is_ok());
        let asset_scheme = asset_scheme.unwrap();
        assert!(asset_scheme.is_some());
        let asset_scheme = asset_scheme.unwrap();

        assert_eq!(&metadata, asset_scheme.metadata());
        assert_eq!(&::std::u64::MAX, asset_scheme.amount());
        assert_eq!(&registrar, asset_scheme.registrar());
        assert!(asset_scheme.is_permissioned());
    }

    #[test]
    fn test_is_input_and_output_consistent() {
        let asset_type = H256::random();
        let amount = 100;

        assert!(is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount,
            }]
        ));
    }

    #[test]
    fn test_multiple_asset_is_input_and_output_consistent() {
        let asset_type1 = H256::random();
        let asset_type2 = {
            let mut asset_type = H256::random();
            while asset_type == asset_type1 {
                asset_type = H256::random();
            }
            asset_type
        };
        let amount1 = 100;
        let amount2 = 200;

        assert!(is_input_and_output_consistent(
            &[
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        amount: amount1,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type2,
                        amount: amount2,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
            ],
            &[
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type1,
                    amount: amount1,
                },
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type2,
                    amount: amount2,
                },
            ]
        ));
    }

    #[test]
    fn test_multiple_asset_different_order_is_input_and_output_consistent() {
        let asset_type1 = H256::random();
        let asset_type2 = {
            let mut asset_type = H256::random();
            while asset_type == asset_type1 {
                asset_type = H256::random();
            }
            asset_type
        };
        let amount1 = 100;
        let amount2 = 200;

        assert!(is_input_and_output_consistent(
            &[
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        amount: amount1,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type2,
                        amount: amount2,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
            ],
            &[
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type2,
                    amount: amount2,
                },
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type1,
                    amount: amount1,
                },
            ]
        ));
    }

    #[test]
    fn test_empty_is_input_and_output_consistent() {
        assert!(is_input_and_output_consistent(&[], &[]));
    }

    #[test]
    fn output_has_more_asset() {
        let asset_type = H256::random();
        let output_amount = 100;
        assert!(!is_input_and_output_consistent(
            &[],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount: output_amount,
            }]
        ));
    }

    #[test]
    fn input_has_more_asset() {
        let asset_type = H256::random();
        let input_amount = 100;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[]
        ));
    }

    #[test]
    fn input_is_larger_than_output() {
        let asset_type = H256::random();
        let input_amount = 100;
        let output_amount = 80;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount: output_amount,
            }]
        ));
    }
}
