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
use std::collections::hash_map::Entry as HashMapEntry;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

use cbytes::Bytes;
use ctypes::{Address, H256, Public, U256, U512};
use error::Error;
use rlp::{Decodable, Encodable};
use transaction::{Action, SignedTransaction};
use trie::{self, Trie, TrieError, TrieFactory};

use super::invoice::{Invoice, TransactionOutcome};
use super::state_db::StateDB;
use super::transaction::TransactionError;

mod account;
mod asset_scheme;
pub mod backend;

pub use self::account::Account;
pub use self::asset_scheme::{AssetScheme, AssetSchemeAddress};
pub use self::backend::Backend;

/// Used to return information about an `State::apply` operation.
pub struct ApplyOutcome {
    /// The invoice for the applied transaction.
    pub invoice: Invoice,
    /// The output of the applied transaction.
    pub error: Option<TransactionError>,
}

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
/// Account modification state. Used to check if the account was
/// Modified in between commits and overall.
enum EntryState {
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
struct Entry<Item>
where
    Item: CacheableItem, {
    /// Account entry. `None` if account known to be non-existant.
    item: Option<Item>,
    /// Entry state.
    state: EntryState,
}

// Account cache item. Contains account data and
// modification state
impl<Item> Entry<Item>
where
    Item: CacheableItem,
{
    fn is_dirty(&self) -> bool {
        self.state == EntryState::Dirty
    }

    fn exists_and_is_null(&self) -> bool {
        self.item.as_ref().map_or(false, |a| a.is_null())
    }

    fn clone(&self) -> Self {
        Self {
            item: self.item.as_ref().map(Clone::clone),
            state: self.state,
        }
    }

    // Create a new account entry and mark it as dirty.
    fn new_dirty(item: Option<Item>) -> Self {
        Self {
            item,
            state: EntryState::Dirty,
        }
    }

    // Create a new account entry and mark it as clean.
    fn new_clean(item: Option<Item>) -> Self {
        Self {
            item,
            state: EntryState::CleanFresh,
        }
    }

    // Create a new account entry and mark it as clean and cached.
    fn new_clean_cached(item: Option<Item>) -> Self {
        Self {
            item,
            state: EntryState::CleanCached,
        }
    }

    // Replace data with another entry but preserve storage cache.
    fn overwrite_with(&mut self, other: Self) {
        self.state = other.state;
        match other.item {
            Some(acc) => {
                if let Some(ref mut ours) = self.item {
                    ours.overwrite_with(acc);
                }
            }
            None => self.item = None,
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
pub struct State<B: Backend> {
    db: B,
    root: H256,
    account_cache: RefCell<HashMap<Address, Entry<Account>>>,
    asset_scheme_cache: RefCell<HashMap<AssetSchemeAddress, Entry<AssetScheme>>>,
    // The original account is preserved in
    account_checkpoints: RefCell<Vec<HashMap<Address, Option<Entry<Account>>>>>,
    asset_scheme_checkpoints: RefCell<Vec<HashMap<AssetSchemeAddress, Option<Entry<AssetScheme>>>>>,
    account_start_nonce: U256,
    trie_factory: TrieFactory,
}

/// Provides subset of `State` methods to query state information
pub trait StateInfo {
    /// Get the nonce of account `a`.
    fn nonce(&self, a: &Address) -> trie::Result<U256>;

    /// Get the balance of account `a`.
    fn balance(&self, a: &Address) -> trie::Result<U256>;

    /// Get the regular key of account `a`.
    fn regular_key(&self, a: &Address) -> trie::Result<Option<Public>>;
}

impl<B: Backend> StateInfo for State<B> {
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
            db,
            root,
            account_cache: RefCell::new(HashMap::new()),
            asset_scheme_cache: RefCell::new(HashMap::new()),
            account_checkpoints: RefCell::new(Vec::new()),
            account_start_nonce,
            asset_scheme_checkpoints: RefCell::new(Vec::new()),
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
            account_cache: RefCell::new(HashMap::new()),
            asset_scheme_cache: RefCell::new(HashMap::new()),
            account_checkpoints: RefCell::new(Vec::new()),
            asset_scheme_checkpoints: RefCell::new(Vec::new()),
            account_start_nonce,
            trie_factory,
        };

        Ok(state)
    }

    /// Create a recoverable checkpoint of this state.
    pub fn checkpoint(&mut self) {
        self.account_checkpoints.get_mut().push(HashMap::new());
        self.asset_scheme_checkpoints.get_mut().push(HashMap::new());
    }

    fn discard_checkpoint_impl<Item>(checkpoints: &mut RefCell<Vec<HashMap<Item::Address, Option<Entry<Item>>>>>)
    where
        Item: CacheableItem, {
        // merge with previous checkpoint
        let last = checkpoints.get_mut().pop();
        if let Some(mut checkpoint) = last {
            if let Some(ref mut prev) = checkpoints.get_mut().last_mut() {
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

    /// Merge last checkpoint with previous.
    pub fn discard_checkpoint(&mut self) {
        Self::discard_checkpoint_impl(&mut self.account_checkpoints);
        Self::discard_checkpoint_impl(&mut self.asset_scheme_checkpoints);
    }

    fn revert_to_checkpoint_impl<Item>(
        checkpoints: &mut RefCell<Vec<HashMap<Item::Address, Option<Entry<Item>>>>>,
        cache: &mut RefCell<HashMap<Item::Address, Entry<Item>>>,
    ) where
        Item: CacheableItem, {
        if let Some(mut checkpoint) = checkpoints.get_mut().pop() {
            for (k, v) in checkpoint.drain() {
                match v {
                    Some(v) => {
                        match cache.get_mut().entry(k) {
                            HashMapEntry::Occupied(mut e) => {
                                // Merge checkpointed changes back into the main account
                                // storage preserving the cache.
                                e.get_mut().overwrite_with(v);
                            }
                            HashMapEntry::Vacant(e) => {
                                e.insert(v);
                            }
                        }
                    }
                    None => {
                        if let HashMapEntry::Occupied(e) = cache.get_mut().entry(k) {
                            if e.get().is_dirty() {
                                e.remove();
                            }
                        }
                    }
                }
            }
        }
    }

    /// Revert to the last checkpoint and discard it.
    pub fn revert_to_checkpoint(&mut self) {
        Self::revert_to_checkpoint_impl(&mut self.account_checkpoints, &mut self.account_cache);
        Self::revert_to_checkpoint_impl(&mut self.asset_scheme_checkpoints, &mut self.asset_scheme_cache);
    }

    fn insert_cache<Item>(
        &self,
        address: &Item::Address,
        item: Entry<Item>,
        cache: &RefCell<HashMap<Item::Address, Entry<Item>>>,
        checkpoints: &RefCell<Vec<HashMap<Item::Address, Option<Entry<Item>>>>>,
    ) where
        Item: CacheableItem, {
        // Dirty item which is not in the cache means this is a new item.
        // It goes directly into the checkpoint as there's nothing to rever to.
        //
        // In all other cases item is read as clean first, and after that made
        // dirty in and added to the checkpoint with `note_cache`.
        let is_dirty = item.is_dirty();
        let old_value = cache.borrow_mut().insert(address.clone(), item);
        if !is_dirty {
            return
        }
        if let Some(ref mut checkpoint) = checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(address.clone()).or_insert(old_value);
        }
    }

    fn insert_cache_account(&self, address: &Address, account: Entry<Account>) {
        self.insert_cache(address, account, &self.account_cache, &self.account_checkpoints)
    }

    fn insert_cache_asset_scheme(&self, address: &AssetSchemeAddress, asset: Entry<AssetScheme>) {
        self.insert_cache(address, asset, &self.asset_scheme_cache, &self.asset_scheme_checkpoints)
    }

    fn note_cache<Item>(
        &self,
        address: &Item::Address,
        cache: &RefCell<HashMap<Item::Address, Entry<Item>>>,
        checkpoints: &RefCell<Vec<HashMap<Item::Address, Option<Entry<Item>>>>>,
    ) where
        Item: CacheableItem, {
        if let Some(ref mut checkpoint) = checkpoints.borrow_mut().last_mut() {
            checkpoint.entry(address.clone()).or_insert_with(|| cache.borrow().get(address).map(Entry::<Item>::clone));
        }
    }

    fn note_cache_account(&self, address: &Address) {
        self.note_cache(address, &self.account_cache, &self.account_checkpoints)
    }

    fn note_cache_asset_scheme(&self, address: &AssetSchemeAddress) {
        self.note_cache(address, &self.asset_scheme_cache, &self.asset_scheme_checkpoints);
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
        self.insert_cache_account(account, Entry::<Account>::new_dirty(None));
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

    /// Add `incr` to the balance of account `a`.
    pub fn add_balance(&mut self, a: &Address, incr: &U256) -> trie::Result<()> {
        trace!(target: "state", "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        let is_value_transfer = !incr.is_zero();
        if is_value_transfer {
            self.require_account(a)?.add_balance(incr);
        }
        Ok(())
    }

    /// Subtract `decr` from the balance of account `a`.
    pub fn sub_balance(&mut self, a: &Address, decr: &U256) -> trie::Result<()> {
        trace!(target: "state", "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
        if !decr.is_zero() || !self.account_exists(a)? {
            self.require_account(a)?.sub_balance(decr);
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
        self.require_account(a).map(|mut x| x.inc_nonce())
    }

    /// Set the regular key of account `a`
    pub fn set_regular_key(&mut self, a: &Address, key: &Public) -> trie::Result<()> {
        self.require_account(a)?.set_regular_key(key);
        Ok(())
    }

    /// Execute a given transaction, charging transaction fee.
    /// This will change the state accordingly.
    pub fn apply(&mut self, t: &SignedTransaction) -> Result<ApplyOutcome, Error> {
        let error = self.execute(t)?;
        self.commit()?;

        let invoice = match error {
            Some(_) => Invoice::new(TransactionOutcome::Failed),
            None => Invoice::new(TransactionOutcome::Success),
        };
        Ok(ApplyOutcome {
            invoice,
            error,
        })
    }

    fn execute(&mut self, t: &SignedTransaction) -> Result<Option<TransactionError>, Error> {
        let sender = t.sender();
        let fee = U512::from(t.as_unsigned().fee);
        let nonce = self.nonce(&sender)?;
        let mut balance = U512::from(self.balance(&sender)?);

        if t.nonce != nonce {
            return Ok(Some(TransactionError::InvalidNonce {
                expected: nonce,
                got: t.nonce,
            }))
        }

        if fee > balance {
            return Ok(Some(TransactionError::NotEnoughCash {
                required: fee,
                got: balance,
            }))
        }

        self.inc_nonce(&sender)?;
        self.sub_balance(&sender, &fee.into())?;
        balance = balance - fee;

        match t.action {
            Action::Noop => Ok(None),
            Action::Payment {
                address,
                value,
            } => {
                if balance < value.into() {
                    return Ok(Some(TransactionError::NotEnoughCash {
                        required: fee + value.into(),
                        got: fee + balance,
                    }))
                }
                self.transfer_balance(&sender, &address, &value)?;
                // NOTE: Uncomment the below line if balance is used after
                // balance = balance - value.into()
                Ok(None)
            }
            Action::SetRegularKey {
                key,
            } => {
                self.set_regular_key(&sender, &key)?;
                Ok(None)
            }
            Action::AssetMint {
                ref metadata,
                ref registrar,
                permissioned,
                ref amount,
            } => unimplemented!(),
        }
    }

    /// Commits our cached account changes into the trie.
    pub fn commit(&mut self) -> Result<(), Error> {
        {
            let mut accounts = self.account_cache.borrow_mut();
            let mut trie = self.trie_factory.from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
            for (address, ref mut a) in accounts.iter_mut().filter(|&(_, ref a)| a.is_dirty()) {
                a.state = EntryState::Committed;
                match a.item {
                    Some(ref mut account) => {
                        trie.insert(address, &account.rlp())?;
                    }
                    None => {
                        trie.remove(address)?;
                    }
                };
            }
        }

        {
            let mut asset_schemes = self.asset_scheme_cache.borrow_mut();
            let mut trie = self.trie_factory.from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
            for (address, ref mut a) in asset_schemes.iter_mut().filter(|&(_, ref a)| a.is_dirty()) {
                a.state = EntryState::Committed;
                let ref mut asset_scheme = a.item.as_ref().expect("Removing asset_scheme is not supported");
                trie.insert(address, &asset_scheme.rlp())?;
            }
        }

        Ok(())
    }

    /// Propagate local cache into shared canonical state cache.
    fn propagate_to_global_cache(&mut self) {
        {
            let mut addresses = self.account_cache.borrow_mut();
            trace!("Committing cache {:?} entries", addresses.len());
            for (address, a) in addresses
                .drain()
                .filter(|&(_, ref a)| a.state == EntryState::Committed || a.state == EntryState::CleanFresh)
            {
                self.db.add_to_account_cache(address, a.item, a.state == EntryState::Committed);
            }
        }
        {
            let mut assets = self.asset_scheme_cache.borrow_mut();
            trace!("Committing cache {:?} entries", assets.len());
            for (address, a) in assets
                .drain()
                .filter(|&(_, ref a)| a.state == EntryState::Committed || a.state == EntryState::CleanFresh)
            {
                self.db.add_to_asset_scheme_cache(
                    address,
                    a.item.expect("Removing asset scheme is not supported feature"),
                );
            }
        }
    }

    /// Clear state cache
    pub fn clear(&mut self) {
        self.account_cache.borrow_mut().clear();
        self.asset_scheme_cache.borrow_mut().clear();
    }

    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn ensure_account_cached<F, U>(&self, a: &Address, f: F) -> trie::Result<U>
    where
        F: Fn(Option<&Account>) -> U, {
        // check local cache first
        if let Some(ref mut maybe_acc) = self.account_cache.borrow_mut().get_mut(a) {
            if let Some(ref mut account) = maybe_acc.item {
                return Ok(f(Some(account)))
            }
            return Ok(f(None))
        }
        // check global cache
        let result = self.db.get_cached_account_with(a, |acc| f(acc.map(|a| &*a)));
        match result {
            Some(r) => Ok(r),
            None => {
                // not found in the global cache, get from the DB and insert into local
                let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
                let mut maybe_acc = db.get_with(a, Account::from_rlp)?;
                let r = f(maybe_acc.as_ref());
                self.insert_cache_account(a, Entry::<Account>::new_clean(maybe_acc));
                Ok(r)
            }
        }
    }

    /// Pull account `a` in our cache from the trie DB.
    fn require_account<'a>(&'a self, a: &Address) -> trie::Result<RefMut<'a, Account>> {
        self.require_account_or_from(a, || Account::new(0u8.into(), self.account_start_nonce), |_| {})
    }

    /// Pull account `a` in our cache from the trie DB.
    /// If it doesn't exist, make account equal the evaluation of `default`.
    fn require_account_or_from<'a, F, G>(
        &'a self,
        a: &Address,
        default: F,
        not_default: G,
    ) -> trie::Result<RefMut<'a, Account>>
    where
        F: FnOnce() -> Account,
        G: FnOnce(&mut Account), {
        let contains_key = self.account_cache.borrow().contains_key(a);
        if !contains_key {
            match self.db.get_cached_account(a) {
                Some(acc) => self.insert_cache_account(a, Entry::<Account>::new_clean_cached(acc)),
                None => {
                    let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
                    let maybe_acc = Entry::<Account>::new_clean(db.get_with(a, Account::from_rlp)?);
                    self.insert_cache_account(a, maybe_acc);
                }
            }
        }
        self.note_cache_account(a);

        // at this point the entry is guaranteed to be in the cache.
        Ok(RefMut::map(self.account_cache.borrow_mut(), |c| {
            let entry = c.get_mut(a).expect("entry known to exist in the cache; qed");

            match &mut entry.item {
                &mut Some(ref mut acc) => not_default(acc),
                slot => *slot = Some(default()),
            }

            // set the dirty flag after changing account data.
            entry.state = EntryState::Dirty;
            match entry.item {
                Some(ref mut account) => account,
                _ => panic!("Required account must always exist; qed"),
            }
        }))
    }

    /// Pull account `a` in our cache from the trie DB.
    fn require_asset_scheme<'a, F>(
        &'a self,
        a: &AssetSchemeAddress,
        default: F,
    ) -> trie::Result<RefMut<'a, AssetScheme>>
    where
        F: FnOnce() -> AssetScheme, {
        self.require_asset_scheme_or_from(a, default, |_| {})
    }

    /// Pull asset `a` in our cache from the trie DB.
    /// If it doesn't exist, make asset equal the evaluation of `default`.
    fn require_asset_scheme_or_from<'a, F, G>(
        &'a self,
        a: &AssetSchemeAddress,
        default: F,
        not_default: G,
    ) -> trie::Result<RefMut<'a, AssetScheme>>
    where
        F: FnOnce() -> AssetScheme,
        G: FnOnce(&mut AssetScheme), {
        let contains_key = self.asset_scheme_cache.borrow().contains_key(a);
        if !contains_key {
            match self.db.get_cached_asset_scheme(a) {
                Some(asset_scheme) => {
                    self.insert_cache_asset_scheme(a, Entry::<AssetScheme>::new_clean_cached(asset_scheme))
                }
                None => {
                    let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
                    let maybe_asset_scheme = Entry::<AssetScheme>::new_clean(db.get_with(a, AssetScheme::from_rlp)?);
                    self.insert_cache_asset_scheme(a, maybe_asset_scheme);
                }
            }
        }
        self.note_cache_asset_scheme(a);

        // at this point the entry is guaranteed to be in the cache.
        Ok(RefMut::map(self.asset_scheme_cache.borrow_mut(), |c| {
            let entry = c.get_mut(a).expect("entry known to exist in the cache; qed");

            match &mut entry.item {
                &mut Some(ref mut asset_scheme) => not_default(asset_scheme),
                slot => *slot = Some(default()),
            }

            // set the dirty flag after changing asset_scheme data.
            entry.state = EntryState::Dirty;
            match entry.item {
                Some(ref mut asset_scheme) => asset_scheme,
                _ => panic!("Required asset_scheme must always exist; qed"),
            }
        }))
    }
}

impl<B: Backend> fmt::Debug for State<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.account_cache.borrow())
    }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for State<StateDB> {
    fn clone(&self) -> State<StateDB> {
        let account_cache = {
            let mut cache: HashMap<Address, Entry<Account>> = HashMap::new();
            for (key, val) in self.account_cache.borrow().iter() {
                if val.is_dirty() {
                    cache.insert(key.clone(), Entry::<Account>::new_dirty(val.item.clone()));
                }
            }
            RefCell::new(cache)
        };

        let asset_scheme_cache = {
            let mut cache: HashMap<AssetSchemeAddress, Entry<AssetScheme>> = HashMap::new();
            for (key, val) in self.asset_scheme_cache.borrow().iter() {
                if val.is_dirty() {
                    cache.insert(key.clone(), Entry::<AssetScheme>::new_dirty(val.item.clone()));
                }
            }
            RefCell::new(cache)
        };

        State {
            db: self.db.boxed_clone(),
            root: self.root.clone(),
            account_cache,
            asset_scheme_cache,
            account_checkpoints: RefCell::new(Vec::new()),
            asset_scheme_checkpoints: RefCell::new(Vec::new()),
            account_start_nonce: self.account_start_nonce.clone(),
            trie_factory: self.trie_factory.clone(),
        }
    }
}

pub trait CacheableItem: Clone + Decodable + Encodable {
    type Address: Clone + fmt::Debug + Eq + Hash;
    fn overwrite_with(&mut self, other: Self);
    fn is_null(&self) -> bool;

