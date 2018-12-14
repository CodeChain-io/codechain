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
use ckey::{public_to_address, recover, verify_address, Address, NetworkId, Public, Signature};
use cmerkle::{Result as TrieResult, TrieError, TrieFactory};
use ctypes::invoice::Invoice;
use ctypes::parcel::{Action, Error as ParcelError, Parcel};
use ctypes::transaction::{AssetWrapCCCOutput, InnerTransaction, Transaction};
use ctypes::util::unexpected::Mismatch;
use ctypes::ShardId;
use cvm::ChainTimeInfo;
use hashdb::AsHashDB;
use kvdb::DBTransaction;
use primitives::{Bytes, H160, H256};
use util_error::UtilError;

use crate::cache::{ShardCache, TopCache};
use crate::checkpoint::{CheckpointId, StateWithCheckpoint};
use crate::traits::{ShardState, ShardStateView, StateWithCache, TopState, TopStateView};
use crate::{
    Account, ActionData, FindActionHandler, Metadata, MetadataAddress, RegularAccount, RegularAccountAddress, Shard,
    ShardAddress, ShardLevelState, StateDB, StateError, StateResult, Text,
};
#[cfg(test)]
use crate::{Asset, AssetSchemeAddress, OwnedAssetAddress};

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
        self.top_cache.account(&a, &trie)
    }

    fn regular_account_by_address(&self, a: &Address) -> TrieResult<Option<RegularAccount>> {
        let a = RegularAccountAddress::from_address(a);
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        Ok(self.top_cache.regular_account(&a, &trie)?)
    }

    fn metadata(&self) -> TrieResult<Option<Metadata>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        let address = MetadataAddress::new();
        self.top_cache.metadata(&address, &trie)
    }

    fn shard(&self, shard_id: ShardId) -> TrieResult<Option<Shard>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        let shard_address = ShardAddress::new(shard_id);
        self.top_cache.shard(&shard_address, &trie)
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

    fn text(&self, key: &H256) -> TrieResult<Option<Text>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        Ok(self.top_cache.text(key, &trie)?.map(Into::into))
    }

    fn action_data(&self, key: &H256) -> TrieResult<Option<ActionData>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        Ok(self.top_cache.action_data(key, &trie)?.map(Into::into))
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

                shard_cache.commit(&mut *trie)?;
            }
            self.set_shard_root(shard_id, shard_root)?;
        }
        {
            let mut db = self.db.borrow_mut();
            let mut trie = TrieFactory::from_existing(db.as_hashdb_mut(), &mut self.root)?;
            self.top_cache.commit(&mut *trie)?;
        }
        Ok(self.root)
    }

    fn commit_and_into_db(mut self) -> StateResult<(StateDB, H256)> {
        let root = self.commit()?;
        Ok((self.db.into_inner(), root))
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
    pub fn new_for_testing(mut db: StateDB) -> Self {
        let mut root = H256::new();
        // init trie and reset root too null
        let _ = TrieFactory::create(db.as_hashdb_mut(), &mut root);

        Self::from_existing(db, root).expect("The empty trie root was initialized")
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
    pub fn apply<C: ChainTimeInfo + FindActionHandler>(
        &mut self,
        parcel: &Parcel,
        signed_hash: &H256,
        signer_public: &Public,
        client: &C,
    ) -> StateResult<Invoice> {
        let fee_payer = self.public_to_owner_address(signer_public)?;

        self.create_checkpoint(PARCEL_FEE_CHECKPOINT);

        match self.apply_internal(parcel, &fee_payer, signed_hash, signer_public, client) {
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

    // Change the public to an owner address if it is a regular key.
    fn public_to_owner_address(&self, public: &Public) -> StateResult<Address> {
        Ok(if self.regular_account_exists_and_not_null(public)? {
            let regular_account = self.get_regular_account_mut(public)?;
            public_to_address(&regular_account.owner_public())
        } else {
            public_to_address(public)
        })
    }


    fn apply_internal<C: ChainTimeInfo + FindActionHandler>(
        &mut self,
        parcel: &Parcel,
        fee_payer: &Address,
        signed_hash: &H256,
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

        self.inc_seq(fee_payer)?;
        self.sub_balance(fee_payer, fee)?;

        // The failed parcel also must pay the fee and increase seq.
        self.create_checkpoint(PARCEL_ACTION_CHECKPOINT);

        match self.apply_action(
            &parcel.action,
            parcel.network_id,
            &parcel.hash(),
            signed_hash,
            fee_payer,
            signer_public,
            client,
        ) {
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

    fn apply_action<C: ChainTimeInfo + FindActionHandler>(
        &mut self,
        action: &Action,
        network_id: NetworkId,
        parcel_hash: &H256,
        signed_hash: &H256,
        fee_payer: &Address,
        signer_public: &Public,
        client: &C,
    ) -> StateResult<Invoice> {
        match action {
            Action::AssetTransaction {
                transaction,
                approvals,
            } => {
                debug_assert_eq!(network_id, transaction.network_id());

                let transaction_hash = transaction.hash();
                let approvers = approvals
                    .iter()
                    .map(|signature| {
                        let public = recover(&signature, &transaction_hash)?;
                        self.public_to_owner_address(&public)
                    })
                    .collect::<StateResult<Vec<_>>>()?;
                Ok(self.apply_transaction(transaction, fee_payer, &approvers, client)?)
            }
            Action::Payment {
                receiver,
                amount,
            } => {
                self.transfer_balance(fee_payer, receiver, *amount)?;
                Ok(Invoice::Success)
            }
            Action::SetRegularKey {
                key,
            } => {
                self.set_regular_key(signer_public, key)?;
                Ok(Invoice::Success)
            }
            Action::CreateShard => {
                // FIXME: Make shard creation cost configurable
                #[cfg(test)]
                let shard_creation_cost = 1;
                #[cfg(not(test))]
                let shard_creation_cost = ::std::u64::MAX;

                self.create_shard(shard_creation_cost, fee_payer)?;
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
                network_id,
                *shard_id,
                *parcel_hash,
                *lock_script_hash,
                parameters.clone(),
                *amount,
                fee_payer,
                client,
            )?),
            Action::Store {
                content,
                certifier,
                signature,
            } => {
                let text = Text::new(content, certifier);
                self.store_text(signed_hash, text, signature)?;
                Ok(Invoice::Success)
            }
            Action::Remove {
                hash,
                signature,
            } => {
                self.remove_text(hash, signature)?;
                Ok(Invoice::Success)
            }
            Action::Custom {
                handler_id,
                bytes,
            } => {
                let handler = client.find_action_handler_for(*handler_id).expect("Unknown custom parsel applied!");
                let invoice = handler.execute(bytes, self, fee_payer).expect("Custom action handler execution failed");
                Ok(invoice)
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
        amount: u64,
        sender: &Address,
        client: &C,
    ) -> StateResult<Invoice> {
        let shard_root = self.shard_root(shard_id)?.ok_or_else(|| ParcelError::InvalidShardId(shard_id))?;
        let shard_users = self.shard_users(shard_id)?.expect("Shard must exist");

        self.sub_balance(sender, amount)?;

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

        let shard_cache = self.shard_caches.entry(shard_id).or_default();
        let mut shard_level_state = ShardLevelState::from_existing(shard_id, &mut self.db, shard_root, shard_cache)?;
        Ok(shard_level_state.apply(&transaction, sender, &shard_users, &[], client)?)
    }

    pub fn apply_transaction<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        sender: &Address,
        approvers: &[Address],
        client: &C,
    ) -> StateResult<Invoice> {
        let shard_ids = transaction.related_shards();

        let first_invoice = self.apply_transaction_for_shard(transaction, shard_ids[0], sender, approvers, client)?;

        for shard_id in shard_ids.iter().skip(1) {
            let invoice = self.apply_transaction_for_shard(transaction, *shard_id, sender, approvers, client)?;
            if invoice != first_invoice {
                return Err(ParcelError::InconsistentShardOutcomes.into())
            }
        }

        if first_invoice == Invoice::Success {
            let unwrapped_amount = transaction.unwrapped_amount();
            self.add_balance(sender, unwrapped_amount)?;
        }
        Ok(first_invoice)
    }

    fn apply_transaction_for_shard<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        shard_id: ShardId,
        sender: &Address,
        approvers: &[Address],
        client: &C,
    ) -> StateResult<Invoice> {
        let shard_root = self.shard_root(shard_id)?.ok_or_else(|| ParcelError::InvalidShardId(shard_id))?;
        let shard_users = self.shard_users(shard_id)?.expect("Shard must exist");

        let shard_cache = self.shard_caches.entry(shard_id).or_default();
        let mut shard_level_state = ShardLevelState::from_existing(shard_id, &mut self.db, shard_root, shard_cache)?;
        shard_level_state.apply(&transaction.clone().into(), sender, &shard_users, approvers, client)
    }

    fn create_shard_level_state(
        &mut self,
        shard_id: ShardId,
        owners: Vec<Address>,
        users: Vec<Address>,
    ) -> StateResult<()> {
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
        self.top_cache.account_mut(&a, &trie)
    }

    fn get_regular_account_mut(&self, public: &Public) -> TrieResult<RefMut<RegularAccount>> {
        let regular_account_address = RegularAccountAddress::new(public);
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.top_cache.regular_account_mut(&regular_account_address, &trie)
    }

    fn get_metadata_mut(&self) -> TrieResult<RefMut<Metadata>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        let address = MetadataAddress::new();
        self.top_cache.metadata_mut(&address, &trie)
    }

    fn get_shard_mut(&self, shard_id: ShardId) -> TrieResult<RefMut<Shard>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        let shard_address = ShardAddress::new(shard_id);
        self.top_cache.shard_mut(&shard_address, &trie)
    }

    fn get_text(&self, key: &H256) -> TrieResult<Option<Text>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.top_cache.text(key, &trie)
    }

    fn get_text_mut(&self, key: &H256) -> TrieResult<RefMut<Text>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.top_cache.text_mut(key, &trie)
    }

    fn get_action_data_mut(&self, key: &H256) -> TrieResult<RefMut<ActionData>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.top_cache.action_data_mut(key, &trie)
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

    #[cfg(test)]
    fn set_balance(&mut self, a: &Address, balance: u64) -> TrieResult<()> {
        self.get_account_mut(a)?.set_balance(balance);
        Ok(())
    }

    #[cfg(test)]
    fn set_number_of_shards(&mut self, number_of_shards: ShardId) -> TrieResult<()> {
        self.get_metadata_mut()?.set_number_of_shards(number_of_shards);
        Ok(())
    }

    #[cfg(test)]
    fn create_asset_scheme(
        &mut self,
        shard_id: ShardId,
        a: &AssetSchemeAddress,
        metadata: String,
        amount: u64,
        approver: Option<Address>,
        administrator: Option<Address>,
        pool: Vec<Asset>,
    ) -> TrieResult<bool> {
        match self.shard_root(shard_id)? {
            Some(shard_root) => {
                let mut shard_cache = self.shard_caches.entry(shard_id).or_default();
                let state = ShardLevelState::from_existing(shard_id, &mut self.db, shard_root, &mut shard_cache)?;
                state.create_asset_scheme(a, metadata, amount, approver, administrator, pool)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    #[cfg(test)]
    fn create_asset(
        &mut self,
        shard_id: ShardId,
        tx_hash: H256,
        index: usize,
        asset_type: H256,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        amount: u64,
        order_hash: Option<H256>,
    ) -> TrieResult<bool> {
        match self.shard_root(shard_id)? {
            Some(shard_root) => {
                let mut shard_cache = self.shard_caches.entry(shard_id).or_default();
                let state = ShardLevelState::from_existing(shard_id, &mut self.db, shard_root, &mut shard_cache)?;
                let a = OwnedAssetAddress::new(tx_hash, index, shard_id);
                state.create_asset(&a, asset_type, lock_script_hash, parameters, amount, order_hash)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

// TODO: cloning for `State` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for TopLevelState {
    fn clone(&self) -> TopLevelState {
        TopLevelState {
            db: RefCell::new(self.db.borrow().clone(&self.root)),
            root: self.root,
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

    fn add_balance(&mut self, a: &Address, incr: u64) -> TrieResult<()> {
        ctrace!(STATE, "add_balance({}, {}): {}", a, incr, self.balance(a)?);
        if incr != 0 {
            self.get_account_mut(a)?.add_balance(incr);
        }
        Ok(())
    }

    fn sub_balance(&mut self, a: &Address, decr: u64) -> StateResult<()> {
        ctrace!(STATE, "sub_balance({}, {}): {}", a, decr, self.balance(a)?);
        if decr == 0 {
            return Ok(())
        }
        let balance = self.balance(a)?;
        if balance < decr {
            return Err(ParcelError::InsufficientBalance {
                address: *a,
                cost: decr,
                balance,
            }
            .into())
        }
        self.get_account_mut(a)?.sub_balance(decr);
        Ok(())
    }

    fn transfer_balance(&mut self, from: &Address, to: &Address, by: u64) -> StateResult<()> {
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
            let owner_public = regular_account.owner_public();
            let owner_address = public_to_address(owner_public);
            (*owner_public, owner_address)
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

    fn create_shard(&mut self, shard_creation_cost: u64, fee_payer: &Address) -> StateResult<()> {
        self.sub_balance(fee_payer, shard_creation_cost)?;

        let shard_id = {
            let mut metadata = self.get_metadata_mut()?;
            metadata.increase_number_of_shards()
        };
        self.create_shard_level_state(shard_id, vec![*fee_payer], vec![])?;

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

    fn store_text(&mut self, key: &H256, text: Text, sig: &Signature) -> StateResult<()> {
        match verify_address(text.certifier(), sig, &text.content_hash()) {
            Ok(false) => {
                return Err(ParcelError::TextVerificationFail("Certifier and signer are different".to_string()).into())
            }
            Err(err) => return Err(ParcelError::TextVerificationFail(err.to_string()).into()),
            _ => {}
        }
        let mut text_entry = self.get_text_mut(key)?;
        *text_entry = text;
        Ok(())
    }

    fn remove_text(&mut self, key: &H256, sig: &Signature) -> StateResult<()> {
        let text = self.get_text(key)?.ok_or_else(|| ParcelError::TextNotExist)?;
        match verify_address(text.certifier(), sig, key) {
            Ok(false) => {
                return Err(ParcelError::TextVerificationFail("Certifier and signer are different".to_string()).into())
            }
            Err(err) => return Err(ParcelError::TextVerificationFail(err.to_string()).into()),
            _ => {}
        }
        self.top_cache.remove_text(key);
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

    use super::*;
    use crate::tests::helpers::{get_memory_db, get_temp_state, get_temp_state_db};

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
        let db = StateDB::new(jorunal.boxed_clone());
        let a = Address::default();
        let root = {
            let mut state = TopLevelState::new_for_testing(StateDB::new(jorunal));
            assert_eq!(Ok(()), state.inc_seq(&a));
            assert_eq!(Ok(()), state.add_balance(&a, 100));
            assert_eq!(Ok(100), state.balance(&a));
            let root = state.commit();
            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(100), state.balance(&a));

            let mut transaction = memory_db.transaction();
            let records = state.journal_under(&mut transaction, 1);
            assert!(records.is_ok(), "{:?}", records);
            assert_eq!(1, records.unwrap());
            memory_db.write_buffered(transaction);

            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(100), state.balance(&a));
            root.unwrap()
        };

        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(100), state.balance(&a));
        assert_eq!(Ok(1), state.seq(&a));
    }

    #[test]
    fn get_from_cache() {
        let memory_db = get_memory_db();
        let jorunal = journaldb::new(Arc::clone(&memory_db), Algorithm::Archive, Some(0));
        let mut db = StateDB::new(jorunal.boxed_clone());
        let a = Address::default();
        let root = {
            let mut state = TopLevelState::new_for_testing(StateDB::new(jorunal));
            assert_eq!(Ok(()), state.inc_seq(&a));
            assert_eq!(Ok(()), state.add_balance(&a, 69));
            assert_eq!(Ok(69), state.balance(&a));
            let root = state.commit();
            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(69), state.balance(&a));

            let mut transaction = memory_db.transaction();
            let records = state.journal_under(&mut transaction, 1);
            assert!(records.is_ok(), "{:?}", records);
            assert_eq!(1, records.unwrap());
            memory_db.write_buffered(transaction);

            assert!(root.is_ok(), "{:?}", root);
            assert_eq!(Ok(69), state.balance(&a));

            db.override_state(&state);
            root.unwrap()
        };

        let state = TopLevelState::from_existing(db, root).unwrap();
        assert_eq!(Ok(69), state.balance(&a));
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
            let mut state = TopLevelState::new_for_testing(db.clone(&H256::zero()));
            assert_eq!(Ok(()), state.add_balance(&a, 0)); // create an empty account
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
        let mut db = StateDB::new(jorunal.boxed_clone());
        let root = {
            let mut state = TopLevelState::new_for_testing(StateDB::new(jorunal));
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
        assert_eq!(Ok(()), state.add_balance(&a, 100));
        assert_eq!(Ok(100), state.balance(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(100), state.balance(&a));
        assert_eq!(Ok(()), state.sub_balance(&a, 42));
        assert_eq!(Ok(100 - 42), state.balance(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(100 - 42), state.balance(&a));
        assert_eq!(Ok(()), state.transfer_balance(&a, &b, 18));
        assert_eq!(Ok(100 - 42 - 18), state.balance(&a));
        assert_eq!(Ok(18), state.balance(&b));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(100 - 42 - 18), state.balance(&a));
        assert_eq!(Ok(18), state.balance(&b));
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
        assert_eq!(Ok(0), state.balance(&a));
        assert_eq!(Ok(0), state.seq(&a));
        let root = state.commit();
        assert!(root.is_ok(), "{:?}", root);
        assert_eq!(Ok(0), state.balance(&a));
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
        assert_eq!(Ok(()), state.add_balance(&a, 100));
        assert_eq!(Ok(100), state.balance(&a));
        state.discard_checkpoint(0);
        assert_eq!(Ok(100), state.balance(&a));
        state.create_checkpoint(1);
        assert_eq!(Ok(()), state.add_balance(&a, 1));
        assert_eq!(Ok(100 + 1), state.balance(&a));
        state.revert_to_checkpoint(1);
        assert_eq!(Ok(100), state.balance(&a));
    }

    #[test]
    fn checkpoint_nested() {
        let mut state = get_temp_state();
        let a = Address::default();
        state.create_checkpoint(0);
        assert_eq!(Ok(()), state.add_balance(&a, 100));
        state.create_checkpoint(1);
        assert_eq!(Ok(()), state.add_balance(&a, 120));
        assert_eq!(Ok(100 + 120), state.balance(&a));
        state.revert_to_checkpoint(1);
        assert_eq!(Ok(100), state.balance(&a));
        state.revert_to_checkpoint(0);
        assert_eq!(Ok(0), state.balance(&a));
    }

    #[test]
    fn checkpoint_discard() {
        let mut state = get_temp_state();
        let a = Address::default();
        state.create_checkpoint(0);
        assert_eq!(Ok(()), state.add_balance(&a, 100));
        state.create_checkpoint(1);
        assert_eq!(Ok(()), state.add_balance(&a, 123));
        assert_eq!(Ok(()), state.inc_seq(&a));
        assert_eq!(Ok(100 + 123), state.balance(&a));
        assert_eq!(Ok(1), state.seq(&a));
        state.discard_checkpoint(1);
        assert_eq!(Ok(100 + 123), state.balance(&a));
        assert_eq!(Ok(1), state.seq(&a));
        state.revert_to_checkpoint(0);
        assert_eq!(Ok(0), state.balance(&a));
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
    use ccrypto::Blake;
    use ckey::{sign, Generator, Private, Random};
    use ctypes::transaction::Error as TransactionError;
    use primitives::H160;

    use super::*;
    use crate::tests::helpers::{get_temp_state, get_test_client};
    use crate::AssetSchemeAddress;

    fn address() -> (Address, Public, Private) {
        let keypair = Random.generate().unwrap();
        (keypair.address(), *keypair.public(), *keypair.private())
    }

    #[test]
    fn apply_error_for_invalid_seq() {
        let mut state = get_temp_state();

        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [(account: sender => balance: 20)]);

        let parcel = parcel!(seq: 2, fee: 5, payment!(address().0, 10));
        assert_eq!(
            Err(StateError::Parcel(ParcelError::InvalidSeq(Mismatch {
                expected: 0,
                found: 2
            }))),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 0, balance: 20))
        ]);
    }

    #[test]
    fn apply_error_for_not_enough_cash() {
        let mut state = get_temp_state();

        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [(account: sender => balance: 4)]);

        let parcel = parcel!(fee: 5, payment!(address().0, 10));
        assert_eq!(
            Err(StateError::Parcel(ParcelError::InsufficientBalance {
                address: sender,
                balance: 4,
                cost: 5,
            })),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 0, balance: 4))
        ]);
    }

    #[test]
    fn apply_payment() {
        let mut state = get_temp_state();

        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [(account: sender => balance: 20)]);

        let receiver = 1u64.into();
        let parcel = parcel!(fee: 5, payment!(receiver, 10));
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 5)),
            (account: receiver => (seq: 0, balance: 10))
        ]);
    }

    #[test]
    fn apply_set_regular_key() {
        let mut state = get_temp_state();
        let key = 1u64.into();

        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [(account: sender => balance: 5)]);

        let parcel = parcel!(fee: 5, set_regular_key!(key));
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 0, key: key))
        ]);
    }

    #[test]
    fn use_owner_balance_when_signed_with_regular_key() {
        let mut state = get_temp_state();

        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [(account: sender => balance: 15)]);

        let regular_keypair = Random.generate().unwrap();
        let key = regular_keypair.public();
        let parcel = parcel!(fee: 5, set_regular_key!(*key));

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 10, key: *key))
        ]);

        let parcel = parcel!(seq: 1, fee: 5, Action::CreateShard);

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&parcel, &H256::random(), regular_keypair.public(), &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 2, balance: 4)),
            (shard: 0 => owners: [sender])
        ]);
    }

    #[test]
    fn fail_when_two_accounts_used_the_same_regular_key() {
        let mut state = get_temp_state();

        let (sender, sender_public, _) = address();
        let (sender2, sender_public2, _) = address();
        set_top_level_state!(state, [
            (account: sender => balance: 15),
            (account: sender2 => balance: 15)
        ]);

        let regular_keypair = Random.generate().unwrap();
        let key = regular_keypair.public();
        let parcel = parcel!(fee: 5, set_regular_key!(*key));

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 10, key: *key)),
            (account: sender2 => (seq: 0, balance: 15, key))
        ]);

        let parcel = parcel!(fee: 5, set_regular_key!(*key));
        assert_eq!(
            Ok(Invoice::Failure(ParcelError::RegularKeyAlreadyInUse)),
            state.apply(&parcel, &H256::random(), &sender_public2, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 10, key: *key)),
            (account: sender2 => (seq: 1, balance: 10, key))
        ]);
    }

    #[test]
    fn fail_when_regular_key_is_already_registered_as_owner_key() {
        let mut state = get_temp_state();

        let (sender, sender_public, _) = address();
        let (sender2, sender_public2, _) = address();
        set_top_level_state!(state, [
            (account: sender => balance: 20),
            (account: sender2 => balance: 20)
        ]);

        let parcel = parcel! (fee: 5, set_regular_key!(sender_public2));
        assert_eq!(
            Ok(Invoice::Failure(ParcelError::RegularKeyAlreadyInUseAsPlatformAccount)),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 15))
        ]);
    }

    #[test]
    fn change_regular_key() {
        let mut state = get_temp_state();

        let (sender, sender_public, _) = address();
        let (_, regular_public, _) = address();
        set_top_level_state!(state, [
            (account: sender => balance: 20),
            (regular_key: sender_public => regular_public)
        ]);

        assert_eq!(Ok(true), state.regular_account_exists_and_not_null(&regular_public));

        let (_, regular_public2, _) = address();
        let parcel = parcel! (fee: 5, set_regular_key!(regular_public2));
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &regular_public, &get_test_client()));

        assert_eq!(Ok(false), state.regular_account_exists_and_not_null(&regular_public));
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 20 - 5, key: regular_public2))
        ]);
    }

    #[test]
    fn pass_approver_check_using_a_regular_key() {
        let (sender, sender_public, _) = address();
        let (_, regular_public, _) = address();

        let shard_id = 0x0;
        let mint_hash = H256::random();
        let mut state = get_temp_state();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = H256::from(asset_scheme_address);

        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 25),
            (regular_key: sender_public => regular_public),
            (scheme: (shard_id, asset_scheme_address) => { amount: amount, metadata: metadata, approver: Some(sender) }),
            (asset: (shard_id, mint_hash, 0) => { asset_type: asset_type, amount: amount, lock_script_hash: lock_script_hash })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_hash, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, vec![vec![1]], asset_type, 30)]
        );
        let transfer_parcel =
            parcel!(seq: 0, fee: 11, Action::AssetTransaction { transaction: transfer, approvals: vec![], });

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&transfer_parcel, &H256::random(), &regular_public, &get_test_client())
        );
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 25 - 11))
        ]);
    }


    #[test]
    fn pass_approver_check_using_a_regular_key_with_approval() {
        let (sender, sender_public, _) = address();
        let (_, regular_public, regular_private) = address();

        let shard_id = 0x0;
        let mint_hash = H256::random();
        let mut state = get_temp_state();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = H256::from(asset_scheme_address);

        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 25),
            (regular_key: sender_public => regular_public),
            (scheme: (shard_id, asset_scheme_address) => { amount: amount, metadata: metadata, approver: Some(sender) }),
            (asset: (shard_id, mint_hash, 0) => { asset_type: asset_type, amount: amount, lock_script_hash: lock_script_hash })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_hash, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, vec![vec![1]], asset_type, 30)]
        );
        let approval = sign(&regular_private, &transfer.hash()).unwrap();
        let transfer_parcel =
            parcel!(seq: 0, fee: 11, Action::AssetTransaction { transaction: transfer, approvals: vec![approval], });

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&transfer_parcel, &H256::random(), &regular_public, &get_test_client())
        );
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 25 - 11))
        ]);
    }

    #[test]
    fn use_deleted_regular_key_as_owner_key() {
        let (sender, sender_public, _) = address();
        let (regular_address, regular_public, _) = address();
        let (_, regular_public2, _) = address();

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (account: sender => balance: 20),
            (regular_key: sender_public => regular_public),
            (regular_key: sender_public => regular_public2),
            (account: regular_address => balance: 20)
        ]);

        assert_eq!(Ok(false), state.regular_account_exists_and_not_null(&regular_public));

        let parcel = parcel!(fee: 5, Action::CreateShard);
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &regular_public, &get_test_client()));
        check_top_level_state!(state, [
            (account: sender => (seq: 0, balance: 20)),
            (account: regular_address => (seq: 1, balance: 20 - 5 - 1)),
            (shard: 0 => owners: [regular_address])
        ]);
    }

    #[test]
    fn fail_when_someone_sends_some_ccc_to_an_address_which_used_as_a_regular_key() {
        let (sender, sender_public, _) = address();
        let (regular_address, regular_public, _) = address();

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (account: sender => balance: 20),
            (regular_key: sender_public => regular_public)
        ]);

        let parcel = parcel!(fee: 5, payment!(regular_address, 5));
        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InvalidTransferDestination)),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 20 - 5)),
            (account: regular_address => (seq: 0, balance: 0))
        ]);
    }

    #[test]
    fn apply_error_for_action_failure() {
        let mut state = get_temp_state();
        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [
            (account: sender => balance: 20)
        ]);

        let receiver = 1u64.into();
        let parcel = parcel!(fee: 5, payment!(receiver, 30));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientBalance {
                address: sender,
                balance: 15,
                cost: 30,
            })),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 20 - 5)),
            (account: receiver => (seq: 0, balance: 0))
        ]);
    }

    #[test]
    fn mint_permissioned_asset() {
        let (sender, sender_public, _) = address();

        let shard_id = 0x0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let amount = 30;
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters.clone(), amount),
            metadata.clone(),
            approver: approver
        );
        let transaction_hash = transaction.hash();
        let asset_type = H256::from(AssetSchemeAddress::new(transaction_hash, shard_id));
        let parcel = parcel!(fee: 11, Action::AssetTransaction {
            transaction,
            approvals: vec![],
        });

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 11)),
            (scheme: (transaction_hash, shard_id) => { metadata: metadata, amount: amount, approver: approver }),
            (asset: (transaction_hash, 0, shard_id) => { asset_type: asset_type, amount: amount })
        ]);
    }

    #[test]
    fn mint_infinite_permissioned_asset() {
        let (sender, sender_public, _) = address();

        let shard_id = 0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters: parameters.clone()),
            metadata.clone(),
            approver: approver
        );
        let transaction_hash = transaction.hash();
        let parcel = parcel!(fee: 5, Action::AssetTransaction {
            transaction,
            approvals: vec![],
        });

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        let asset_type = H256::from(AssetSchemeAddress::new(transaction_hash, shard_id));
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 5)),
            (scheme: (transaction_hash, shard_id) => { metadata: metadata, amount: ::std::u64::MAX, approver: approver }),
            (asset: (transaction_hash, 0, shard_id) => { asset_type: asset_type, amount: ::std::u64::MAX })
        ]);
    }

    #[test]
    fn mint_and_transfer_in_different_parcel() {
        let (sender, sender_public, _) = address();

        let shard_id = 0x00;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 120)
        ]);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, amount: amount), metadata.clone());
        let mint_hash = mint.hash();

        let mint_parcel = parcel!(fee: 20, Action::AssetTransaction {
            transaction: mint,
            approvals: vec![],
        });

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint_parcel, &H256::random(), &sender_public, &get_test_client())
        );

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.into();
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 120 - 20)),
            (scheme: (mint_hash, shard_id) => { metadata: metadata.clone(), amount: 30 }),
            (asset: (mint_hash, 0, shard_id) => { asset_type: asset_type, amount: 30 })
        ]);

        let random_lock_script_hash = H160::random();
        let transfer = asset_transfer!(
            inputs: vec![asset_transfer_input!(asset_out_point!(mint_hash, 0, asset_type, 30), vec![0x30, 0x01])],
            vec![
                asset_transfer_output!(lock_script_hash, vec![vec![1]], asset_type, 10),
                asset_transfer_output!(lock_script_hash, asset_type, 5),
                asset_transfer_output!(random_lock_script_hash, asset_type, 15),
            ]
        );
        let transfer_hash = transfer.hash();

        state.shard_root(shard_id).unwrap().unwrap();

        let transfer_parcel = parcel!(seq: 1, fee: 30, Action::AssetTransaction {
            transaction: transfer,
            approvals: vec![],
        });

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&transfer_parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 2, balance: 120 - 20 - 30)),
            (scheme: (mint_hash, shard_id) => { metadata: metadata.clone(), amount: 30 }),
            (asset: (mint_hash, 0, shard_id)),
            (asset: (transfer_hash, 0, shard_id) => { asset_type: asset_type, amount: 10 }),
            (asset: (transfer_hash, 1, shard_id) => { asset_type: asset_type, amount: 5 }),
            (asset: (transfer_hash, 2, shard_id) => { asset_type: asset_type, amount: 15 })
        ]);
    }

    #[test]
    fn cannot_mint_twice_in_different_parcel() {
        let (sender, sender_public, _) = address();

        let shard_id = 0x0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let amount = 30;
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters.clone(), amount),
            metadata.clone(),
            approver: approver
        );
        let parcel = parcel!(fee: 11, Action::AssetTransaction {
            transaction: transaction.clone(),
            approvals: vec![],
        });

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 11))
        ]);

        let transaction_hash = transaction.hash();
        let parcel = parcel!(seq: 1, fee: 11, Action::AssetTransaction {
            transaction,
            approvals: vec![],
        });
        assert_eq!(
            Ok(Invoice::Failure(TransactionError::AssetSchemeDuplicated(transaction_hash).into())),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 2, balance: 100 - 11 - 11))
        ]);
    }

    #[test]
    fn wrap_and_unwrap_ccc() {
        let (sender, sender_public, _) = address();

        let shard_id = 0x0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let amount = 30;

        let parcel = parcel!(fee: 11, wrap_ccc!(lock_script_hash, amount));
        let parcel_hash = parcel.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        let asset_type = H256::from(AssetSchemeAddress::new_with_zero_suffix(shard_id));
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 11 - 30)),
            (asset: (parcel_hash, 0, 0) => { asset_type: asset_type, amount: amount })
        ]);

        let unwrap_ccc_tx =
            asset_unwrap_ccc!(asset_transfer_input!(asset_out_point!(parcel_hash, 0, asset_type, 30), vec![0x01]));
        let parcel = parcel!(seq: 1, fee: 11, Action::AssetTransaction {
            transaction: unwrap_ccc_tx,
            approvals: vec![],
        });

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 2, balance: 100 - 11 - 30 - 11 + 30)),
            (asset: (parcel_hash, 0, 0))
        ]);
    }

    #[test]
    fn wrap_and_failed_unwrap() {
        let (sender, sender_public, _) = address();

        let shard_id = 0x0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let amount = 30;

        let parcel = parcel!(fee: 11, wrap_ccc!(lock_script_hash, amount));
        let parcel_hash = parcel.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        let asset_type = H256::from(AssetSchemeAddress::new_with_zero_suffix(shard_id));
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 11 - 30)),
            (asset: (parcel_hash, 0, 0) => { asset_type: asset_type, amount: amount })
        ]);

        let failed_lock_script = vec![0x02];
        let unwrap_ccc_tx = asset_unwrap_ccc!(asset_transfer_input!(
            asset_out_point!(parcel_hash, 0, asset_type, 30),
            failed_lock_script.clone()
        ));
        let parcel = parcel!(seq: 1, fee: 11, Action::AssetTransaction {
            transaction: unwrap_ccc_tx,
            approvals: vec![],
        });

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InvalidTransaction(TransactionError::ScriptHashMismatch(Mismatch {
                expected: lock_script_hash,
                found: Blake::blake(&failed_lock_script),
            })))),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 2, balance: 100 - 11 - 30 - 11)),
            (asset: (parcel_hash, 0, 0) => { asset_type: asset_type, amount: amount })
        ]);
    }

    #[test]
    fn wrap_ccc_with_insufficient_balance() {
        let (sender, sender_public, _) = address();
        let shard_id = 0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 20)
        ]);

        let lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let amount = 30;

        let parcel = parcel!(fee: 11, wrap_ccc!(lock_script_hash, amount));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientBalance {
                address: sender,
                balance: 9,
                cost: 30,
            })),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 20 - 11))
        ]);
    }

    #[test]
    fn wrap_ccc_and_transfer_and_unwrap_ccc() {
        let (sender, sender_public, _) = address();

        let shard_id = 0x0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;

        let parcel = parcel!(fee: 11, wrap_ccc!(lock_script_hash, amount));
        let parcel_hash = parcel.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        let asset_type = H256::from(AssetSchemeAddress::new_with_zero_suffix(shard_id));
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 30 - 11)),
            (asset: (parcel_hash, 0, 0) => { asset_type: asset_type, amount: amount })
        ]);

        let lock_script_hash_burn = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let random_lock_script_hash = H160::random();
        let transfer_tx = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(parcel_hash, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![
                (lock_script_hash, vec![vec![1]], asset_type, 10),
                (lock_script_hash_burn, asset_type, 5),
                (random_lock_script_hash, asset_type, 15),
            ]
        );
        let transfer_tx_hash = transfer_tx.hash();

        let parcel = parcel!(seq: 1, fee: 11, Action::AssetTransaction {
            transaction: transfer_tx,
            approvals: vec![],
        });

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 2, balance: 100 - 30 - 11 - 11)),
            (asset: (parcel_hash, 0, 0)),
            (asset: (transfer_tx_hash, 0, 0) => { asset_type: asset_type, amount: 10 }),
            (asset: (transfer_tx_hash, 1, 0) => { asset_type: asset_type, amount: 5 }),
            (asset: (transfer_tx_hash, 2, 0) => { asset_type: asset_type, amount: 15 })
        ]);

        let unwrap_ccc_tx =
            asset_unwrap_ccc!(asset_transfer_input!(asset_out_point!(transfer_tx_hash, 1, asset_type, 5), vec![0x01]));
        let parcel = parcel!(seq: 2, fee: 11, Action::AssetTransaction {
            transaction: unwrap_ccc_tx,
            approvals: vec![],
        });

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 3, balance: 100 - 30 - 11 - 11 - 11 + 5)),
            (asset: (transfer_tx_hash, 0, 0) => { asset_type: asset_type, amount: 10 }),
            (asset: (transfer_tx_hash, 1, 0)),
            (asset: (transfer_tx_hash, 2, 0) => { asset_type: asset_type, amount: 15 })
        ]);
    }

    #[test]
    fn get_invalid_shard_root() {
        let state = get_temp_state();

        let shard_id = 3;
        check_top_level_state!(state, [(shard: shard_id)]);
    }

    #[test]
    fn get_asset_in_invalid_shard() {
        let state = get_temp_state();

        let shard_id = 3;
        check_top_level_state!(state, [
            (asset: (H256::random(), 0, shard_id))
        ]);
    }


    #[test]
    fn get_asset_scheme_in_invalid_shard() {
        let state = get_temp_state();

        let shard_id = 3;
        check_top_level_state!(state, [(scheme: (H256::random(), shard_id))]);
    }

    #[test]
    fn apply_create_shard() {
        let mut state = get_temp_state();
        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [
            (account: sender => balance: 20)
        ]);

        let parcel = parcel!(fee: 5, Action::CreateShard);
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 20 - 5 - 1)),
            (shard: 0 => owners: [sender]),
            (shard: 1)
        ]);
    }

    #[test]
    fn get_asset_in_invalid_shard2() {
        let mut state = get_temp_state();
        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [
            (account: sender => balance: 20)
        ]);

        let parcel = parcel!(fee: 5, Action::CreateShard);
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        let invalid_shard_id = 3;
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 20 - 5 - 1)),
            (shard: 0 => owners: [sender]),
            (asset: (H256::random(), 0, invalid_shard_id))
        ]);
    }

    #[test]
    fn get_asset_scheme_in_invalid_shard2() {
        let mut state = get_temp_state();
        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [
            (account: sender => balance: 20)
        ]);

        let parcel = parcel!(fee: 5, Action::CreateShard);
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        let invalid_shard_id = 3;
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 20 - 5 - 1)),
            (shard: 0 => owners: [sender]),
            (asset: (H256::random(), 0, invalid_shard_id))
        ]);
    }

    #[test]
    fn mint_asset_on_invalid_shard_must_fail() {
        let mut state = get_temp_state();
        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [
            (account: sender => balance: 100)
        ]);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let amount = 30;
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters.clone(), amount),
            metadata.clone(),
            approver: approver
        );
        let parcel = parcel!(fee: 11, Action::AssetTransaction {
            transaction,
            approvals: vec![],
        });

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InvalidShardId(0))),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 11))
        ]);
    }

    #[test]
    fn transfer_on_invalid_parcel_must_fail() {
        let (sender, sender_public, _) = address();

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (account: sender => balance: 120)
        ]);

        let shard_id = 100;

        let asset_type = H256::from(AssetSchemeAddress::new(H256::zero(), shard_id));
        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(H256::random(), 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![
                (H160::random(), vec![vec![1]], asset_type, 10),
                (H160::random(), asset_type, 5),
                (H160::random(), asset_type, 15),
            ]
        );

        let parcel = parcel!(fee: 30, Action::AssetTransaction {
            transaction: transfer,
            approvals: vec![],
        });

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InvalidShardId(100))),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 120 - 30))
        ]);
    }

    #[test]
    fn set_shard_owners() {
        let (sender, sender_public, _) = address();

        let shard_id = 0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let owners = vec![Address::random(), Address::random(), sender];

        let parcel = parcel!(fee: 5, set_shard_owners!(owners.clone()));
        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 5)),
            (shard: 0 => owners: owners)
        ]);
    }

    #[test]
    fn new_owners_must_contain_sender() {
        let (sender, sender_public, _) = address();

        let shard_id = 0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let owners = vec![Address::random(), Address::random()];
        let parcel = parcel!(fee: 5, set_shard_owners!(owners));
        assert_eq!(
            Ok(Invoice::Failure(ParcelError::NewOwnersMustContainSender)),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 5)),
            (shard: 0 => owners: [sender])
        ]);
    }

    #[test]
    fn only_owner_can_set_owners() {
        let (original_owner, ..) = address();

        let shard_id = 0;

        let mut state = get_temp_state();
        let (sender, sender_public, _) = address();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [original_owner]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let owners = vec![Address::random(), Address::random(), sender];
        let parcel = parcel!(fee: 5, set_shard_owners!(owners));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientPermission)),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 5)),
            (shard: 0 => owners: [original_owner])
        ]);
    }

    #[test]
    fn set_shard_owners_fail_on_invalid_shard_id() {
        let (sender, sender_public, _) = address();
        let shard_id = 0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let invalid_shard_id = 0xF;
        let owners = vec![Address::random(), Address::random(), sender];
        let parcel = parcel!(fee: 5, set_shard_owners!(shard_id: invalid_shard_id, owners));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InvalidShardId(invalid_shard_id))),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 5)),
            (shard: 0 => owners: [sender]),
            (shard: invalid_shard_id)
        ]);
    }

    #[test]
    fn user_cannot_set_owners() {
        let (original_owner, ..) = address();
        let (sender, sender_public, _) = address();
        let shard_id = 0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [original_owner], users: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let owners = vec![Address::random(), Address::random(), sender];

        let parcel = parcel!(fee: 5, set_shard_owners!(owners));
        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientPermission)),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );

        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 5)),
            (shard: 0 => owners: [original_owner])
        ]);
    }


    #[test]
    fn user_can_mint() {
        let (original_owner, ..) = address();
        let (sender, sender_public, _) = address();
        let shard_id = 0x00;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [original_owner], users: [sender]),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let parameters = vec![];

        let mint = asset_mint!(asset_mint_output!(lock_script_hash, parameters.clone(), amount), metadata.clone());
        let mint_hash = mint.hash();

        let parcel = parcel!(fee: 20, Action::AssetTransaction {
            transaction: mint,
            approvals: vec![],
        });

        assert_eq!(
            Invoice::Success,
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()).unwrap()
        );

        let asset_type = H256::from(AssetSchemeAddress::new(mint_hash, shard_id));
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 20)),
            (scheme: (mint_hash, shard_id) => { metadata: metadata.clone(), amount: amount }),
            (asset: (mint_hash, 0, shard_id) => { asset_type: asset_type, amount: amount })
        ]);
    }

    #[test]
    fn set_shard_users() {
        let (sender, sender_public, _) = address();
        let old_users = vec![Address::random(), Address::random(), Address::random()];
        let shard_id = 0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: [sender], users: old_users),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let new_users = vec![Address::random(), Address::random(), sender];
        let parcel = parcel!(fee: 5, set_shard_users!(new_users.clone()));

        assert_eq!(Ok(Invoice::Success), state.apply(&parcel, &H256::random(), &sender_public, &get_test_client()));
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 5))
        ]);
    }


    #[test]
    fn user_cannot_set_shard_users() {
        let (sender, sender_public, _) = address();
        let owners = vec![Address::random(), Address::random(), Address::random()];
        let old_users = vec![Address::random(), Address::random(), Address::random(), sender];
        let shard_id = 0;

        let mut state = get_temp_state();
        set_top_level_state!(state, [
            (shard: shard_id => owners: owners.clone(), users: old_users.clone()),
            (metadata: shards: 1),
            (account: sender => balance: 100)
        ]);

        let new_users = vec![Address::random(), Address::random(), sender];
        let parcel = parcel!(fee: 5, set_shard_users!(new_users.clone()));

        assert_eq!(
            Ok(Invoice::Failure(ParcelError::InsufficientPermission)),
            state.apply(&parcel, &H256::random(), &sender_public, &get_test_client())
        );
        check_top_level_state!(state, [
            (account: sender => (seq: 1, balance: 100 - 5)),
            (shard: 0 => owners: owners, users: old_users)
        ]);
    }
}
