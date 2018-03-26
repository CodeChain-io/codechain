// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! A mutable state representation suitable to execute transactions.
//! Generic over a `Backend`. Deals with `Account`s.
//! Unconfirmed sub-states are managed with `checkpoint`s which may be canonicalized
//! or rolled back.

use std::cell::{RefCell, RefMut};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, BTreeMap, HashSet};
use std::fmt;
use std::sync::Arc;

use error::Error;
use transaction::SignedTransaction;

use ctypes::{H256, U256, Address};
use hashdb::{HashDB, AsHashDB};
use kvdb::DBValue;

use trie;
use trie::{Trie, TrieFactory, TrieError};

mod account;

pub mod backend;

pub use self::account::Account;
pub use self::backend::Backend;

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
/// Account modification state. Used to check if the account was
/// Modified in between commits and overall.
enum AccountState {
    /// Account was loaded from disk and never modified in this state object.
    CleanFresh,
    /// Account was loaded from the global cache and never modified.
    CleanCached,
    /// Account has been modified and is not committed to the trie yet.
    /// This is set if any of the account data is changed, including
    /// storage and code.
    Dirty,
    /// Account was modified and committed to the trie.
    Committed,
}

#[derive(Debug)]
/// In-memory copy of the account data. Holds the optional account
/// and the modification status.
/// Account entry can contain existing (`Some`) or non-existing
/// account (`None`)
struct AccountEntry {
    /// Account entry. `None` if account known to be non-existant.
    account: Option<Account>,
    /// Entry state.
    state: AccountState,
}

// Account cache item. Contains account data and
// modification state
impl AccountEntry {
    fn is_dirty(&self) -> bool {
        self.state == AccountState::Dirty
    }

    fn exists_and_is_null(&self) -> bool {
        self.account.as_ref().map_or(false, |a| a.is_null())
    }

    fn clone(&self) -> AccountEntry {
        AccountEntry {
            account: self.account.as_ref().map(Account::clone),
            state: self.state,
        }
    }

    // Create a new account entry and mark it as dirty.
    fn new_dirty(account: Option<Account>) -> AccountEntry {
        AccountEntry {
            account: account,
            state: AccountState::Dirty,
        }
    }

    // Create a new account entry and mark it as clean.
    fn new_clean(account: Option<Account>) -> AccountEntry {
        AccountEntry {
            account: account,
            state: AccountState::CleanFresh,
        }
    }

    // Create a new account entry and mark it as clean and cached.
    fn new_clean_cached(account: Option<Account>) -> AccountEntry {
        AccountEntry {
            account: account,
            state: AccountState::CleanCached,
        }
    }

    // Replace data with another entry but preserve storage cache.
    fn overwrite_with(&mut self, other: AccountEntry) {
        self.state = other.state;
        match other.account {
            Some(acc) => {
                if let Some(ref mut ours) = self.account {
                    ours.overwrite_with(acc);
                }
            },
            None => self.account = None,
        }
    }
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
/// `AccountEntry`. This is done in `require` and `require_or_from`. So just
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
pub struct State<B: Backend> {
    db: B,
    root: H256,
    cache: RefCell<HashMap<Address, AccountEntry>>,
    // The original account is preserved in
    checkpoints: RefCell<Vec<HashMap<Address, Option<AccountEntry>>>>,
    account_start_nonce: U256,
    trie_factory: TrieFactory,
}

/// Provides subset of `State` methods to query state information
pub trait StateInfo {
    /// Get the nonce of account `a`.
    fn nonce(&self, a: &Address) -> trie::Result<U256>;

