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

use std::cell::{RefCell, RefMut};
use std::collections::HashMap;

use ccrypto::BLAKE_NULL_RLP;
use ckey::{public_to_address, Address, NetworkId, Public};
use cmerkle::{Result as TrieResult, TrieError, TrieFactory};
use ctypes::invoice::Invoice;
use ctypes::parcel::{Action, Error as ParcelError, Parcel};
use ctypes::transaction::{AssetWrapCCCOutput, InnerTransaction, Transaction};
use ctypes::util::unexpected::Mismatch;
use ctypes::ShardId;
use cvm::ChainTimeInfo;
use hashdb::AsHashDB;
use kvdb::DBTransaction;
use primitives::{Bytes, H160, H256, U256};
use util_error::UtilError;

use super::super::cache::{ShardCache, TopCache};
use super::super::checkpoint::{CheckpointId, StateWithCheckpoint};
use super::super::traits::{ShardState, ShardStateView, StateWithCache, TopState, TopStateView};
use super::super::{
    Account, ActionData, Metadata, MetadataAddress, RegularAccount, RegularAccountAddress, Shard, ShardAddress,
    ShardLevelState, StateDB, StateError, StateResult,
};

/// Representation of the entire state of all accounts in the system.
///
/// Local cache contains changes made locally and changes accumulated
/// locally from previous commits.
///
/// **** IMPORTANT *************************************************************
/// All the modifications to the account data must set the `Dirty` state in the
/// `Entry<Item>`. This is done in `require` and `require_or_from`. So just
/// use that.
/// ****************************************************************************
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
    db: RefCell<StateDB>,
    root: H256,

    top_cache: TopCache,
    shard_caches: HashMap<ShardId, ShardCache>,
    id_of_checkpoints: Vec<CheckpointId>,
}

impl TopStateView for TopLevelState {
    /// Check caches for required data
    /// First searches for account in the local, then the shared cache.
    /// Populates local cache if nothing found.
    fn account(&self, a: &Address) -> TrieResult<Option<Account>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.top_cache.account(&a, trie)
    }

    fn regular_account_by_address(&self, a: &Address) -> TrieResult<Option<RegularAccount>> {
        let a = RegularAccountAddress::from_address(a);
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        Ok(self.top_cache.regular_account(&a, trie)?)
    }

    fn metadata(&self) -> TrieResult<Option<Metadata>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        let address = MetadataAddress::new();
        self.top_cache.metadata(&address, trie)
    }

    fn shard(&self, shard_id: ShardId) -> TrieResult<Option<Shard>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        let shard_address = ShardAddress::new(shard_id);
        self.top_cache.shard(&shard_address, trie)
    }

    fn shard_state<'db>(&'db self, shard_id: ShardId) -> TrieResult<Option<Box<ShardStateView + 'db>>> {
        match self.shard_root(shard_id)? {
            // FIXME: Find a way to use stored cache.
            Some(shard_root) => {
                let shard_cache = self.shard_caches.get(&shard_id).cloned().unwrap_or_default();
                Ok(Some(Box::new(ShardLevelState::read_only(&self.db, shard_root, shard_cache)?)))
            }
            None => Ok(None),
        }
    }

    fn action_data(&self, key: &H256) -> TrieResult<Option<ActionData>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        Ok(self.top_cache.action_data(key, trie)?.map(Into::into))
    }
}

impl StateWithCache for TopLevelState {
    fn commit(&mut self) -> StateResult<H256> {
        let shard_ids: Vec<_> = self.shard_caches.iter().map(|(shard_id, _)| *shard_id).collect();
        let shard_changes = shard_ids
            .into_iter()
            .map(|shard_id| {
                let shard_root = self.shard_root(shard_id)?.expect("Shard must exist");
                Ok((shard_id, shard_root))
            })
            .collect::<StateResult<Vec<_>>>()?;
        for (shard_id, mut shard_root) in shard_changes.into_iter() {
            {
                let mut db = self.db.borrow_mut();
                let mut trie = TrieFactory::from_existing(db.as_hashdb_mut(), &mut shard_root)?;

                let mut shard_cache = self.shard_caches.get_mut(&shard_id).expect("Shard must exist");

                shard_cache.commit(&mut trie)?;
            }
            self.set_shard_root(shard_id, shard_root)?;
        }
        {
            let mut db = self.db.borrow_mut();
            let mut trie = TrieFactory::from_existing(db.as_hashdb_mut(), &mut self.root)?;
            self.top_cache.commit(&mut trie)?;
        }
        Ok(self.root)
    }
}

const PARCEL_FEE_CHECKPOINT: CheckpointId = 123;
const PARCEL_ACTION_CHECKPOINT: CheckpointId = 130;

impl StateWithCheckpoint for TopLevelState {
    fn create_checkpoint(&mut self, id: CheckpointId) {
        ctrace!(STATE, "Checkpoint({}) for top level is created", id);
        self.id_of_checkpoints.push(id);
        self.top_cache.checkpoint();

        for (_, mut cache) in self.shard_caches.iter_mut() {
            cache.checkpoint()
        }
    }

    fn discard_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        ctrace!(STATE, "Checkpoint({}) for top level is discarded", id);
        self.top_cache.discard_checkpoint();

        for (_, mut cache) in self.shard_caches.iter_mut() {
            cache.discard_checkpoint();
        }
    }

    fn revert_to_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        ctrace!(STATE, "Checkpoint({}) for top level is reverted", id);
        self.top_cache.revert_to_checkpoint();

        for (_, mut cache) in self.shard_caches.iter_mut() {
            cache.revert_to_checkpoint();
        }
    }
}

