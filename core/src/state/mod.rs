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
use pod_account::*;
use types::state_diff::StateDiff;
use transaction::SignedTransaction;
use state_db::StateDB;

use ethereum_types::{H256, U256, Address};
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
	/// Unmodified account balance.
	old_balance: Option<U256>,
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

	/// Clone dirty data into new `AccountEntry`. This includes
	/// basic account data and modified storage keys.
	/// Returns None if clean.
	fn clone_if_dirty(&self) -> Option<AccountEntry> {
		match self.is_dirty() {
			true => Some(self.clone_dirty()),
			false => None,
		}
	}

	/// Clone dirty data into new `AccountEntry`. This includes
	/// basic account data and modified storage keys.
	fn clone_dirty(&self) -> AccountEntry {
		AccountEntry {
			old_balance: self.old_balance,
			account: self.account.as_ref().map(Account::clone_dirty),
			state: self.state,
		}
	}

	// Create a new account entry and mark it as dirty.
	fn new_dirty(account: Option<Account>) -> AccountEntry {
		AccountEntry {
			old_balance: account.as_ref().map(|a| a.balance().clone()),
			account: account,
			state: AccountState::Dirty,
		}
	}

	// Create a new account entry and mark it as clean.
	fn new_clean(account: Option<Account>) -> AccountEntry {
		AccountEntry {
			old_balance: account.as_ref().map(|a| a.balance().clone()),
			account: account,
			state: AccountState::CleanFresh,
		}
	}

	// Create a new account entry and mark it as clean and cached.
	fn new_clean_cached(account: Option<Account>) -> AccountEntry {
		AccountEntry {
			old_balance: account.as_ref().map(|a| a.balance().clone()),
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

/// Mode of dealing with null accounts.
#[derive(PartialEq)]
pub enum CleanupMode<'a> {
	/// Create accounts which would be null.
	ForceCreate,
	/// Don't delete null accounts upon touching, but also don't create them.
	NoEmpty,
	/// Mark all touched accounts.
	TrackTouched(&'a mut HashSet<Address>),
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

const SEC_TRIE_DB_UNWRAP_STR: &'static str = "A state can only be created with valid root. Creating a SecTrieDB with a valid root will not fail. \
			 Therefore creating a SecTrieDB with this state's root will not fail.";

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

	/// Swap the current backend for another.
	// TODO: [rob] find a less hacky way to avoid duplication of `Client::state_at`.
	pub fn replace_backend<T: Backend>(self, backend: T) -> State<T> {
		State {
			db: backend,
			root: self.root,
			cache: self.cache,
			checkpoints: self.checkpoints,
			account_start_nonce: self.account_start_nonce,
			trie_factory: self.trie_factory,
		}
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
				.or_insert_with(|| self.cache.borrow().get(address).map(AccountEntry::clone_dirty));
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
		self.ensure_cached(a, false, |a| a.is_some())
	}

	/// Determine whether an account exists and if not empty.
	pub fn exists_and_not_null(&self, a: &Address) -> trie::Result<bool> {
		self.ensure_cached(a, false, |a| a.map_or(false, |a| !a.is_null()))
	}

	/// Determine whether an account exists and has code or non-zero nonce.
	pub fn exists_and_has_nonce(&self, a: &Address) -> trie::Result<bool> {
		self.ensure_cached(a, false,
			|a| a.map_or(false, |a| *a.nonce() != self.account_start_nonce))
	}

	/// Get the balance of account `a`.
	pub fn balance(&self, a: &Address) -> trie::Result<U256> {
		self.ensure_cached(a, true,
			|a| a.as_ref().map_or(U256::zero(), |account| *account.balance()))
	}

	/// Get the nonce of account `a`.
	pub fn nonce(&self, a: &Address) -> trie::Result<U256> {
		self.ensure_cached(a, true,
			|a| a.as_ref().map_or(self.account_start_nonce, |account| *account.nonce()))
	}

	/// Add `incr` to the balance of account `a`.
	pub fn add_balance(&mut self, a: &Address, incr: &U256, cleanup_mode: CleanupMode) -> trie::Result<()> {
		trace!(target: "state", "add_balance({}, {}): {}", a, incr, self.balance(a)?);
		let is_value_transfer = !incr.is_zero();
		if is_value_transfer || (cleanup_mode == CleanupMode::ForceCreate && !self.exists(a)?) {
			self.require(a)?.add_balance(incr);
		} else if let CleanupMode::TrackTouched(set) = cleanup_mode {
			if self.exists(a)? {
				set.insert(*a);
				self.touch(a)?;
			}
		}
		Ok(())
	}

	/// Subtract `decr` from the balance of account `a`.
	pub fn sub_balance(&mut self, a: &Address, decr: &U256, cleanup_mode: &mut CleanupMode) -> trie::Result<()> {
		trace!(target: "state", "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
		if !decr.is_zero() || !self.exists(a)? {
			self.require(a)?.sub_balance(decr);
		}
		if let CleanupMode::TrackTouched(ref mut set) = *cleanup_mode {
			set.insert(*a);
		}
		Ok(())
	}

	/// Subtracts `by` from the balance of `from` and adds it to that of `to`.
	pub fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256, mut cleanup_mode: CleanupMode) -> trie::Result<()> {
		self.sub_balance(from, by, &mut cleanup_mode)?;
		self.add_balance(to, by, cleanup_mode)?;
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

	/// Remove any touched empty or dust accounts.
	pub fn kill_garbage(&mut self, touched: &HashSet<Address>, remove_empty_touched: bool, min_balance: &Option<U256>, kill_contracts: bool) -> trie::Result<()> {
		let to_kill: HashSet<_> = {
			self.cache.borrow().iter().filter_map(|(address, ref entry)|
			if touched.contains(address) && // Check all touched accounts
				((remove_empty_touched && entry.exists_and_is_null()) // Remove all empty touched accounts.
				|| min_balance.map_or(false, |ref balance| entry.account.as_ref().map_or(false, |account|
					(account.is_basic() || kill_contracts) // Remove all basic and optionally contract accounts where balance has been decreased.
					&& account.balance() < balance && entry.old_balance.as_ref().map_or(false, |b| account.balance() < b)))) {

				Some(address.clone())
			} else { None }).collect()
		};
		for address in to_kill {
			self.kill_account(&address);
		}
		Ok(())
	}

	/// Check caches for required data
	/// First searches for account in the local, then the shared cache.
	/// Populates local cache if nothing found.
	fn ensure_cached<F, U>(&self, a: &Address, check_null: bool, f: F) -> trie::Result<U>
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
				// first check if it is not in database for sure
				if check_null && self.db.is_known_null(a) { return Ok(f(None)); }

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
		self.require_or_from(a, || Account::new_basic(0u8.into(), self.account_start_nonce), |_|{})
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
					let maybe_acc = if !self.db.is_known_null(a) {
						let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
						AccountEntry::new_clean(db.get_with(a, Account::from_rlp)?)
					} else {
						AccountEntry::new_clean(None)
					};
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

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for State<StateDB> {
	fn clone(&self) -> State<StateDB> {
		let cache = {
			let mut cache: HashMap<Address, AccountEntry> = HashMap::new();
			for (key, val) in self.cache.borrow().iter() {
				if let Some(entry) = val.clone_if_dirty() {
					cache.insert(key.clone(), entry);
				}
			}
			cache
		};

		State {
			db: self.db.boxed_clone(),
			root: self.root.clone(),
			cache: RefCell::new(cache),
			checkpoints: RefCell::new(Vec::new()),
			account_start_nonce: self.account_start_nonce.clone(),
			trie_factory: self.trie_factory.clone(),
		}
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use std::str::FromStr;
	use rustc_hex::FromHex;
	use hash::keccak;
	use super::*;
	use ethkey::Secret;
	use ethereum_types::{H256, U256, Address};
	use tests::helpers::{get_temp_state, get_temp_state_db};
	use machine::EthereumMachine;
	use vm::EnvInfo;
	use spec::*;
	use transaction::*;
	use ethcore_logger::init_log;
	use trace::{FlatTrace, TraceError, trace};
	use evm::CallType;

	fn secret() -> Secret {
		keccak("").into()
	}

	fn make_frontier_machine(max_depth: usize) -> EthereumMachine {
		let mut machine = ::ethereum::new_frontier_test_machine();
		machine.set_schedule_creation_rules(Box::new(move |s, _| s.max_depth = max_depth));
		machine
	}

	#[test]
	fn should_apply_create_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Create,
			value: 100.into(),
			data: FromHex::from_hex("601080600c6000396000f3006000355415600957005b60203560003555").unwrap(),
		}.sign(&secret(), None);

		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 0,
			action: trace::Action::Create(trace::Create {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				value: 100.into(),
				gas: 77412.into(),
				init: vec![96, 16, 128, 96, 12, 96, 0, 57, 96, 0, 243, 0, 96, 0, 53, 84, 21, 96, 9, 87, 0, 91, 96, 32, 53, 96, 0, 53, 85],
			}),
			result: trace::Res::Create(trace::CreateResult {
				gas_used: U256::from(3224),
				address: Address::from_str("8988167e088c87cd314df6d3c2b83da5acb93ace").unwrap(),
				code: vec![96, 0, 53, 84, 21, 96, 9, 87, 0, 91, 96, 32, 53, 96, 0, 53]
			}),
		}];

		assert_eq!(result.trace, expected_trace);
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
	fn should_trace_failed_create_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Create,
			value: 100.into(),
			data: FromHex::from_hex("5b600056").unwrap(),
		}.sign(&secret(), None);

		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			action: trace::Action::Create(trace::Create {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				value: 100.into(),
				gas: 78792.into(),
				init: vec![91, 96, 0, 86],
			}),
			result: trace::Res::FailedCreate(TraceError::OutOfGas),
			subtraces: 0
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_call_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("6000").unwrap()).unwrap();
		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(3),
				output: vec![]
			}),
			subtraces: 0,
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_basic_call_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(0),
				output: vec![]
			}),
			subtraces: 0,
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_call_transaction_to_builtin() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = Spec::new_test_machine();

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0x1.into()),
			value: 0.into(),
			data: vec![],
		}.sign(&secret(), None);

		let result = state.apply(&info, &machine, &t, true).unwrap();

		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: "0000000000000000000000000000000000000001".into(),
				value: 0.into(),
				gas: 79_000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(3000),
				output: vec![]
			}),
			subtraces: 0,
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_not_trace_subcall_transaction_to_builtin() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = Spec::new_test_machine();

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 0.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("600060006000600060006001610be0f1").unwrap()).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();

		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 0.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(3_721), // in post-eip150
				output: vec![]
			}),
			subtraces: 0,
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_not_trace_callcode() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = Spec::new_test_machine();

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 0.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("60006000600060006000600b611000f2").unwrap()).unwrap();
		state.init_code(&0xb.into(), FromHex::from_hex("6000").unwrap()).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();

		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 0.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: 724.into(), // in post-eip150
				output: vec![]
			}),
		}, FlatTrace {
			trace_address: vec![0].into_iter().collect(),
			subtraces: 0,
			action: trace::Action::Call(trace::Call {
				from: 0xa.into(),
				to: 0xa.into(),
				value: 0.into(),
				gas: 4096.into(),
				input: vec![],
				call_type: CallType::CallCode,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: 3.into(),
				output: vec![],
			}),
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_delegatecall_properly() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		info.number = 0x789b0;
		let machine = Spec::new_test_machine();

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 0.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("6000600060006000600b618000f4").unwrap()).unwrap();
		state.init_code(&0xb.into(), FromHex::from_hex("60056000526001601ff3").unwrap()).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();

		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 0.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(736), // in post-eip150
				output: vec![]
			}),
		}, FlatTrace {
			trace_address: vec![0].into_iter().collect(),
			subtraces: 0,
			action: trace::Action::Call(trace::Call {
				from: 0xa.into(),
				to: 0xb.into(),
				value: 0.into(),
				gas: 32768.into(),
				input: vec![],
				call_type: CallType::DelegateCall,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: 18.into(),
				output: vec![5],
			}),
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_failed_call_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("5b600056").unwrap()).unwrap();
		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::FailedCall(TraceError::OutOfGas),
			subtraces: 0,
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_call_with_subcall_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("60006000600060006000600b602b5a03f1").unwrap()).unwrap();
		state.init_code(&0xb.into(), FromHex::from_hex("6000").unwrap()).unwrap();
		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();

		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(69),
				output: vec![]
			}),
		}, FlatTrace {
			trace_address: vec![0].into_iter().collect(),
			subtraces: 0,
			action: trace::Action::Call(trace::Call {
				from: 0xa.into(),
				to: 0xb.into(),
				value: 0.into(),
				gas: 78934.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(3),
				output: vec![]
			}),
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_call_with_basic_subcall_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("60006000600060006045600b6000f1").unwrap()).unwrap();
		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(31761),
				output: vec![]
			}),
		}, FlatTrace {
			trace_address: vec![0].into_iter().collect(),
			subtraces: 0,
			action: trace::Action::Call(trace::Call {
				from: 0xa.into(),
				to: 0xb.into(),
				value: 69.into(),
				gas: 2300.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult::default()),
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_not_trace_call_with_invalid_basic_subcall_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("600060006000600060ff600b6000f1").unwrap()).unwrap();	// not enough funds.
		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 0,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(31761),
				output: vec![]
			}),
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_failed_subcall_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],//600480600b6000396000f35b600056
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("60006000600060006000600b602b5a03f1").unwrap()).unwrap();
		state.init_code(&0xb.into(), FromHex::from_hex("5b600056").unwrap()).unwrap();
		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(79_000),
				output: vec![]
			}),
		}, FlatTrace {
			trace_address: vec![0].into_iter().collect(),
			subtraces: 0,
			action: trace::Action::Call(trace::Call {
				from: 0xa.into(),
				to: 0xb.into(),
				value: 0.into(),
				gas: 78934.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::FailedCall(TraceError::OutOfGas),
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_call_with_subcall_with_subcall_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("60006000600060006000600b602b5a03f1").unwrap()).unwrap();
		state.init_code(&0xb.into(), FromHex::from_hex("60006000600060006000600c602b5a03f1").unwrap()).unwrap();
		state.init_code(&0xc.into(), FromHex::from_hex("6000").unwrap()).unwrap();
		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(135),
				output: vec![]
			}),
		}, FlatTrace {
			trace_address: vec![0].into_iter().collect(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: 0xa.into(),
				to: 0xb.into(),
				value: 0.into(),
				gas: 78934.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(69),
				output: vec![]
			}),
		}, FlatTrace {
			trace_address: vec![0, 0].into_iter().collect(),
			subtraces: 0,
			action: trace::Action::Call(trace::Call {
				from: 0xb.into(),
				to: 0xc.into(),
				value: 0.into(),
				gas: 78868.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(3),
				output: vec![]
			}),
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_failed_subcall_with_subcall_transaction() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],//600480600b6000396000f35b600056
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("60006000600060006000600b602b5a03f1").unwrap()).unwrap();
		state.init_code(&0xb.into(), FromHex::from_hex("60006000600060006000600c602b5a03f1505b601256").unwrap()).unwrap();
		state.init_code(&0xc.into(), FromHex::from_hex("6000").unwrap()).unwrap();
		state.add_balance(&t.sender(), &(100.into()), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();

		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(79_000),
				output: vec![]
			})
		}, FlatTrace {
			trace_address: vec![0].into_iter().collect(),
			subtraces: 1,
				action: trace::Action::Call(trace::Call {
				from: 0xa.into(),
				to: 0xb.into(),
				value: 0.into(),
				gas: 78934.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::FailedCall(TraceError::OutOfGas),
		}, FlatTrace {
			trace_address: vec![0, 0].into_iter().collect(),
			subtraces: 0,
			action: trace::Action::Call(trace::Call {
				from: 0xb.into(),
				to: 0xc.into(),
				value: 0.into(),
				gas: 78868.into(),
				call_type: CallType::Call,
				input: vec![],
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: U256::from(3),
				output: vec![]
			}),
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn should_trace_suicide() {
		init_log();

		let mut state = get_temp_state();

		let mut info = EnvInfo::default();
		info.gas_limit = 1_000_000.into();
		let machine = make_frontier_machine(5);

		let t = Transaction {
			nonce: 0.into(),
			gas_price: 0.into(),
			gas: 100_000.into(),
			action: Action::Call(0xa.into()),
			value: 100.into(),
			data: vec![],
		}.sign(&secret(), None);

		state.init_code(&0xa.into(), FromHex::from_hex("73000000000000000000000000000000000000000bff").unwrap()).unwrap();
		state.add_balance(&0xa.into(), &50.into(), CleanupMode::NoEmpty).unwrap();
		state.add_balance(&t.sender(), &100.into(), CleanupMode::NoEmpty).unwrap();
		let result = state.apply(&info, &machine, &t, true).unwrap();
		let expected_trace = vec![FlatTrace {
			trace_address: Default::default(),
			subtraces: 1,
			action: trace::Action::Call(trace::Call {
				from: "9cce34f7ab185c7aba1b7c8140d620b4bda941d6".into(),
				to: 0xa.into(),
				value: 100.into(),
				gas: 79000.into(),
				input: vec![],
				call_type: CallType::Call,
			}),
			result: trace::Res::Call(trace::CallResult {
				gas_used: 3.into(),
				output: vec![]
			}),
		}, FlatTrace {
			trace_address: vec![0].into_iter().collect(),
			subtraces: 0,
			action: trace::Action::Suicide(trace::Suicide {
				address: 0xa.into(),
				refund_address: 0xb.into(),
				balance: 150.into(),
			}),
			result: trace::Res::None,
		}];

		assert_eq!(result.trace, expected_trace);
	}

	#[test]
	fn code_from_database() {
		let a = Address::zero();
		let (root, db) = {
			let mut state = get_temp_state();
			state.require_or_from(&a, false, ||Account::new_contract(42.into(), 0.into()), |_|{}).unwrap();
			state.init_code(&a, vec![1, 2, 3]).unwrap();
			assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
			state.commit().unwrap();
			assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
			state.drop()
		};

		let state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
		assert_eq!(state.code(&a).unwrap(), Some(Arc::new(vec![1u8, 2, 3])));
	}

	#[test]
	fn storage_at_from_database() {
		let a = Address::zero();
		let (root, db) = {
			let mut state = get_temp_state();
			state.set_storage(&a, H256::from(&U256::from(1u64)), H256::from(&U256::from(69u64))).unwrap();
			state.commit().unwrap();
			state.drop()
		};

		let s = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
		assert_eq!(s.storage_at(&a, &H256::from(&U256::from(1u64))).unwrap(), H256::from(&U256::from(69u64)));
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
	fn should_not_panic_on_state_diff_with_storage() {
		let mut state = get_temp_state();

		let a: Address = 0xa.into();
		state.init_code(&a, b"abcdefg".to_vec()).unwrap();;
		state.add_balance(&a, &256.into(), CleanupMode::NoEmpty).unwrap();
		state.set_storage(&a, 0xb.into(), 0xc.into()).unwrap();

		let mut new_state = state.clone();
		new_state.set_storage(&a, 0xb.into(), 0xd.into()).unwrap();

		new_state.diff_from(state).unwrap();
	}

	#[test]
	fn should_kill_garbage() {
		let a = 10.into();
		let b = 20.into();
		let c = 30.into();
		let d = 40.into();
		let e = 50.into();
		let x = 0.into();
		let db = get_temp_state_db();
		let (root, db) = {
			let mut state = State::new(db, U256::from(0), Default::default());
			state.add_balance(&a, &U256::default(), CleanupMode::ForceCreate).unwrap(); // create an empty account
			state.add_balance(&b, &100.into(), CleanupMode::ForceCreate).unwrap(); // create a dust account
			state.add_balance(&c, &101.into(), CleanupMode::ForceCreate).unwrap(); // create a normal account
			state.add_balance(&d, &99.into(), CleanupMode::ForceCreate).unwrap(); // create another dust account
			state.new_contract(&e, 100.into(), 1.into()); // create a contract account
			state.init_code(&e, vec![0x00]).unwrap();
			state.commit().unwrap();
			state.drop()
		};

		let mut state = State::from_existing(db, root, U256::from(0u8), Default::default()).unwrap();
		let mut touched = HashSet::new();
		state.add_balance(&a, &U256::default(), CleanupMode::TrackTouched(&mut touched)).unwrap(); // touch an account
		state.transfer_balance(&b, &x, &1.into(), CleanupMode::TrackTouched(&mut touched)).unwrap(); // touch an account decreasing its balance
		state.transfer_balance(&c, &x, &1.into(), CleanupMode::TrackTouched(&mut touched)).unwrap(); // touch an account decreasing its balance
		state.transfer_balance(&e, &x, &1.into(), CleanupMode::TrackTouched(&mut touched)).unwrap(); // touch an account decreasing its balance
		state.kill_garbage(&touched, true, &None, false).unwrap();
		assert!(!state.exists(&a).unwrap());
		assert!(state.exists(&b).unwrap());
		state.kill_garbage(&touched, true, &Some(100.into()), false).unwrap();
		assert!(!state.exists(&b).unwrap());
		assert!(state.exists(&c).unwrap());
		assert!(state.exists(&d).unwrap());
		assert!(state.exists(&e).unwrap());
		state.kill_garbage(&touched, true, &Some(100.into()), true).unwrap();
		assert!(state.exists(&c).unwrap());
		assert!(state.exists(&d).unwrap());
		assert!(!state.exists(&e).unwrap());
	}
}