    /// Get the balance of account `a`.
    fn balance(&self, a: &Address) -> trie::Result<U256>;
}

impl<B: Backend> StateInfo for State<B> {
    fn nonce(&self, a: &Address) -> trie::Result<U256> { State::nonce(self, a) }
    fn balance(&self, a: &Address) -> trie::Result<U256> { State::balance(self, a) }
}

impl<B: Backend> State<B> {
    /// Creates new state with empty state root
    /// Used for tests.
    pub fn new(mut db: B, account_start_nonce: U256, trie_factory: TrieFactory) -> State<B> {
        let mut root = H256::new();
        {
            // init trie and reset root too null
            let _ = trie_factory.create(db.as_hashdb_mut(), &mut root);
        }

        State {
            db: db,
            root: root,
            cache: RefCell::new(HashMap::new()),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: account_start_nonce,
            trie_factory: trie_factory,
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(db: B, root: H256, account_start_nonce: U256, trie_factory: TrieFactory) -> Result<State<B>, TrieError> {
        if !db.as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root));
        }

        let state = State {
            db: db,
            root: root,
            cache: RefCell::new(HashMap::new()),
            checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: account_start_nonce,
            trie_factory: trie_factory
        };

        Ok(state)
    }

    /// Create a recoverable checkpoint of this state.
    pub fn checkpoint(&mut self) {
        self.checkpoints.get_mut().push(HashMap::new());
    }

    /// Merge last checkpoint with previous.
    pub fn discard_checkpoint(&mut self) {
        // merge with previous checkpoint
        let last = self.checkpoints.get_mut().pop();
        if let Some(mut checkpoint) = last {
            if let Some(ref mut prev) = self.checkpoints.get_mut().last_mut() {
                if prev.is_empty() {
                    **prev = checkpoint;
                } else {
                    for (k, v) in checkpoint.drain() {
                        prev.entry(k).or_insert(v);
                    }
                }
            }
        }
    }

    /// Revert to the last checkpoint and discard it.
    pub fn revert_to_checkpoint(&mut self) {
        if let Some(mut checkpoint) = self.checkpoints.get_mut().pop() {
            for (k, v) in checkpoint.drain() {
                match v {
                    Some(v) => {
                        match self.cache.get_mut().entry(k) {
                            Entry::Occupied(mut e) => {
                                // Merge checkpointed changes back into the main account
                                // storage preserving the cache.
                                e.get_mut().overwrite_with(v);
                            },
                            Entry::Vacant(e) => {
                                e.insert(v);
                            }
                        }
                    },
                    None => {
                        if let Entry::Occupied(e) = self.cache.get_mut().entry(k) {
                            if e.get().is_dirty() {
                                e.remove();
                            }
                        }
                    }
                }
            }
        }
    }