impl TopLevelState {
    /// Creates new state with empty state root
    /// Used for tests.
    #[cfg(test)]
    pub fn new(mut db: StateDB) -> Self {
        let root = {
            let mut root = H256::new();
            // init trie and reset root too null
            let _ = TrieFactory::create(db.as_hashdb_mut(), &mut root);
            root
        };

        let top_cache = db.top_cache();
        let shard_caches = db.shard_caches();

        TopLevelState {
            db: RefCell::new(db),
            root,
            top_cache,
            shard_caches,
            id_of_checkpoints: Default::default(),
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(db: StateDB, root: H256) -> Result<Self, TrieError> {
        if !db.as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        let top_cache = db.top_cache();
        let shard_caches = db.shard_caches();

        let state = TopLevelState {
            db: RefCell::new(db),
            root,
            top_cache,
            shard_caches,
            id_of_checkpoints: Default::default(),
        };

        Ok(state)
    }

    /// Execute a given parcel, charging parcel fee.
    /// This will change the state accordingly.
    pub fn apply<C: ChainTimeInfo>(
        &mut self,
        parcel: &Parcel,
        signer_public: &Public,
        client: &C,
    ) -> StateResult<Invoice> {
        // Change the public to an owner address if it is a regular key.
        let fee_payer = if self.regular_account_exists_and_not_null(signer_public)? {
            let regular_account = self.get_regular_account_mut(signer_public)?;
            public_to_address(&regular_account.owner_public())
        } else {
            public_to_address(signer_public)
        };

        self.create_checkpoint(PARCEL_FEE_CHECKPOINT);

        match self.apply_internal(parcel, &fee_payer, signer_public, client) {
            Err(StateError::Transaction(err)) => unreachable!("{:?}", err),
            Err(err) => {
                self.revert_to_checkpoint(PARCEL_FEE_CHECKPOINT);
                Err(err)
            }
            Ok(invoice) => {
                self.discard_checkpoint(PARCEL_FEE_CHECKPOINT);
                Ok(invoice)
            }
        }
    }

    fn apply_internal<C: ChainTimeInfo>(
        &mut self,
        parcel: &Parcel,
        fee_payer: &Address,
        signer_public: &Public,
        client: &C,
    ) -> StateResult<Invoice> {
        let seq = self.seq(fee_payer)?;

        if parcel.seq != seq {
            return Err(ParcelError::InvalidSeq(Mismatch {
                expected: seq,
                found: parcel.seq,
            })
            .into())
        }

        let fee = parcel.fee;
        let balance = self.balance(fee_payer)?;
        if fee > balance {
            return Err(ParcelError::InsufficientBalance {
                address: *fee_payer,
                cost: fee,
                balance,
            }
            .into())
        }

        self.inc_seq(fee_payer)?;
        self.sub_balance(fee_payer, &fee)?;

        // The failed parcel also must pay the fee and increase seq.
        self.create_checkpoint(PARCEL_ACTION_CHECKPOINT);

        match self.apply_action(&parcel.action, &parcel.network_id, &parcel.hash(), fee_payer, signer_public, client) {
            Ok(invoice) => {
                self.discard_checkpoint(PARCEL_ACTION_CHECKPOINT);
                Ok(invoice)
            }
            Err(StateError::Parcel(ParcelError::ParcelAlreadyImported)) => {
                unreachable!();
            }
            Err(StateError::Parcel(ParcelError::Old)) => {
                unreachable!();
            }
            Err(StateError::Parcel(ParcelError::TooCheapToReplace)) => {
                unreachable!();
            }
            Err(StateError::Parcel(ParcelError::InvalidNetworkId(_))) => {
                unreachable!();
            }
            Err(StateError::Parcel(ParcelError::LimitReached)) => {
                unreachable!();
            }
            Err(StateError::Parcel(err)) => {
                self.revert_to_checkpoint(PARCEL_ACTION_CHECKPOINT);
                Ok(Invoice::Failure(err))
            }
            Err(StateError::Transaction(err)) => {
                self.revert_to_checkpoint(PARCEL_ACTION_CHECKPOINT);
                Ok(Invoice::Failure(err.into()))
            }
            Err(err) => {
                self.revert_to_checkpoint(PARCEL_ACTION_CHECKPOINT);
                Err(err)
            }
        }
    }

    fn apply_action<C: ChainTimeInfo>(
        &mut self,
        action: &Action,
        network_id: &NetworkId,
        parcel_hash: &H256,
        fee_payer: &Address,
        signer_public: &Public,
        client: &C,
    ) -> StateResult<Invoice> {
        match action {
            Action::AssetTransaction(transaction) => {
                debug_assert_eq!(network_id, &transaction.network_id());
                Ok(self.apply_transaction(transaction, fee_payer, client)?)
            }
            Action::Payment {
                receiver,
                amount,
            } => match self.transfer_balance(fee_payer, receiver, amount) {
                Ok(()) => Ok(Invoice::Success),
                Err(err) => Err(err.into()),
            },
            Action::SetRegularKey {
                key,
            } => match self.set_regular_key(signer_public, key) {
                Ok(()) => Ok(Invoice::Success),
                Err(error) => Err(error.into()),
            },
            Action::CreateShard => {
                // FIXME: Make shard creation cost configurable
                #[cfg(test)]
                let shard_creation_cost = 1.into();
                #[cfg(not(test))]
                let shard_creation_cost = U256::max_value();

                self.create_shard(&shard_creation_cost, fee_payer)?;
                Ok(Invoice::Success)
            }
            Action::SetShardOwners {
                shard_id,
                owners,
            } => {
                self.change_shard_owners(*shard_id, owners, fee_payer)?;
                Ok(Invoice::Success)
            }
            Action::SetShardUsers {
                shard_id,
                users,
            } => {
                self.change_shard_users(*shard_id, users, fee_payer)?;
                Ok(Invoice::Success)
            }
            Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount,
            } => Ok(self.apply_wrap_ccc(
                *network_id,
                *shard_id,
                *parcel_hash,
                *lock_script_hash,
                parameters.clone(),
                *amount,
                fee_payer,
                client,
            )?),
            Action::Custom(bytes) => {
                let handlers = self.db.borrow().custom_handlers().to_vec();
                for h in handlers {
                    if let Some(result) = h.execute(bytes, self) {
                        return result
                    }
                }
                panic!("Unknown custom parcel accepted!")
            }
        }
    }

    fn apply_wrap_ccc<C: ChainTimeInfo>(
        &mut self,
        network_id: NetworkId,
        shard_id: ShardId,
        parcel_hash: H256,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        amount: U256,
        sender: &Address,
        client: &C,
    ) -> StateResult<Invoice> {
        let shard_root = self.shard_root(shard_id)?.ok_or_else(|| ParcelError::InvalidShardId(shard_id))?;
        let shard_users = self.shard_users(shard_id)?.expect("Shard must exist");

        let balance = self.balance(sender)?;
        if amount > balance {
            return Err(ParcelError::InsufficientBalance {
                address: *sender,
                cost: amount,
                balance,
            }
            .into())
        }

        let transaction = InnerTransaction::AssetWrapCCC {
            network_id,
            shard_id,
            parcel_hash,
            output: AssetWrapCCCOutput {
                lock_script_hash,
                parameters,
                amount,
            },
        };

        let invoice = {
            let shard_cache = self.shard_caches.entry(shard_id).or_default();
            let mut shard_level_state =
                ShardLevelState::from_existing(shard_id, &mut self.db, shard_root, shard_cache)?;
            shard_level_state.apply(&transaction, sender, &shard_users, client)?
        };

        if invoice == Invoice::Success {
            self.sub_balance(sender, &amount)?;
        }
        Ok(invoice)
    }

    pub fn apply_transaction<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        sender: &Address,
        client: &C,
    ) -> StateResult<Invoice> {
        let shard_ids = transaction.related_shards();

        let first_invoice = self.apply_transaction_for_shard(transaction, shard_ids[0], sender, client)?;

        for shard_id in shard_ids.iter().skip(1) {
            let invoice = self.apply_transaction_for_shard(transaction, *shard_id, sender, client)?;
            if invoice != first_invoice {
                return Err(ParcelError::InconsistentShardOutcomes.into())
            }
        }

        let unwrapped_amount = transaction.unwrapped_amount();
        if !unwrapped_amount.is_zero() {
            self.add_balance(sender, &unwrapped_amount)?;
        }
        Ok(first_invoice)
    }