    fn from_rlp(rlp: &[u8]) -> Self;
    fn rlp(&self) -> Bytes;
}

#[cfg(test)]
mod tests {
    use ccrypto::blake256;
    use clogger::init_log;
    use ctypes::{Address, Secret, U256};

    use super::super::tests::helpers::{get_temp_state, get_temp_state_db};
    use super::super::transaction::Transaction;
    use super::*;

    fn secret() -> Secret {
        blake256("").into()
    }

    #[test]
    fn should_apply_ok() {
        // account_start_nonce is 0
        let mut state = get_temp_state();

        let t = Transaction {
            fee: 5.into(),
            ..Transaction::default()
        }.sign(&secret().into());
        let sender = t.sender();
        state.add_balance(&sender, &20.into()).unwrap();

        let res = state.apply(&t).unwrap();
        assert_eq!(res.invoice.outcome, TransactionOutcome::Success);
        assert!(res.error.is_none());
        assert_eq!(state.balance(&sender).unwrap(), 15.into());
        assert_eq!(state.nonce(&sender).unwrap(), 1.into());
    }

    #[test]
    fn should_apply_error_for_invalid_nonce() {
        // account_start_nonce is 0
        let mut state = get_temp_state();

        let t = Transaction {
            nonce: 2.into(),
            fee: 5.into(),
            ..Transaction::default()
        }.sign(&secret().into());
        let sender = t.sender();
        state.add_balance(&sender, &20.into()).unwrap();

        let res = state.apply(&t).unwrap();
        assert_eq!(res.invoice.outcome, TransactionOutcome::Failed);
        assert_eq!(
            res.error.unwrap(),
            TransactionError::InvalidNonce {
                expected: 0.into(),
                got: 2.into()
            }
        );
        assert_eq!(state.balance(&sender).unwrap(), 20.into());
        assert_eq!(state.nonce(&sender).unwrap(), 0.into());
    }