    fn insert_cache(&self, address: &Address, account: AccountEntry) {
        // Dirty account which is not in the cache means this is a new account.
        // It goes directly into the checkpoint as there's nothing to rever to.
        //
        // In all other cases account is read as clean first, and after that made
        // dirty in and added to the checkpoint with `note_cache`.
        let is_dirty = account.is_dirty();
        let old_value = self.cache.borrow_mut().insert(*address, account);
        if is_dirty {
            if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
                checkpoint.entry(*address).or_insert(old_value);
            }
        }
    }

    fn note_cache(&self, address: &Address) {
        if let Some(ref mut checkpoint) = self.checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(*address)
                .or_insert_with(|| self.cache.borrow().get(address).map(AccountEntry::clone));
        }
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
        self.insert_cache(account, AccountEntry::new_dirty(None));
    }

    /// Determine whether an account exists.
    pub fn exists(&self, a: &Address) -> trie::Result<bool> {
        // Bloom filter does not contain empty accounts, so it is important here to
        // check if account exists in the database directly before EIP-161 is in effect.
        self.ensure_cached(a, |a| a.is_some())
    }

    /// Determine whether an account exists and if not empty.
    pub fn exists_and_not_null(&self, a: &Address) -> trie::Result<bool> {
        self.ensure_cached(a, |a| a.map_or(false, |a| !a.is_null()))
    }

    /// Determine whether an account exists and has code or non-zero nonce.
    pub fn exists_and_has_nonce(&self, a: &Address) -> trie::Result<bool> {
        self.ensure_cached(a,
            |a| a.map_or(false, |a| *a.nonce() != self.account_start_nonce))
    }

    /// Get the balance of account `a`.
    pub fn balance(&self, a: &Address) -> trie::Result<U256> {
        self.ensure_cached(a,
            |a| a.as_ref().map_or(U256::zero(), |account| *account.balance()))
    }

    /// Get the nonce of account `a`.
    pub fn nonce(&self, a: &Address) -> trie::Result<U256> {
        self.ensure_cached(a,
            |a| a.as_ref().map_or(self.account_start_nonce, |account| *account.nonce()))
    }

    /// Add `incr` to the balance of account `a`.
    pub fn add_balance(&mut self, a: &Address, incr: &U256) -> trie::Result<()> {
        trace!(target: "state", "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        let is_value_transfer = !incr.is_zero();
        if is_value_transfer {
            self.require(a)?.add_balance(incr);
        }
        Ok(())
    }

    /// Subtract `decr` from the balance of account `a`.
    pub fn sub_balance(&mut self, a: &Address, decr: &U256) -> trie::Result<()> {
        trace!(target: "state", "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
        if !decr.is_zero() || !self.exists(a)? {
            self.require(a)?.sub_balance(decr);
        }
        Ok(())
    }

    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    pub fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) -> trie::Result<()> {
        self.sub_balance(from, by)?;
        self.add_balance(to, by)?;
        Ok(())
    }

    /// Increment the nonce of account `a` by 1.
    pub fn inc_nonce(&mut self, a: &Address) -> trie::Result<()> {
        self.require(a).map(|mut x| x.inc_nonce())
    }

    /// Execute a given transaction, charging transaction fee.
    /// This will change the state accordingly.
    pub fn apply(&mut self, t: &SignedTransaction) -> Result<(), Error> {
            // FIXME: Apply transaction using add_balance/sub_balance here.
            self.commit()?;
            Ok(())
    }

    fn touch(&mut self, a: &Address) -> trie::Result<()> {
        self.require(a)?;
        Ok(())
    }

    /// Commits our cached account changes into the trie.
    pub fn commit(&mut self) -> Result<(), Error> {
        let mut accounts = self.cache.borrow_mut();
        {
            let mut trie = self.trie_factory.from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
            for (address, ref mut a) in accounts.iter_mut().filter(|&(_, ref a)| a.is_dirty()) {
                a.state = AccountState::Committed;
                match a.account {
                    Some(ref mut account) => {
                        trie.insert(address, &account.rlp())?;
                    },
                    None => {
                        trie.remove(address)?;
                    },
                };
            }
        }

        Ok(())
    }

    /// Propagate local cache into shared canonical state cache.
    fn propagate_to_global_cache(&mut self) {
        let mut addresses = self.cache.borrow_mut();
        trace!("Committing cache {:?} entries", addresses.len());
        for (address, a) in addresses.drain().filter(|&(_, ref a)| a.state == AccountState::Committed || a.state == AccountState::CleanFresh) {
            self.db.add_to_account_cache(address, a.account, a.state == AccountState::Committed);
        }
    }

    /// Clear state cache
    pub fn clear(&mut self) {
        self.cache.borrow_mut().clear();
    }

    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn ensure_cached<F, U>(&self, a: &Address, f: F) -> trie::Result<U>
        where F: Fn(Option<&Account>) -> U {
        // check local cache first
        if let Some(ref mut maybe_acc) = self.cache.borrow_mut().get_mut(a) {
            if let Some(ref mut account) = maybe_acc.account {
                return Ok(f(Some(account)));
            }
            return Ok(f(None));
        }
        // check global cache
        let result = self.db.get_cached(a, |mut acc| {
            f(acc.map(|a| &*a))
        });
        match result {
            Some(r) => Ok(r),
            None => {
                // not found in the global cache, get from the DB and insert into local
                let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
                let mut maybe_acc = db.get_with(a, Account::from_rlp)?;
                let r = f(maybe_acc.as_ref());
                self.insert_cache(a, AccountEntry::new_clean(maybe_acc));
                Ok(r)
            }
        }
    }

    /// Pull account `a` in our cache from the trie DB.
    fn require<'a>(&'a self, a: &Address) -> trie::Result<RefMut<'a, Account>> {
        self.require_or_from(a, || Account::new(0u8.into(), self.account_start_nonce), |_|{})
    }

    /// Pull account `a` in our cache from the trie DB.
    /// If it doesn't exist, make account equal the evaluation of `default`.
    fn require_or_from<'a, F, G>(&'a self, a: &Address, default: F, not_default: G) -> trie::Result<RefMut<'a, Account>>
        where F: FnOnce() -> Account, G: FnOnce(&mut Account),
    {
        let contains_key = self.cache.borrow().contains_key(a);
        if !contains_key {
            match self.db.get_cached_account(a) {
                Some(acc) => self.insert_cache(a, AccountEntry::new_clean_cached(acc)),
                None => {
                    let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
                    let maybe_acc = AccountEntry::new_clean(db.get_with(a, Account::from_rlp)?);
                    self.insert_cache(a, maybe_acc);
                }
            }
        }
        self.note_cache(a);

        // at this point the entry is guaranteed to be in the cache.
        Ok(RefMut::map(self.cache.borrow_mut(), |c| {
            let entry = c.get_mut(a).expect("entry known to exist in the cache; qed");

            match &mut entry.account {
                &mut Some(ref mut acc) => not_default(acc),
                slot => *slot = Some(default()),
            }

            // set the dirty flag after changing account data.
            entry.state = AccountState::Dirty;
            match entry.account {
                Some(ref mut account) => {
                    account
                },
                _ => panic!("Required account must always exist; qed"),
            }
        }))
    }
}