    fn apply_transaction_for_shard<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        shard_id: ShardId,
        sender: &Address,
        client: &C,
    ) -> StateResult<Invoice> {
        let shard_root = self.shard_root(shard_id)?.ok_or_else(|| ParcelError::InvalidShardId(shard_id))?;
        let shard_users = self.shard_users(shard_id)?.expect("Shard must exist");

        let shard_cache = self.shard_caches.entry(shard_id).or_default();
        let mut shard_level_state = ShardLevelState::from_existing(shard_id, &mut self.db, shard_root, shard_cache)?;
        shard_level_state.apply(&transaction.clone().into(), sender, &shard_users, client)
    }

    fn create_shard_level_state(&mut self, owners: Vec<Address>, users: Vec<Address>) -> StateResult<()> {
        let shard_id = {
            let mut metadata = self.get_metadata_mut()?;
            metadata.increase_number_of_shards()
        };
        const DEFAULT_SHARD_ROOT: H256 = BLAKE_NULL_RLP;
        {
            let shard_cache = self.shard_caches.entry(shard_id).or_default();
            ShardLevelState::from_existing(shard_id, &mut self.db, DEFAULT_SHARD_ROOT, shard_cache)?;
        }

        ctrace!(STATE, "shard({}) created. owners: {:?}, users: {:?}", shard_id, owners, users);

        self.set_shard_root(shard_id, DEFAULT_SHARD_ROOT)?;
        self.set_shard_owners(shard_id, owners)?;
        self.set_shard_users(shard_id, users)?;
        Ok(())
    }

    fn get_account_mut(&self, a: &Address) -> TrieResult<RefMut<Account>> {
        debug_assert_eq!(Ok(false), self.regular_account_exists_and_not_null_by_address(a));

        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.top_cache.account_mut(&a, trie)
    }

    fn get_regular_account_mut(&self, public: &Public) -> TrieResult<RefMut<RegularAccount>> {
        let regular_account_address = RegularAccountAddress::new(public);
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.top_cache.regular_account_mut(&regular_account_address, trie)
    }

    fn get_metadata_mut(&self) -> TrieResult<RefMut<Metadata>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        let address = MetadataAddress::new();
        self.top_cache.metadata_mut(&address, trie)
    }

    fn get_shard_mut(&self, shard_id: ShardId) -> TrieResult<RefMut<Shard>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        let shard_address = ShardAddress::new(shard_id);
        self.top_cache.shard_mut(&shard_address, trie)
    }

    fn get_action_data_mut(&self, key: &H256) -> TrieResult<RefMut<ActionData>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.top_cache.action_data_mut(key, trie)
    }

    pub fn journal_under(&self, batch: &mut DBTransaction, now: u64) -> Result<u32, UtilError> {
        self.db.borrow_mut().journal_under(batch, now, self.root)
    }

    pub fn top_cache(&self) -> &TopCache {
        &self.top_cache
    }
    pub fn shard_caches(&self) -> &HashMap<ShardId, ShardCache> {
        &self.shard_caches
    }

    pub fn root(&self) -> H256 {
        self.root
    }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for TopLevelState {
    fn clone(&self) -> TopLevelState {
        TopLevelState {
            db: RefCell::new(self.db.borrow().clone(&self.root)),
            root: self.root.clone(),
            id_of_checkpoints: self.id_of_checkpoints.clone(),
            top_cache: self.top_cache.clone(),
            shard_caches: self.shard_caches.clone(),
        }
    }
}

impl TopState for TopLevelState {
    fn kill_account(&mut self, account: &Address) {
        self.top_cache.remove_account(account);
    }

    fn kill_regular_account(&mut self, account: &Public) {
        self.top_cache.remove_regular_account(&RegularAccountAddress::new(account));
    }

    fn add_balance(&mut self, a: &Address, incr: &U256) -> TrieResult<()> {
        ctrace!(STATE, "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        let is_value_transfer = !incr.is_zero();
        if is_value_transfer {
            self.get_account_mut(a)?.add_balance(incr);
        }
        Ok(())
    }

    fn sub_balance(&mut self, a: &Address, decr: &U256) -> TrieResult<()> {
        ctrace!(STATE, "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
        if !decr.is_zero() || !self.account_exists(a)? {
            self.get_account_mut(a)?.sub_balance(decr);
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
            }
            .into())
        }
        if self.regular_account_exists_and_not_null_by_address(to)? {
            return Err(ParcelError::InvalidTransferDestination.into())
        }
        self.sub_balance(from, by)?;
        self.add_balance(to, by)?;
        Ok(())
    }

    fn inc_seq(&mut self, a: &Address) -> TrieResult<()> {
        self.get_account_mut(a)?.inc_seq();
        Ok(())
    }

    fn set_regular_key(&mut self, signer_public: &Public, regular_key: &Public) -> StateResult<()> {
        let (owner_public, owner_address) = if self.regular_account_exists_and_not_null(signer_public)? {
            let regular_account = self.get_regular_account_mut(&signer_public)?;
            let owner_public = regular_account.owner_public().clone();
            let owner_address = public_to_address(&owner_public);
            (owner_public, owner_address)
        } else {
            (*signer_public, public_to_address(&signer_public))
        };

        if self.regular_account_exists_and_not_null(regular_key)? {
            return Err(ParcelError::RegularKeyAlreadyInUse.into())
        }

        let regular_address = public_to_address(regular_key);
        if self.account_exists_and_not_null(&regular_address)? {
            return Err(ParcelError::RegularKeyAlreadyInUseAsPlatformAccount.into())
        }

        let prev_regular_key = self.get_account_mut(&owner_address)?.regular_key();

        if let Some(prev_regular_key) = prev_regular_key {
            self.kill_regular_account(&prev_regular_key);
        }

        let mut owner_account = self.get_account_mut(&owner_address)?;
        owner_account.set_regular_key(regular_key);
        self.get_regular_account_mut(&regular_key)?.set_owner_public(&owner_public);
        Ok(())
    }

    fn create_shard(&mut self, shard_creation_cost: &U256, fee_payer: &Address) -> StateResult<()> {
        let balance = self.balance(fee_payer)?;
        if &balance < shard_creation_cost {
            return Err(ParcelError::InsufficientBalance {
                address: *fee_payer,
                cost: *shard_creation_cost,
                balance,
            }
            .into())
        }
        self.sub_balance(fee_payer, shard_creation_cost)?;

        self.create_shard_level_state(vec![*fee_payer], vec![])?;

        Ok(())
    }

    fn change_shard_owners(&mut self, shard_id: ShardId, owners: &[Address], sender: &Address) -> StateResult<()> {
        let old_owners = self.shard_owners(shard_id)?.ok_or_else(|| ParcelError::InvalidShardId(shard_id))?;
        if !old_owners.contains(sender) {
            return Err(ParcelError::InsufficientPermission.into())
        }
        if !owners.contains(sender) {
            return Err(ParcelError::NewOwnersMustContainSender.into())
        }

        self.set_shard_owners(shard_id, owners.to_vec())
    }

    fn change_shard_users(&mut self, shard_id: ShardId, users: &[Address], sender: &Address) -> StateResult<()> {
        let owners = self.shard_owners(shard_id)?.ok_or_else(|| ParcelError::InvalidShardId(shard_id))?;
        if !owners.contains(sender) {
            return Err(ParcelError::InsufficientPermission.into())
        }

        self.set_shard_users(shard_id, users.to_vec())
    }

    fn set_shard_root(&mut self, shard_id: ShardId, new_root: H256) -> StateResult<()> {
        let mut shard = self.get_shard_mut(shard_id)?;
        shard.set_root(new_root);
        Ok(())
    }

    fn set_shard_owners(&mut self, shard_id: ShardId, new_owners: Vec<Address>) -> StateResult<()> {
        let mut shard = self.get_shard_mut(shard_id)?;
        shard.set_owners(new_owners);
        Ok(())
    }