    #[test]
    fn should_apply_error_for_not_enough_cash() {
        let mut state = get_temp_state();
        let t = Transaction {
            fee: 5.into(),
            ..Transaction::default()
        }.sign(&secret().into());
        let sender = t.sender();
        state.add_balance(&sender, &4.into()).unwrap();

        let res = state.apply(&t).unwrap();
        assert_eq!(res.invoice.outcome, TransactionOutcome::Failed);
        assert_eq!(
            res.error.unwrap(),
            TransactionError::NotEnoughCash {
                required: 5.into(),
                got: 4.into()
            }
        );
        assert_eq!(state.balance(&sender).unwrap(), 4.into());
        assert_eq!(state.nonce(&sender).unwrap(), 0.into());
    }

    #[test]
    fn should_apply_payment() {
        // account_start_nonce is 0
        let mut state = get_temp_state();
        let receiver = 1u64.into();

        let t = Transaction {
            fee: 5.into(),
            action: Action::Payment {
                address: receiver,
                value: 10.into(),
            },
            ..Transaction::default()
        }.sign(&secret().into());
        let sender = t.sender();
        state.add_balance(&sender, &20.into()).unwrap();

        let res = state.apply(&t).unwrap();
        assert_eq!(res.invoice.outcome, TransactionOutcome::Success);
        assert!(res.error.is_none());
        assert_eq!(state.balance(&receiver).unwrap(), 10.into());
        assert_eq!(state.balance(&sender).unwrap(), 5.into());
        assert_eq!(state.nonce(&sender).unwrap(), 1.into());
    }