impl<B: Backend> fmt::Debug for State<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.cache.borrow())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::str::FromStr;
    use rustc_hex::FromHex;
    use ccrypto::blake256;
    use super::*;
    use ctypes::{H256, U256, Address, Secret};
    use tests::helpers::{get_temp_state, get_temp_state_db};
    use spec::*;
    use clogger::init_log;

    fn secret() -> Secret {
        blake256("").into()
    }

    #[test]
    fn should_work_when_cloned() {
        init_log();

        let a = Address::zero();

        let mut state = {
            let mut state = get_temp_state();
            assert_eq!(state.exists(&a).unwrap(), false);
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
            state.add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty).unwrap();
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
        assert_eq!(state.exists(&a).unwrap(), false);
        assert_eq!(state.exists_and_not_null(&a).unwrap(), false);
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.exists(&a).unwrap(), true);
        assert_eq!(state.exists_and_not_null(&a).unwrap(), true);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.kill_account(&a);
        assert_eq!(state.exists(&a).unwrap(), false);
        assert_eq!(state.exists_and_not_null(&a).unwrap(), false);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn empty_account_is_not_created() {
        let a = Address::zero();
        let db = get_temp_state_db();
        let (root, db) = {
            let mut state = State::new(db, U256::from(0), Default::default());
            state.add_balance(&a, &U256::default(), CleanupMode::NoEmpty).unwrap(); // create an empty account
            state.commit().unwrap();
            state.drop()
        };
        let state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
        assert!(!state.exists(&a).unwrap());
        assert!(!state.exists_and_not_null(&a).unwrap());
    }

    #[test]
    fn empty_account_exists_when_creation_forced() {
        let a = Address::zero();
        let db = get_temp_state_db();
        let (root, db) = {
            let mut state = State::new(db, U256::from(0), Default::default());
            state.add_balance(&a, &U256::default(), CleanupMode::ForceCreate).unwrap(); // create an empty account
            state.commit().unwrap();
            state.drop()
        };
        let state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
        assert!(state.exists(&a).unwrap());
        assert!(!state.exists_and_not_null(&a).unwrap());
    }

    #[test]
    fn remove_from_database() {
        let a = Address::zero();
        let (root, db) = {
            let mut state = get_temp_state();
            state.inc_nonce(&a).unwrap();
            state.commit().unwrap();
            assert_eq!(state.exists(&a).unwrap(), true);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
            state.drop()
        };

        let (root, db) = {
            let mut state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
            assert_eq!(state.exists(&a).unwrap(), true);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
            state.kill_account(&a);
            state.commit().unwrap();
            assert_eq!(state.exists(&a).unwrap(), false);
            assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
            state.drop()
        };

        let state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
        assert_eq!(state.exists(&a).unwrap(), false);
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn alter_balance() {
        let mut state = get_temp_state();
        let a = Address::zero();
        let b = 1u64.into();
        state.add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.sub_balance(&a, &U256::from(42u64), &mut CleanupMode::NoEmpty).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(27u64));
        state.commit().unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(27u64));
        state.transfer_balance(&a, &b, &U256::from(18u64), CleanupMode::NoEmpty).unwrap();
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
        state.require(&a, false).unwrap();
        state.commit().unwrap();
        assert_eq!(*state.root(), "0ce23f3c809de377b008a4a3ee94a0834aac8bec1f86e28ffe4fdb5a15b0c785".into());
    }

    #[test]
    fn checkpoint_basic() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.checkpoint();
        state.add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.discard_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.checkpoint();
        state.add_balance(&a, &U256::from(1u64), CleanupMode::NoEmpty).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(70u64));
        state.revert_to_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    }

    #[test]
    fn checkpoint_nested() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.checkpoint();
        state.checkpoint();
        state.add_balance(&a, &U256::from(69u64), CleanupMode::NoEmpty).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.discard_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.revert_to_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(0));
    }

    #[test]
    fn create_empty() {
        let mut state = get_temp_state();
        state.commit().unwrap();
        assert_eq!(*state.root(), "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421".into());
    }

    #[test]
    fn should_kill_garbage() {
        let a = 10.into();
        let b = 20.into();
        let c = 30.into();
        let d = 40.into();
        let x = 0.into();
        let db = get_temp_state_db();
        let (root, db) = {
            let mut state = State::new(db, U256::from(0), Default::default());
            state.add_balance(&a, &U256::default(), CleanupMode::ForceCreate).unwrap(); // create an empty account
            state.add_balance(&b, &100.into(), CleanupMode::ForceCreate).unwrap(); // create a dust account
            state.add_balance(&c, &101.into(), CleanupMode::ForceCreate).unwrap(); // create a normal account
            state.add_balance(&d, &99.into(), CleanupMode::ForceCreate).unwrap(); // create another dust account
            state.commit().unwrap();
            state.drop()
        };

        let mut state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
        let mut touched = HashSet::new();
        state.add_balance(&a, &U256::default(), CleanupMode::TrackTouched(&mut touched)).unwrap(); // touch an account
        state.transfer_balance(&b, &x, &1.into(), CleanupMode::TrackTouched(&mut touched)).unwrap(); // touch an account decreasing its balance
        state.transfer_balance(&c, &x, &1.into(), CleanupMode::TrackTouched(&mut touched)).unwrap(); // touch an account decreasing its balance
        state.kill_garbage(&touched, true, &None, false).unwrap();
        assert!(!state.exists(&a).unwrap());
        assert!(state.exists(&b).unwrap());
        state.kill_garbage(&touched, true, &Some(100.into()), false).unwrap();
        assert!(!state.exists(&b).unwrap());
        assert!(state.exists(&c).unwrap());
        assert!(state.exists(&d).unwrap());
        state.kill_garbage(&touched, true, &Some(100.into()), true).unwrap();
        assert!(state.exists(&c).unwrap());
        assert!(state.exists(&d).unwrap());
    }
}