    fn set_shard_users(&mut self, shard_id: ShardId, new_users: Vec<Address>) -> StateResult<()> {
        let mut shard = self.get_shard_mut(shard_id)?;
        shard.set_users(new_users);
        Ok(())
    }
    fn update_action_data(&mut self, key: &H256, data: Bytes) -> StateResult<()> {
        let mut action_data = self.get_action_data_mut(key)?;
        *action_data = data.into();
        Ok(())
    }
}

#[cfg(test)]
mod tests_state {
    use std::sync::Arc;

    use journaldb::{self, Algorithm};

    use super::super::super::tests::helpers::{get_memory_db, get_temp_state, get_temp_state_db};
    use super::*;

    #[test]
    fn work_when_cloned() {
        let a = Address::default();

        let mut state = {
            let mut state = get_temp_state();
            assert_eq!(Ok(false), state.account_exists(&a));
            assert_eq!(Ok(()), state.inc_seq(&a));
            assert_eq!(Ok(1), state.seq(&a));
            let root = state.commit();
            assert!(root.is_ok(), "{:?}", root);
            state.clone()
        };
        assert_eq!(Ok(1), state.seq(&a));
        assert_eq!(Ok(()), state.inc_seq(&a));
        assert_eq!(Ok(2), state.seq(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(2), state.seq(&a));
    }

    #[test]
    fn work_when_cloned_even_not_committed() {
        let a = Address::default();

        let mut state = {
            let mut state = get_temp_state();
            assert_eq!(Ok(false), state.account_exists(&a));
            assert_eq!(Ok(()), state.inc_seq(&a));
            assert_eq!(Ok(1), state.seq(&a));
            state.clone()
        };
        assert_eq!(Ok(1), state.seq(&a));
        assert_eq!(Ok(()), state.inc_seq(&a));
        assert_eq!(Ok(2), state.seq(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(2), state.seq(&a));
    }

    #[test]
    fn state_is_not_synchronized_when_cloned() {
        let a = Address::random();

        let original_state = get_temp_state();

        assert_eq!(Ok(false), original_state.account_exists(&a));

        let mut cloned_state = original_state.clone();

        assert_eq!(Ok(()), cloned_state.inc_seq(&a));
        let root = cloned_state.commit();
        assert!(root.is_ok(), "{:?}", root);

        assert_ne!(original_state.seq(&a), cloned_state.seq(&a));
    }

    #[test]
    fn get_from_database() {
        let memory_db = get_memory_db();
        let jorunal = journaldb::new(Arc::clone(&memory_db), Algorithm::Archive, Some(0));
        let db = StateDB::new(jorunal.boxed_clone(), vec![]);
        let a = Address::default();
        let root = {
            let mut state = TopLevelState::new(StateDB::new(jorunal, vec![]));
            assert_eq!(Ok(()), state.inc_seq(&a));
            assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
            assert_eq!(Ok(69.into()), state.balance(&a));
            let root = state.commit();
            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(69.into()), state.balance(&a));

            let mut transaction = memory_db.transaction();
            let records = state.journal_under(&mut transaction, 1);
            assert!(records.is_ok(), "{:?}", records);
            assert_eq!(1, records.unwrap());
            memory_db.write_buffered(transaction);

            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(69.into()), state.balance(&a));
            root.unwrap()
        };

        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(69.into()), state.balance(&a));
        assert_eq!(Ok(1), state.seq(&a));
    }

    #[test]
    fn get_from_cache() {
        let memory_db = get_memory_db();
        let jorunal = journaldb::new(Arc::clone(&memory_db), Algorithm::Archive, Some(0));
        let mut db = StateDB::new(jorunal.boxed_clone(), vec![]);
        let a = Address::default();
        let root = {
            let mut state = TopLevelState::new(StateDB::new(jorunal, vec![]));
            assert_eq!(Ok(()), state.inc_seq(&a));
            assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
            assert_eq!(Ok(69.into()), state.balance(&a));
            let root = state.commit();
            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(69.into()), state.balance(&a));

            let mut transaction = memory_db.transaction();
            let records = state.journal_under(&mut transaction, 1);
            assert!(records.is_ok(), "{:?}", records);
            assert_eq!(1, records.unwrap());
            memory_db.write_buffered(transaction);

            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(69.into()), state.balance(&a));

            db.override_state(&state);
            root.unwrap()
        };

        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(69.into()), state.balance(&a));
        assert_eq!(Ok(1), state.seq(&a));
    }

    #[test]
    fn remove() {
        let a = Address::default();
        let mut state = get_temp_state();
        assert_eq!(Ok(false), state.account_exists(&a));
        assert_eq!(Ok(false), state.account_exists_and_not_null(&a));
        assert_eq!(Ok(()), state.inc_seq(&a));
        assert_eq!(Ok(true), state.account_exists(&a));
        assert_eq!(Ok(true), state.account_exists_and_not_null(&a));
        assert_eq!(Ok(1), state.seq(&a));
        state.kill_account(&a);
        assert_eq!(Ok(false), state.account_exists(&a));
        assert_eq!(Ok(false), state.account_exists_and_not_null(&a));
        assert_eq!(Ok(0), state.seq(&a));
    }

    #[test]
    fn empty_account_is_not_created() {
        let a = Address::default();
        let mut db = get_temp_state_db();
        let root = {
            let mut state = TopLevelState::new(db.clone(&H256::zero()));
            assert_eq!(Ok(()), state.add_balance(&a, &U256::default())); // create an empty account
            let root = state.commit();
            assert!(root.is_ok(), "{:?}", root);

            assert_eq!(Ok(false), state.account_exists(&a));
            assert_eq!(Ok(false), state.account_exists_and_not_null(&a));

            db.override_state(&state);

            root.unwrap()
        };
        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(false), state.account_exists(&a));
        assert_eq!(Ok(false), state.account_exists_and_not_null(&a));
    }

    #[test]
    fn remove_from_database() {
        let a = Address::default();
        let memory_db = get_memory_db();
        let jorunal = journaldb::new(Arc::clone(&memory_db), Algorithm::Archive, Some(0));
        let mut db = StateDB::new(jorunal.boxed_clone(), vec![]);
        let root = {
            let mut state = TopLevelState::new(StateDB::new(jorunal, vec![]));
            assert_eq!(Ok(()), state.inc_seq(&a));
            let root = state.commit();
            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(true), state.account_exists(&a));
            assert_eq!(Ok(1), state.seq(&a));

            let mut transaction = memory_db.transaction();
            let records = state.journal_under(&mut transaction, 1);
            assert!(records.is_ok(), "{:?}", records);
            assert_eq!(1, records.unwrap());
            memory_db.write_buffered(transaction);

            assert_eq!(Ok(true), state.account_exists(&a));
            assert_eq!(Ok(1), state.seq(&a));

            db.override_state(&state);

            root.unwrap()
        };

        let root = {
            let mut state = TopLevelState::from_existing(db.clone(&root), root).unwrap();
            assert_eq!(Ok(true), state.account_exists(&a));
            assert_eq!(Ok(1), state.seq(&a));
            state.kill_account(&a);
            let root = state.commit();
            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(false), state.account_exists(&a));
            assert_eq!(Ok(0), state.seq(&a));

            let mut transaction = memory_db.transaction();
            let records = state.journal_under(&mut transaction, 1);
            assert!(records.is_ok(), "{:?}", records);
            assert_eq!(0, records.unwrap());
            memory_db.write_buffered(transaction);

            assert_eq!(Ok(false), state.account_exists(&a));
            assert_eq!(Ok(0), state.seq(&a));

            db.override_state(&state);

            root.unwrap()
        };

        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(false), state.account_exists(&a));
        assert_eq!(Ok(0), state.seq(&a));
    }

    #[test]
    fn alter_balance() {
        let mut state = get_temp_state();
        let a = Address::default();
        let b = 1u64.into();
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        assert_eq!(Ok(69.into()), state.balance(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(69.into()), state.balance(&a));
        assert_eq!(Ok(()), state.sub_balance(&a, &U256::from(42u64)));
        assert_eq!(Ok(27.into()), state.balance(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(27.into()), state.balance(&a));
        assert_eq!(Ok(()), state.transfer_balance(&a, &b, &U256::from(18u64)));
        assert_eq!(Ok(9.into()), state.balance(&a));
        assert_eq!(Ok(18.into()), state.balance(&b));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(9.into()), state.balance(&a));
        assert_eq!(Ok(18.into()), state.balance(&b));
    }

    #[test]
    fn alter_seq() {
        let mut state = get_temp_state();
        let a = Address::default();
        assert_eq!(Ok(()), state.inc_seq(&a));
        assert_eq!(Ok(1), state.seq(&a));
        assert_eq!(Ok(()), state.inc_seq(&a));
        assert_eq!(Ok(2), state.seq(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(2), state.seq(&a));
        assert_eq!(Ok(()), state.inc_seq(&a));
        assert_eq!(Ok(3), state.seq(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(3), state.seq(&a));
    }

    #[test]
    fn balance_seq() {
        let mut state = get_temp_state();
        let a = Address::default();
        assert_eq!(Ok(0.into()), state.balance(&a));
        assert_eq!(Ok(0), state.seq(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(0.into()), state.balance(&a));
        assert_eq!(Ok(0), state.seq(&a));
    }

    #[test]
    fn ensure_cached() {
        let mut state = get_temp_state();
        let a = Address::default();
        state.get_account_mut(&a).unwrap();
        assert_eq!(Ok(H256::from("db4046bb91a12a37cbfb0f09631aad96a97248423163eca791e19b430cc7fe4a")), state.commit());
    }

    #[test]
    fn checkpoint_basic() {
        let mut state = get_temp_state();
        let a = Address::default();
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
        let a = Address::default();
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
        let a = Address::default();
        state.create_checkpoint(0);
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        state.create_checkpoint(1);
        assert_eq!(Ok(()), state.add_balance(&a, &U256::from(69u64)));
        assert_eq!(Ok(()), state.inc_seq(&a));
        assert_eq!(Ok((69 + 69).into()), state.balance(&a));
        assert_eq!(Ok(1), state.seq(&a));
        state.discard_checkpoint(1);
        assert_eq!(Ok((69 + 69).into()), state.balance(&a));
        assert_eq!(Ok(1), state.seq(&a));
        state.revert_to_checkpoint(0);
        assert_eq!(Ok(0.into()), state.balance(&a));
        assert_eq!(Ok(0), state.seq(&a));
    }

    #[test]
    fn create_empty() {
        let mut state = get_temp_state();
        assert_eq!(Ok(BLAKE_NULL_RLP), state.commit());
    }
}

#[cfg(test)]
mod tests_parcel {
    use ckey::{Generator, Random};
    use ctypes::transaction::{
        AssetMintOutput, AssetOutPoint, AssetTransferInput, AssetTransferOutput, Error as TransactionError,
    };
    use primitives::H160;

    use super::super::super::tests::helpers::{get_temp_state, get_test_client};
    use super::super::super::{AssetScheme, AssetSchemeAddress, OwnedAsset, OwnedAssetAddress};
    use super::*;

    fn address() -> (Address, Public) {
        let keypair = Random.generate().unwrap();
        (keypair.address(), keypair.public().clone())
    }

    #[test]
    fn apply_error_for_invalid_seq() {
        let mut state = get_temp_state();

        let parcel = Parcel {
            seq: 2,
            fee: 5.into(),
            network_id: "tc".into(),
            action: Action::Payment {
                receiver: address().0,
                amount: 10.into(),
            },
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));

        let result = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(
            Err(StateError::Parcel(ParcelError::InvalidSeq(Mismatch {
                expected: 0,
                found: 2
            }))),
            result
        );

        assert_eq!(Ok(20.into()), state.balance(&sender));
        assert_eq!(Ok(0), state.seq(&sender));
    }

    #[test]
    fn apply_error_for_not_enough_cash() {
        let mut state = get_temp_state();
        let parcel = Parcel {
            fee: 5.into(),
            seq: 0,
            network_id: "tc".into(),
            action: Action::Payment {
                receiver: address().0,
                amount: 10.into(),
            },
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &4.into()));

        let result = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(
            Err(StateError::Parcel(ParcelError::InsufficientBalance {
                address: sender,
                balance: 4.into(),
                cost: 5.into(),
            })),
            result
        );
        assert_eq!(Ok(4.into()), state.balance(&sender));
        assert_eq!(Ok(0), state.seq(&sender));
    }

    #[test]
    fn apply_payment() {
        let mut state = get_temp_state();
        let receiver = 1u64.into();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::Payment {
                receiver,
                amount: 10.into(),
            },
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(Ok(10.into()), state.balance(&receiver));
        assert_eq!(Ok(5.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
    }

    #[test]
    fn apply_set_regular_key() {
        let mut state = get_temp_state();
        let key = 1u64.into();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key,
            },
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &5.into()));

        assert_eq!(state.regular_key(&sender), Ok(None));
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));
        assert_eq!(Ok(Some(key)), state.regular_key(&sender));
    }

    #[test]
    fn use_owner_balance_when_signed_with_regular_key() {
        let mut state = get_temp_state();
        let regular_keypair = Random.generate().unwrap();
        let key = regular_keypair.public();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: key.clone(),
            },
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &15.into()));

        assert_eq!(state.regular_key(&sender), Ok(None));
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));
        assert_eq!(Ok(Some(*key)), state.regular_key(&sender));

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            seq: 1,
            network_id: "tc".into(),
        };

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, regular_keypair.public(), &get_test_client()));
        assert_eq!(Ok(4.into()), state.balance(&sender));
        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(0));
    }

    #[test]
    fn fail_when_two_accounts_used_the_same_regular_key() {
        let mut state = get_temp_state();
        let regular_keypair = Random.generate().unwrap();
        let key = regular_keypair.public();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: key.clone(),
            },
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &15.into()));

        assert_eq!(state.regular_key(&sender), Ok(None));
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));
        assert_eq!(Ok(Some(*key)), state.regular_key(&sender));

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: key.clone(),
            },
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender2, sender_public2) = address();
        assert_eq!(Ok(()), state.add_balance(&sender2, &15.into()));

        let result = state.apply(&parcel, &sender_public2, &get_test_client());
        assert_eq!(Ok(Invoice::Failure(ParcelError::RegularKeyAlreadyInUse)), result);
        assert_eq!(Ok(10.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(None), state.regular_key(&sender2));
    }

    #[test]
    fn fail_when_regular_key_is_already_registered_as_owner_key() {
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
            seq: 0,
            network_id: "tc".into(),
        };

        let result = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(Ok(Invoice::Failure(ParcelError::RegularKeyAlreadyInUseAsPlatformAccount)), result);
        assert_eq!(Ok(15.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
    }

    #[test]
    fn change_regular_key() {
        let (sender, sender_public) = address();
        let (_, regular_public) = address();
        let (_, regular_public2) = address();

        let mut state = get_temp_state();

        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        assert_eq!(Ok(()), state.set_regular_key(&sender_public, &regular_public));

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetRegularKey {
                key: regular_public2,
            },
            seq: 0,
            network_id: "tc".into(),
        };

        assert_eq!(Some(regular_public), state.regular_key(&sender).unwrap());
        assert_eq!(Ok(true), state.regular_account_exists_and_not_null(&regular_public));
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &regular_public, &get_test_client()));
        assert_eq!(Ok(false), state.regular_account_exists_and_not_null(&regular_public));
        assert_eq!(Some(regular_public2), state.regular_key(&sender).unwrap());
    }

    #[test]
    fn pass_registrar_check_using_a_regular_key() {
        let (sender, sender_public) = address();
        let (_, regular_public) = address();

        let network_id = "tc".into();
        let shard_id = 0x0;
        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(()), state.add_balance(&sender, &25.into()));
        assert_eq!(Ok(()), state.set_regular_key(&sender_public, &regular_public));

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let registrar = Some(sender);
        let amount = 30.into();
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
        };
        let mint_hash = mint.hash();
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.clone().into();

        let transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 30.into(),
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash,
                parameters: vec![vec![1]],
                asset_type,
                amount: 30.into(),
            }],
        };
        let mint_parcel = Parcel {
            fee: 11.into(),
            action: Action::AssetTransaction(mint),
            seq: 0,
            network_id,
        };
        let transfer_parcel = Parcel {
            fee: 11.into(),
            action: Action::AssetTransaction(transfer),
            seq: 1,
            network_id,
        };

        assert_eq!(Ok(Invoice::Success), state.apply(&mint_parcel, &regular_public, &get_test_client()));
        assert_eq!(Ok(U256::from(25 - 11)), state.balance(&sender));
        assert_eq!(Ok(Invoice::Success), state.apply(&transfer_parcel, &regular_public, &get_test_client()));
        assert_eq!(Ok(U256::from(25 - 11 - 11)), state.balance(&sender));
    }

    #[test]
    fn use_deleted_regular_key_as_owner_key() {
        let (sender, sender_public) = address();
        let (regular_address, regular_public) = address();
        let (_, regular_public2) = address();

        let mut state = get_temp_state();

        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        assert_eq!(Ok(()), state.set_regular_key(&sender_public, &regular_public));
        assert_eq!(Ok(()), state.set_regular_key(&sender_public, &regular_public2));

        assert_eq!(Ok(false), state.regular_account_exists_and_not_null(&regular_public));
        assert_eq!(Ok(()), state.add_balance(&regular_address, &20.into()));

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            seq: 0,
            network_id: "tc".into(),
        };
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &regular_public, &get_test_client()));
        assert_eq!(Ok(14.into()), state.balance(&regular_address));
        assert_eq!(Ok(20.into()), state.balance(&sender));
        assert_eq!(Ok(Some(vec![regular_address])), state.shard_owners(0));
    }

    #[test]
    fn fail_when_someone_sends_some_ccc_to_an_address_which_used_as_a_regular_key() {
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
            seq: 0,
            network_id: "tc".into(),
        };
        let result = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(Ok(Invoice::Failure(ParcelError::InvalidTransferDestination)), result);
        assert_eq!(Ok(15.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
    }

    #[test]
    fn apply_error_for_action_failure() {
        let mut state = get_temp_state();
        let receiver = 1u64.into();

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::Payment {
                receiver,
                amount: 30.into(),
            },
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientBalance {
                address: sender,
                balance: 15.into(),
                cost: 30.into(),
            })),
            state.apply(&parcel, &sender_public, &get_test_client())
        );

        assert_eq!(Ok(0.into()), state.balance(&receiver));
        assert_eq!(Ok(15.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
    }

    #[test]
    fn mint_permissioned_asset() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let network_id = "tc".into();
        let shard_id = 0x0;

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let amount = 30.into();
        let transaction = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: Some(amount),
            },
            registrar,
        };
        let transaction_hash = transaction.hash();
        let parcel = Parcel {
            fee: 11.into(),
            action: Action::AssetTransaction(transaction),
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(state.balance(&sender), Ok(58.into()));
        assert_eq!(Ok(1), state.seq(&sender));

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_scheme_address.into(), lock_script_hash, parameters, amount))), asset);
    }

    #[test]
    fn mint_infinite_permissioned_asset() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let shard_id = 0;
        let network_id = "tc".into();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            registrar,
        };
        let transaction_hash = transaction.hash();
        let parcel = Parcel {
            fee: 5.into(),
            action: Action::AssetTransaction(transaction),
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(state.balance(&sender), Ok(64.into()));
        assert_eq!(Ok(1), state.seq(&sender));

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), U256::max_value(), registrar))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(
            Ok(Some(OwnedAsset::new(asset_scheme_address.into(), lock_script_hash, parameters, U256::max_value()))),
            asset
        );
    }

    #[test]
    fn mint_and_transfer_in_different_parcel() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let network_id = "tc".into();
        let shard_id = 0x00;

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
        let amount = 30.into();
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
        };
        let mint_hash = mint.hash();

        let mint_parcel = Parcel {
            fee: 20.into(),
            network_id,
            seq: 0,
            action: Action::AssetTransaction(mint),
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(120)));

        assert_eq!(Ok(Invoice::Success), state.apply(&mint_parcel, &sender_public, &get_test_client()));
        assert_eq!(state.balance(&sender), Ok(100.into()));
        assert_eq!(Ok(1), state.seq(&sender));

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.clone().into();
        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);

        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], 30.into()))), asset);

        let random_lock_script_hash = H160::random();
        let transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 30.into(),
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![vec![1]],
                    asset_type,
                    amount: 10.into(),
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type,
                    amount: 5.into(),
                },
                AssetTransferOutput {
                    lock_script_hash: random_lock_script_hash,
                    parameters: vec![],
                    asset_type,
                    amount: 15.into(),
                },
            ],
        };
        let transfer_hash = transfer.hash();

        state.shard_root(shard_id).unwrap().unwrap();

        let transfer_parcel = Parcel {
            fee: 30.into(),
            network_id,
            seq: 1,
            action: Action::AssetTransaction(transfer),
        };

        assert_eq!(Ok(Invoice::Success), state.apply(&transfer_parcel, &sender_public, &get_test_client()));

        assert_eq!(state.balance(&sender), Ok(70.into()));
        assert_eq!(Ok(2), state.seq(&sender));

        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(None), asset);

        let asset0_address = OwnedAssetAddress::new(transfer_hash, 0, shard_id);
        let asset0 = state.asset(shard_id, &asset0_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![vec![1]], 10.into()))), asset0);

        let asset1_address = OwnedAssetAddress::new(transfer_hash, 1, shard_id);
        let asset1 = state.asset(shard_id, &asset1_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], 5.into()))), asset1);

        let asset2_address = OwnedAssetAddress::new(transfer_hash, 2, shard_id);
        let asset2 = state.asset(shard_id, &asset2_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, random_lock_script_hash, vec![], 15.into()))), asset2);
    }

    #[test]
    fn cannot_mint_twice_in_different_parcel() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let network_id = "tc".into();
        let shard_id = 0x0;

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let amount = 30.into();
        let transaction = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: Some(amount),
            },
            registrar,
        };
        let parcel = Parcel {
            fee: 11.into(),
            action: Action::AssetTransaction(transaction.clone()),
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        state.shard_root(shard_id).expect("Shard must exist").expect("Shard root must exist");

        let transaction_hash = transaction.hash();
        let parcel = Parcel {
            fee: 11.into(),
            action: Action::AssetTransaction(transaction),
            seq: 1,
            network_id,
        };
        assert_eq!(
            Ok(Invoice::Failure(TransactionError::AssetSchemeDuplicated(transaction_hash).into())),
            state.apply(&parcel, &sender_public, &get_test_client())
        );
    }

    #[test]
    fn wrap_and_unwrap_ccc() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(100u64)));

        let network_id = "tc".into();
        let shard_id = 0x0;

        let lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let amount = 30.into();

        let parcel = Parcel {
            fee: 11.into(),
            action: Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters: vec![],
                amount,
            },
            seq: 0,
            network_id,
        };
        let parcel_hash = parcel.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(state.balance(&sender), Ok(59.into()));
        assert_eq!(Ok(1), state.seq(&sender));

        let asset_scheme_address = AssetSchemeAddress::new_with_zero_suffix(shard_id);
        let asset_type = asset_scheme_address.into();
        let asset_address = OwnedAssetAddress::new(parcel_hash, 0, shard_id);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let unwrap_ccc_tx = Transaction::AssetUnwrapCCC {
            network_id,
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: parcel_hash,
                    index: 0,
                    asset_type,
                    amount: 30.into(),
                },
                timelock: None,
                lock_script: vec![0x01],
                unlock_script: vec![],
            },
        };
        let parcel = Parcel {
            fee: 11.into(),
            action: Action::AssetTransaction(unwrap_ccc_tx),
            seq: 1,
            network_id,
        };

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(state.balance(&sender), Ok(78.into()));
        assert_eq!(Ok(2), state.seq(&sender));

        let asset_address = OwnedAssetAddress::new(parcel_hash, 0, shard_id);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(None), asset);
    }

    #[test]
    fn wrap_ccc_and_transfer_and_unwrap_ccc() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(100u64)));

        let network_id = "tc".into();
        let shard_id = 0x0;

        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30.into();

        let parcel = Parcel {
            fee: 11.into(),
            action: Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters: vec![],
                amount,
            },
            seq: 0,
            network_id,
        };
        let parcel_hash = parcel.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(state.balance(&sender), Ok(59.into()));
        assert_eq!(Ok(1), state.seq(&sender));

        let asset_scheme_address = AssetSchemeAddress::new_with_zero_suffix(shard_id);
        let asset_type = asset_scheme_address.into();
        let asset_address = OwnedAssetAddress::new(parcel_hash, 0, shard_id);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let lock_script_hash_burn = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let random_lock_script_hash = H160::random();
        let transfer_tx = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: parcel_hash,
                    index: 0,
                    asset_type,
                    amount: 30.into(),
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![vec![1]],
                    asset_type,
                    amount: 10.into(),
                },
                AssetTransferOutput {
                    lock_script_hash: lock_script_hash_burn,
                    parameters: vec![],
                    asset_type,
                    amount: 5.into(),
                },
                AssetTransferOutput {
                    lock_script_hash: random_lock_script_hash,
                    parameters: vec![],
                    asset_type,
                    amount: 15.into(),
                },
            ],
        };
        let transfer_tx_hash = transfer_tx.hash();

        let parcel = Parcel {
            fee: 11.into(),
            action: Action::AssetTransaction(transfer_tx),
            seq: 1,
            network_id,
        };

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(state.balance(&sender), Ok(48.into()));
        assert_eq!(Ok(2), state.seq(&sender));

        let asset_address = OwnedAssetAddress::new(parcel_hash, 0, shard_id);
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(None), asset);

        let asset0_address = OwnedAssetAddress::new(transfer_tx_hash, 0, shard_id);
        let asset0 = state.asset(shard_id, &asset0_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![vec![1]], 10.into()))), asset0);

        let asset1_address = OwnedAssetAddress::new(transfer_tx_hash, 1, shard_id);
        let asset1 = state.asset(shard_id, &asset1_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash_burn, vec![], 5.into()))), asset1);

        let asset2_address = OwnedAssetAddress::new(transfer_tx_hash, 2, shard_id);
        let asset2 = state.asset(shard_id, &asset2_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, random_lock_script_hash, vec![], 15.into()))), asset2);

        let unwrap_ccc_tx = Transaction::AssetUnwrapCCC {
            network_id,
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: transfer_tx_hash,
                    index: 1,
                    asset_type,
                    amount: 5.into(),
                },
                timelock: None,
                lock_script: vec![0x01],
                unlock_script: vec![],
            },
        };
        let parcel = Parcel {
            fee: 11.into(),
            action: Action::AssetTransaction(unwrap_ccc_tx),
            seq: 2,
            network_id,
        };

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(state.balance(&sender), Ok(42.into()));
        assert_eq!(Ok(3), state.seq(&sender));

        let asset1_address = OwnedAssetAddress::new(transfer_tx_hash, 1, shard_id);
        let asset1 = state.asset(shard_id, &asset1_address);
        assert_eq!(Ok(None), asset1);
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
        assert_eq!(Ok(None), state.asset(shard_id, &OwnedAssetAddress::new(H256::random(), 0, shard_id)));
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
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        let res = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(Ok(Invoice::Success), res);
        assert_eq!(Ok(14.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_ne!(Ok(None), state.shard_root(0));
        assert_ne!(Ok(None), state.shard_root(0));
        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(0));
    }

    #[test]
    fn get_asset_in_invalid_shard2() {
        let mut state = get_temp_state();

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        let res = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(Ok(Invoice::Success), res);
        assert_eq!(Ok(14.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(0));

        let shard_id = 3;
        assert_eq!(Ok(None), state.asset(shard_id, &OwnedAssetAddress::new(H256::random(), 0, shard_id)));
    }

    #[test]
    fn get_asset_scheme_in_invalid_shard2() {
        let mut state = get_temp_state();

        let parcel = Parcel {
            action: Action::CreateShard,
            fee: 5.into(),
            seq: 0,
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &20.into()));
        let res = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(Ok(Invoice::Success), res);
        assert_eq!(Ok(14.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(0));

        let shard_id = 3;
        assert_eq!(Ok(None), state.asset_scheme(shard_id, &AssetSchemeAddress::new(H256::random(), shard_id)));
    }

    #[test]
    fn mint_asset_on_invalid_parcel_must_fail() {
        let mut state = get_temp_state();

        let shard_id = 0;
        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let amount = 30.into();
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: Some(amount),
            },
            registrar,
        };
        let parcel = Parcel {
            fee: 11.into(),
            seq: 0,
            action: Action::AssetTransaction(transaction),
            network_id: "tc".into(),
        };
        let (sender, sender_public) = address();

        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));

        let res = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(Ok(Invoice::Failure(ParcelError::InvalidShardId(0))), res);
        assert_eq!(Ok(58.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
    }

    #[test]
    fn transfer_on_invalid_parcel_must_fail() {
        let mut state = get_temp_state();

        let network_id = "tc".into();
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
                    amount: 30.into(),
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash: H160::random(),
                    parameters: vec![vec![1]],
                    asset_type,
                    amount: 10.into(),
                },
                AssetTransferOutput {
                    lock_script_hash: H160::random(),
                    parameters: vec![],
                    asset_type,
                    amount: 5.into(),
                },
                AssetTransferOutput {
                    lock_script_hash: H160::random(),
                    parameters: vec![],
                    asset_type,
                    amount: 15.into(),
                },
            ],
        };

        let parcel = Parcel {
            fee: 30.into(),
            network_id,
            seq: 0,
            action: Action::AssetTransaction(transfer),
        };

        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(120)));

        let res = state.apply(&parcel, &sender_public, &get_test_client());
        assert_eq!(Ok(Invoice::Failure(ParcelError::InvalidShardId(100))), res);
        assert_eq!(Ok(90.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
    }

    #[test]
    fn set_shard_owners() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let network_id = "tc".into();
        let shard_id = 0;
        let owners = vec![Address::random(), Address::random(), sender];

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetShardOwners {
                shard_id,
                owners: owners.clone(),
            },
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(shard_id));

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(Ok(64.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(owners)), state.shard_owners(shard_id));
    }

    #[test]
    fn new_owners_must_contain_sender() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let network_id = "tc".into();
        let shard_id = 0;
        let owners = {
            let a1 = loop {
                let a = Address::random();
                if a != sender {
                    break a
                }
            };
            let a2 = loop {
                let a = Address::random();
                if a != sender {
                    break a
                }
            };
            vec![a1, a2]
        };

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetShardOwners {
                shard_id,
                owners,
            },
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(shard_id));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::NewOwnersMustContainSender)),
            state.apply(&parcel, &sender_public, &get_test_client())
        );

        assert_eq!(Ok(64.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(shard_id));
    }

    #[test]
    fn only_owner_can_set_owners() {
        let (original_owner, _) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![original_owner], vec![]));
        let (sender, sender_public) = address();
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let network_id = "tc".into();
        let shard_id = 0;

        let owners = {
            let a1 = loop {
                let a = Address::random();
                if a != original_owner {
                    break a
                }
            };
            let a2 = loop {
                let a = Address::random();
                if a != original_owner {
                    break a
                }
            };
            vec![a1, a2, sender]
        };

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetShardOwners {
                shard_id,
                owners,
            },
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(Some(vec![original_owner])), state.shard_owners(shard_id));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientPermission)),
            state.apply(&parcel, &sender_public, &get_test_client())
        );

        assert_eq!(Ok(64.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(vec![original_owner])), state.shard_owners(shard_id));
    }

    #[test]
    fn set_shard_owners_fail_on_invalid_shard_id() {
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], vec![]));
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let network_id = "tc".into();
        let real_shard_id = 0;
        let shard_id = 0xF;

        let owners = vec![Address::random(), Address::random(), sender];

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetShardOwners {
                shard_id,
                owners: owners.clone(),
            },
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(real_shard_id));
        assert_eq!(Ok(None), state.shard_owners(shard_id));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InvalidShardId(shard_id))),
            state.apply(&parcel, &sender_public, &get_test_client())
        );

        assert_eq!(Ok(64.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(real_shard_id));
        assert_eq!(Ok(None), state.shard_owners(shard_id));
    }

    #[test]
    fn user_cannot_set_owners() {
        let (original_owner, _) = address();
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![original_owner], vec![sender]));
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let network_id = "CA".into();
        let shard_id = 0;

        let owners = {
            let a1 = loop {
                let a = Address::random();
                if a != original_owner {
                    break a
                }
            };
            let a2 = loop {
                let a = Address::random();
                if a != original_owner {
                    break a
                }
            };
            vec![a1, a2, sender]
        };

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetShardOwners {
                shard_id,
                owners,
            },
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(Some(vec![original_owner])), state.shard_owners(shard_id));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientPermission)),
            state.apply(&parcel, &sender_public, &get_test_client())
        );

        assert_eq!(Ok(64.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(vec![original_owner])), state.shard_owners(shard_id));
    }


    #[test]
    fn user_can_mint() {
        let (original_owner, _) = address();
        let (sender, sender_public) = address();

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![original_owner], vec![sender]));
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        let shard_id = 0x00;
        let network_id = "ne".into();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
        let amount = 30.into();
        let parameters = vec![];

        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: Some(amount),
            },
            registrar,
        };
        let mint_hash = mint.hash();

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);

        let parcel = Parcel {
            fee: 20.into(),
            seq: 0,
            network_id,
            action: Action::AssetTransaction(mint),
        };

        assert_eq!(Invoice::Success, state.apply(&parcel, &sender_public, &get_test_client()).unwrap());

        assert_eq!(Ok(0x31.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));

        let asset_scheme = state.asset_scheme(shard_id, &asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset_type = asset_scheme_address.into();
        let asset = state.asset(shard_id, &asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, parameters, amount))), asset);
    }

    #[test]
    fn set_shard_users() {
        let network_id = "a2".into();
        let shard_id = 0;

        let (sender, sender_public) = address();
        let old_users = vec![Address::random(), Address::random(), Address::random()];

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(vec![sender], old_users.clone()));
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(shard_id));
        assert_eq!(Ok(Some(old_users.clone())), state.shard_users(shard_id));

        let new_users = vec![Address::random(), Address::random(), sender];

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetShardUsers {
                shard_id,
                users: new_users.clone(),
            },
            seq: 0,
            network_id,
        };

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &sender_public, &get_test_client()));

        assert_eq!(Ok(64.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(vec![sender])), state.shard_owners(shard_id));
        assert_eq!(Ok(Some(new_users)), state.shard_users(shard_id));
    }


    #[test]
    fn user_cannot_set_shard_users() {
        let network_id = "a2".into();
        let shard_id = 0;

        let (sender, sender_public) = address();
        let owners = vec![Address::random(), Address::random(), Address::random()];
        let old_users = vec![Address::random(), Address::random(), Address::random(), sender];

        let mut state = get_temp_state();
        assert_eq!(Ok(()), state.create_shard_level_state(owners.clone(), old_users.clone()));
        assert_eq!(Ok(()), state.add_balance(&sender, &U256::from(69u64)));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);

        assert_eq!(Ok(Some(owners.clone())), state.shard_owners(shard_id));
        assert_eq!(Ok(Some(old_users.clone())), state.shard_users(shard_id));

        let new_users = vec![Address::random(), Address::random(), sender];

        let parcel = Parcel {
            fee: 5.into(),
            action: Action::SetShardUsers {
                shard_id,
                users: new_users.clone(),
            },
            seq: 0,
            network_id,
        };

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientPermission)),
            state.apply(&parcel, &sender_public, &get_test_client())
        );

        assert_eq!(Ok(64.into()), state.balance(&sender));
        assert_eq!(Ok(1), state.seq(&sender));
        assert_eq!(Ok(Some(owners)), state.shard_owners(shard_id));
        assert_eq!(Ok(Some(old_users)), state.shard_users(shard_id));
    }
}