    #[test]
    fn should_apply_set_regular_key() {
        // account_start_nonce is 0
        let mut state = get_temp_state();
        let key = 1u64.into();

        let t = Transaction {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key,
            },
            ..Transaction::default()
        }.sign(&secret().into());
        let sender = t.sender();
        state.add_balance(&sender, &5.into()).unwrap();

        assert_eq!(state.regular_key(&sender).unwrap(), None);
        let res = state.apply(&t).unwrap();
        assert_eq!(res.invoice.outcome, TransactionOutcome::Success);
        assert_eq!(state.regular_key(&sender).unwrap(), Some(key));
    }

    #[test]
    fn should_apply_error_for_action_failure() {
        // account_start_nonce is 0
        let mut state = get_temp_state();
        let receiver = 1u64.into();

        let t = Transaction {
            fee: 5.into(),
            action: Action::Payment {
                address: receiver,
                value: 30.into(),
            },
            ..Transaction::default()
        }.sign(&secret().into());
        let sender = t.sender();
        state.add_balance(&sender, &20.into()).unwrap();

        let res = state.apply(&t).unwrap();
        assert_eq!(res.invoice.outcome, TransactionOutcome::Failed);
        assert_eq!(
            res.error.unwrap(),
            TransactionError::NotEnoughCash {
                required: 35.into(),
                got: 20.into()
            }
        );
        assert_eq!(state.balance(&receiver).unwrap(), 0.into());
        assert_eq!(state.balance(&sender).unwrap(), 15.into());
        assert_eq!(state.nonce(&sender).unwrap(), 1.into());
    }

    #[test]
    fn should_work_when_cloned() {
        init_log();

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
        assert_eq!(*state.root(), "4b5fdb97048c16016fb85e635a11073e375d07b692d7372ec166885e0aa6624a".into());
    }

    #[test]
    fn checkpoint_basic() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.checkpoint();
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.discard_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.checkpoint();
        state.add_balance(&a, &U256::from(1u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(70u64));
        state.revert_to_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
    }

    #[test]
    fn checkpoint_nested() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.checkpoint();
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        state.checkpoint();
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64 + 69u64));
        state.revert_to_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64));
        state.revert_to_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(0));
    }

    #[test]
    fn checkpoint_discard() {
        let mut state = get_temp_state();
        let a = Address::zero();
        state.checkpoint();
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        state.checkpoint();
        state.add_balance(&a, &U256::from(69u64)).unwrap();
        state.inc_nonce(&a).unwrap();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64 + 69u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.discard_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(69u64 + 69u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(1u64));
        state.revert_to_checkpoint();
        assert_eq!(state.balance(&a).unwrap(), U256::from(0u64));
        assert_eq!(state.nonce(&a).unwrap(), U256::from(0u64));
    }

    #[test]
    fn create_empty() {
        let mut state = get_temp_state();
        state.commit().unwrap();
        assert_eq!(*state.root(), "45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0".into());
    }
}
