// Copyright 2018-2019 Kodebox, Inc.
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

use std::cell::{RefCell, RefMut};
use std::collections::HashSet;
use std::iter::{once, FromIterator};

use ccrypto::{Blake, BLAKE_NULL_RLP};
use ckey::Address;
use cmerkle::{self, TrieError, TrieFactory};
use ctypes::errors::{RuntimeError, UnlockFailureReason};
use ctypes::transaction::{
    AssetMintOutput, AssetOutPoint, AssetTransferInput, AssetTransferOutput, AssetWrapCCCOutput, Order,
    OrderOnTransfer, PartialHashing, ShardTransaction,
};
use ctypes::util::unexpected::Mismatch;
use ctypes::{BlockNumber, ShardId};
use cvm::{decode, execute, ChainTimeInfo, ScriptResult, VMConfig};
use hashdb::AsHashDB;
use primitives::{Bytes, H160, H256};

use crate::cache::ShardCache;
use crate::checkpoint::{CheckpointId, StateWithCheckpoint};
use crate::traits::{ShardState, ShardStateView};
use crate::{Asset, AssetScheme, AssetSchemeAddress, OwnedAsset, OwnedAssetAddress, StateDB, StateResult};


pub struct ShardLevelState<'db> {
    db: &'db mut RefCell<StateDB>,
    root: H256,
    cache: &'db mut ShardCache,
    id_of_checkpoints: Vec<CheckpointId>,
    shard_id: ShardId,
}

impl<'db> ShardLevelState<'db> {
    /// Creates new state with empty state root
    pub fn try_new(shard_id: ShardId, db: &'db mut RefCell<StateDB>, cache: &'db mut ShardCache) -> StateResult<Self> {
        let root = BLAKE_NULL_RLP;
        Ok(Self {
            db,
            root,
            cache,
            id_of_checkpoints: Default::default(),
            shard_id,
        })
    }

    /// Creates new state with existing state root
    pub fn from_existing(
        shard_id: ShardId,
        db: &'db mut RefCell<StateDB>,
        root: H256,
        cache: &'db mut ShardCache,
    ) -> cmerkle::Result<Self> {
        if !db.borrow().as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        Ok(Self {
            db,
            root,
            cache,
            id_of_checkpoints: Default::default(),
            shard_id,
        })
    }

    /// Creates immutable shard state
    pub fn read_only(
        shard_id: ShardId,
        db: &RefCell<StateDB>,
        root: H256,
        cache: ShardCache,
    ) -> cmerkle::Result<ReadOnlyShardLevelState> {
        if !db.borrow().as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        Ok(ReadOnlyShardLevelState {
            db,
            root,
            cache,
            shard_id,
        })
    }

    fn apply_internal<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        sender: &Address,
        shard_users: &[Address],
        approvers: &[Address],
        client: &C,
        parent_block_number: BlockNumber,
        parent_block_timestamp: u64,
    ) -> StateResult<()> {
        match transaction {
            ShardTransaction::MintAsset {
                metadata,
                shard_id,
                approver,
                registrar,
                allowed_script_hashes,
                output,
                ..
            } => {
                assert_eq!(*shard_id, self.shard_id);
                self.mint_asset(
                    transaction.tracker(),
                    metadata,
                    output,
                    approver,
                    approvers,
                    registrar,
                    allowed_script_hashes,
                    sender,
                    shard_users,
                    Vec::new(),
                )?;
                Ok(())
            }
            ShardTransaction::TransferAsset {
                burns,
                inputs,
                outputs,
                orders,
                ..
            } => {
                debug_assert!(outputs.len() <= 512);
                self.transfer_asset(
                    &transaction,
                    sender,
                    approvers,
                    burns,
                    inputs,
                    outputs,
                    orders,
                    client,
                    parent_block_number,
                    parent_block_timestamp,
                )
            }
            ShardTransaction::ChangeAssetScheme {
                shard_id,
                asset_type,
                seq,
                metadata,
                approver,
                registrar,
                allowed_script_hashes,
                ..
            } => {
                assert_eq!(*shard_id, self.shard_id);
                self.change_asset_scheme(
                    sender,
                    approvers,
                    asset_type,
                    *seq,
                    metadata,
                    approver,
                    registrar,
                    allowed_script_hashes,
                )
            }
            ShardTransaction::IncreaseAssetSupply {
                shard_id,
                asset_type,
                output,
                seq,
                ..
            } => {
                assert_eq!(*shard_id, self.shard_id);
                self.increase_asset_supply(transaction.tracker(), *seq, sender, approvers, asset_type, output)
            }
            ShardTransaction::UnwrapCCC {
                burn,
                ..
            } => {
                assert_eq!(burn.prev_out.shard_id, self.shard_id);
                self.unwrap_ccc(&transaction, sender, burn, client, parent_block_number, parent_block_timestamp)
            }
            ShardTransaction::WrapCCC {
                tx_hash,
                output:
                    AssetWrapCCCOutput {
                        lock_script_hash,
                        quantity,
                        parameters,
                    },
                shard_id,
                ..
            } => {
                assert_eq!(*shard_id, self.shard_id);
                self.wrap_ccc(tx_hash, lock_script_hash, &parameters, *quantity)
            }
        }
    }

    // FIXME: Remove this clippy config
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::too_many_arguments))]
    fn mint_asset(
        &mut self,
        transaction_tracker: H256,
        metadata: &str,
        output: &AssetMintOutput,
        approver: &Option<Address>,
        approvers: &[Address],
        registrar: &Option<Address>,
        allowed_script_hashes: &[H160],
        sender: &Address,
        shard_users: &[Address],
        pool: Vec<Asset>,
    ) -> StateResult<()> {
        if !shard_users.is_empty() {
            let sender_and_approvers: HashSet<&Address> = HashSet::from_iter(once(sender).chain(approvers.iter()));
            let shard_users = HashSet::from_iter(shard_users.iter());
            if shard_users.is_disjoint(&sender_and_approvers) {
                return Err(RuntimeError::InsufficientPermission.into())
            }
        }

        let asset_type = Blake::blake(transaction_tracker);
        if self.asset_scheme(asset_type)?.is_some() {
            return Err(RuntimeError::AssetSchemeDuplicated {
                tracker: transaction_tracker,
                shard_id: self.shard_id,
            }
            .into())
        }
        let asset_scheme = self.create_asset_scheme(
            self.shard_id,
            asset_type,
            metadata.to_string(),
            output.supply,
            *approver,
            *registrar,
            allowed_script_hashes.to_vec(),
            pool,
        )?;

        ctrace!(TX, "{:?} is minted on {}:{:?}", asset_scheme, self.shard_id, asset_type);

        self.create_asset(
            transaction_tracker,
            0,
            asset_type,
            output.lock_script_hash,
            output.parameters.clone(),
            output.supply,
            None,
        )?;
        ctrace!(TX, "Created asset on {}:{}:{}", self.shard_id, transaction_tracker, 0);
        Ok(())
    }

    // FIXME: Remove this clippy config
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::too_many_arguments))]
    fn transfer_asset<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        sender: &Address,
        approvers: &[Address],
        burns: &[AssetTransferInput],
        inputs: &[AssetTransferInput],
        outputs: &[AssetTransferOutput],
        orders: &[OrderOnTransfer],
        client: &C,
        parent_block_number: BlockNumber,
        parent_block_timestamp: u64,
    ) -> StateResult<()> {
        let mut values_to_hash = vec![None; inputs.len()];
        for order_tx in orders {
            let order = &order_tx.order;
            for input_idx in order_tx.input_from_indices.iter().chain(order_tx.input_fee_indices.iter()) {
                values_to_hash[*input_idx] = Some(order);
            }
        }

        for (input, transaction, order, burn) in inputs
            .iter()
            .enumerate()
            .map(|(index, input)| (input, transaction, values_to_hash[index], false))
            .chain(burns.iter().map(|input| (input, transaction, None, true)))
        {
            if input.prev_out.shard_id != self.shard_id {
                continue
            }
            self.check_and_run_input_script(
                input,
                transaction,
                order,
                burn,
                sender,
                approvers,
                client,
                parent_block_number,
                parent_block_timestamp,
            )?;
        }

        self.check_orders(orders, inputs)?;
        let mut output_order_hashes = vec![None; outputs.len()];
        for order_tx in orders {
            let order = &order_tx.order;
            for output_idx in order_tx
                .output_from_indices
                .iter()
                .chain(order_tx.output_to_indices.iter())
                .chain(order_tx.output_owned_fee_indices.iter())
                .chain(order_tx.output_transferred_fee_indices.iter())
            {
                output_order_hashes[*output_idx] = Some(order.consume(order_tx.spent_quantity).hash());
            }
        }

        let mut deleted_asset = Vec::with_capacity(inputs.len() + burns.len());
        for input in inputs.iter().chain(burns) {
            if input.prev_out.shard_id != self.shard_id {
                continue
            }
            self.kill_asset(input.prev_out.tracker, input.prev_out.index);
            deleted_asset.push((input.prev_out.tracker, input.prev_out.index, input.prev_out.quantity));
        }
        let transaction_tracker = transaction.tracker();
        for (index, output) in outputs.iter().enumerate() {
            if output.shard_id != self.shard_id {
                continue
            }
            self.check_output_script_hash(output, sender, approvers)?;
            self.create_asset(
                transaction_tracker,
                index,
                output.asset_type,
                output.lock_script_hash,
                output.parameters.clone(),
                output.quantity,
                output_order_hashes[index],
            )?;
        }
        let mut reduced_supplies = Vec::with_capacity(burns.len());
        for burn in burns {
            let AssetOutPoint {
                asset_type,
                shard_id,
                quantity,
                ..
            } = burn.prev_out;
            if shard_id != self.shard_id {
                continue
            }
            let mut asset_scheme = self.get_asset_scheme_mut(shard_id, asset_type)?;
            let previous_supply = asset_scheme.reduce_supply(quantity);
            reduced_supplies.push((asset_type, previous_supply, quantity))
        }

        ctrace!(TX, "Deleted assets on {} {:?}", self.shard_id, deleted_asset);
        ctrace!(TX, "Created assets {}:{}:(0..{})", self.shard_id, transaction_tracker, outputs.len());
        ctrace!(TX, "Reduced asset supplies {:?}", reduced_supplies);
        Ok(())
    }

    fn check_orders(&self, orders: &[OrderOnTransfer], inputs: &[AssetTransferInput]) -> StateResult<()> {
        for order_tx in orders {
            let order = &order_tx.order;
            let mut counter: usize = 0;
            for input_idx in order_tx.input_from_indices.iter().chain(order_tx.input_fee_indices.iter()) {
                let input = &inputs[*input_idx];
                let tracker = input.prev_out.tracker;
                let index = input.prev_out.index;
                if input.prev_out.shard_id != self.shard_id {
                    continue
                }
                let asset = self.asset(tracker, index)?.ok_or_else(|| RuntimeError::AssetNotFound {
                    shard_id: self.shard_id,
                    tracker,
                    index,
                })?;

                match &asset.order_hash() {
                    Some(order_hash) if *order_hash == order.hash() => {}
                    _ => {
                        if order.origin_outputs.contains(&input.prev_out) {
                            counter += 1;
                        } else {
                            return Err(RuntimeError::InvalidOriginOutputs(order.hash()).into())
                        }
                    }
                }
            }
            if counter > 0 && counter != order.origin_outputs.len() {
                return Err(RuntimeError::InvalidOriginOutputs(order.hash()).into())
            }
        }
        Ok(())
    }

    fn approved_by_registrar(&self, asset_type: H160, sender: &Address, approvers: &[Address]) -> StateResult<bool> {
        let asset_scheme = self.asset_scheme(asset_type)?.ok_or_else(|| RuntimeError::AssetSchemeNotFound {
            asset_type,
            shard_id: self.shard_id,
        })?;

        if let Some(registrar) = asset_scheme.registrar() {
            Ok(registrar == sender || approvers.contains(registrar))
        } else {
            Ok(false)
        }
    }

    fn change_asset_scheme(
        &mut self,
        sender: &Address,
        approvers: &[Address],
        asset_type: &H160,
        seq: usize,
        metadata: &str,
        approver: &Option<Address>,
        registrar: &Option<Address>,
        allowed_script_hashes: &[H160],
    ) -> StateResult<()> {
        if !self.approved_by_registrar(*asset_type, sender, approvers)? {
            return Err(RuntimeError::InsufficientPermission.into())
        }

        let mut asset_scheme = self.get_asset_scheme_mut(self.shard_id, *asset_type)?;
        if asset_scheme.seq() != seq {
            return Err(RuntimeError::InvalidSeqOfAssetScheme {
                asset_type: *asset_type,
                shard_id: self.shard_id,
                expected: asset_scheme.seq(),
                actual: seq,
            }
            .into())
        }

        asset_scheme.change_data(
            metadata.to_string(),
            approver.clone(),
            registrar.clone(),
            allowed_script_hashes.to_vec(),
        );
        asset_scheme.increase_seq();

        Ok(())
    }

    fn increase_asset_supply(
        &mut self,
        transaction_tracker: H256,
        seq: usize,
        sender: &Address,
        approvers: &[Address],
        asset_type: &H160,
        output: &AssetMintOutput,
    ) -> StateResult<()> {
        if !self.approved_by_registrar(*asset_type, sender, approvers)? {
            return Err(RuntimeError::InsufficientPermission.into())
        }

        // This assertion should be filtered while verifying action.
        assert!(output.supply > 0, "Supply increasing quantity must be specified and greater than 0");

        let mut asset_scheme = self.get_asset_scheme_mut(self.shard_id, *asset_type)?;
        if seq != asset_scheme.seq() {
            return Err(RuntimeError::InvalidSeqOfAssetScheme {
                asset_type: *asset_type,
                shard_id: self.shard_id,
                expected: asset_scheme.seq(),
                actual: seq,
            }
            .into())
        }
        let previous_supply = asset_scheme.increase_supply(output.supply)?;
        asset_scheme.increase_seq();
        self.create_asset(
            transaction_tracker,
            0,
            *asset_type,
            output.lock_script_hash,
            output.parameters.clone(),
            output.supply,
            None,
        )?;
        ctrace!(TX, "Increased asset supply {:?} {:?} => {:?}", asset_type, previous_supply, output.supply);
        ctrace!(TX, "Created asset on {}:{}", self.shard_id, transaction_tracker);

        Ok(())
    }

    fn check_input_asset(
        &self,
        input: &AssetTransferInput,
        sender: &Address,
        approvers: &[Address],
    ) -> StateResult<(OwnedAsset, bool)> {
        let AssetOutPoint {
            index,
            tracker,
            asset_type,
            shard_id,
            quantity,
        } = input.prev_out;

        assert_eq!(self.shard_id, shard_id);
        let approved_by_regulator = self.approved_by_registrar(asset_type, sender, approvers)?;
        if !approved_by_regulator {
            let asset_scheme = self.asset_scheme(asset_type)?.ok_or_else(|| RuntimeError::AssetSchemeNotFound {
                shard_id,
                asset_type,
            })?;
            if let Some(approver) = asset_scheme.approver() {
                if sender != approver && !approvers.contains(approver) {
                    return Err(RuntimeError::NotApproved(*approver).into())
                }
            }
        }

        let asset = self.asset(tracker, index)?.ok_or_else(|| RuntimeError::AssetNotFound {
            shard_id,
            tracker,
            index,
        })?;
        if asset.quantity() != quantity {
            return Err(RuntimeError::InvalidAssetQuantity {
                shard_id,
                tracker,
                index,
                expected: asset.quantity(),
                got: quantity,
            }
            .into())
        }
        if *asset.asset_type() != asset_type {
            return Err(RuntimeError::UnexpectedAssetType {
                index,
                mismatch: Mismatch {
                    expected: *asset.asset_type(),
                    found: asset_type,
                },
            }
            .into())
        }
        Ok((asset, approved_by_regulator))
    }

    fn check_output_script_hash(
        &self,
        output: &AssetTransferOutput,
        sender: &Address,
        approvers: &[Address],
    ) -> StateResult<()> {
        let asset_scheme = {
            assert_eq!(self.shard_id, output.shard_id);
            self.asset_scheme(output.asset_type)?.ok_or_else(|| RuntimeError::AssetSchemeNotFound {
                asset_type: output.asset_type,
                shard_id: self.shard_id,
            })?
        };
        if let Some(registrar) = asset_scheme.registrar().as_ref() {
            if sender == registrar || approvers.contains(registrar) {
                return Ok(())
            }
        }

        let lock_script_hash = output.lock_script_hash;
        if asset_scheme.is_allowed_script_hash(&lock_script_hash) {
            Ok(())
        } else {
            Err(RuntimeError::ScriptNotAllowed(lock_script_hash).into())
        }
    }

    fn check_and_run_input_script<C: ChainTimeInfo>(
        &self,
        input: &AssetTransferInput,
        transaction: &PartialHashing,
        order: Option<&Order>,
        burn: bool,
        sender: &Address,
        approvers: &[Address],
        client: &C,
        parent_block_number: BlockNumber,
        parent_block_timestamp: u64,
    ) -> StateResult<()> {
        debug_assert!(!burn || order.is_none());

        let (asset, from_regulator) = self.check_input_asset(input, sender, approvers)?;
        if from_regulator {
            return Ok(()) // Don't execute scripts when regulator sends the transaction.
        }

        let to_hash: &PartialHashing = if let Some(order) = order {
            if let Some(order_hash) = &asset.order_hash() {
                if *order_hash == order.hash() {
                    // If an order on an input and an order on the corresponding prev_out(asset) is same,
                    // then skip checking lock script and running VM.
                    return Ok(())
                }
            }
            order
        } else {
            transaction
        };

        if *asset.lock_script_hash() != Blake::blake(&input.lock_script) {
            return Err(RuntimeError::ScriptHashMismatch(Mismatch {
                expected: *asset.lock_script_hash(),
                found: Blake::blake(&input.lock_script),
            })
            .into())
        }

        let script_result = match (decode(&input.lock_script), decode(&input.unlock_script)) {
            (Ok(lock_script), Ok(unlock_script)) => execute(
                &unlock_script,
                &asset.parameters(),
                &lock_script,
                to_hash,
                VMConfig::default(),
                input,
                burn,
                client,
                parent_block_number,
                parent_block_timestamp,
            ),
            // FIXME : Deliver full decode error
            _ => return Err(RuntimeError::InvalidScript.into()),
        };

        match (script_result, burn) {
            (Ok(ScriptResult::Burnt), true) => Ok(()),
            (Ok(ScriptResult::Burnt), false) => Err(UnlockFailureReason::ScriptShouldBeBurnt),
            (Ok(ScriptResult::Unlocked), false) => Ok(()),
            (Ok(ScriptResult::Unlocked), true) => Err(UnlockFailureReason::ScriptShouldNotBeBurnt),
            (Ok(ScriptResult::Fail), _) | (Err(_), _) => Err(UnlockFailureReason::ScriptError),
        }
        .map_err(|reason| {
            ctrace!(TX, "Cannot run unlock/lock script {:?}", reason);
            RuntimeError::FailedToUnlock {
                shard_id: self.shard_id,
                tracker: input.prev_out.tracker,
                index: input.prev_out.index,
                reason,
            }
            .into()
        })
    }

    fn wrap_ccc(
        &mut self,
        tx_hash: &H256,
        lock_script_hash: &H160,
        parameters: &[Bytes],
        quantity: u64,
    ) -> StateResult<()> {
        let asset_type = H160::zero();
        if self.asset_scheme(asset_type)?.is_none() {
            let asset_scheme = self.create_asset_scheme(
                self.shard_id,
                asset_type,
                String::new(),
                0,
                None,
                None,
                Vec::new(),
                Vec::new(),
            );
            // FIXME: Wrapped CCC is minted in here, but the metadata is not well-defined.
            ctrace!(TX, "Wrapped CCC in shard {} ({:?}) is minted on {:?}", self.shard_id, asset_scheme, asset_type);
        }
        let mut asset_scheme = self.get_asset_scheme_mut(self.shard_id, asset_type)?;
        asset_scheme.increase_supply(quantity)?;

        self.create_asset(*tx_hash, 0, asset_type, *lock_script_hash, parameters.to_vec(), quantity, None)?;
        ctrace!(TX, "Created Wrapped CCC on {}:{}:{}", self.shard_id, tx_hash, 0);
        Ok(())
    }

    fn unwrap_ccc<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        sender: &Address,
        burn: &AssetTransferInput,
        client: &C,
        parent_block_number: BlockNumber,
        parent_block_timestamp: u64,
    ) -> StateResult<()> {
        // WCCC has no approvers
        let approvers = [];
        self.check_and_run_input_script(
            burn,
            transaction,
            None,
            true,
            sender,
            &approvers,
            client,
            parent_block_number,
            parent_block_timestamp,
        )?;

        self.kill_asset(burn.prev_out.tracker, burn.prev_out.index);
        let asset_type = H160::zero();
        let mut asset_scheme = self.get_asset_scheme_mut(self.shard_id, asset_type)?;
        asset_scheme.reduce_supply(burn.prev_out.quantity);
        ctrace!(
            TX,
            "Removed Wrapped CCC asset {}:{}:{}, quantity {:?}",
            self.shard_id,
            burn.prev_out.tracker,
            burn.prev_out.index,
            burn.prev_out.quantity
        );
        Ok(())
    }

    fn kill_asset(&mut self, tracker: H256, index: usize) {
        self.cache.remove_asset(&OwnedAssetAddress::new(tracker, index, self.shard_id));
    }

    pub fn create_asset_scheme(
        &self,
        shard_id: ShardId,
        asset_type: H160,
        metadata: String,
        supply: u64,
        approver: Option<Address>,
        registrar: Option<Address>,
        allowed_script_hashes: Vec<H160>,
        pool: Vec<Asset>,
    ) -> cmerkle::Result<AssetScheme> {
        self.cache.create_asset_scheme(&AssetSchemeAddress::new(asset_type, shard_id), || {
            AssetScheme::new_with_pool(metadata, supply, approver, registrar, allowed_script_hashes, pool)
        })
    }

    fn get_asset_scheme_mut(&self, shard_id: ShardId, asset_type: H160) -> cmerkle::Result<RefMut<AssetScheme>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset_scheme_mut(&AssetSchemeAddress::new(asset_type, shard_id), &trie)
    }

    pub fn create_asset(
        &self,
        tracker: H256,
        index: usize,
        asset_type: H160,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        quantity: u64,
        order_hash: Option<H256>,
    ) -> cmerkle::Result<OwnedAsset> {
        self.cache.create_asset(&OwnedAssetAddress::new(tracker, index, self.shard_id), || {
            OwnedAsset::new(asset_type, lock_script_hash, parameters, quantity, order_hash)
        })
    }

    #[cfg(test)]
    fn shard_id(&self) -> ShardId {
        self.shard_id
    }
}

impl<'db> ShardStateView for ShardLevelState<'db> {
    fn asset_scheme(&self, asset_type: H160) -> cmerkle::Result<Option<AssetScheme>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset_scheme(&AssetSchemeAddress::new(asset_type, self.shard_id), &trie)
    }

    fn asset(&self, tracker: H256, index: usize) -> Result<Option<OwnedAsset>, TrieError> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset(&OwnedAssetAddress::new(tracker, index, self.shard_id), &trie)
    }
}

impl<'db> StateWithCheckpoint for ShardLevelState<'db> {
    fn create_checkpoint(&mut self, id: CheckpointId) {
        ctrace!(STATE, "Checkpoint({}) for shard({}) is created", id, self.shard_id);
        self.id_of_checkpoints.push(id);
        self.cache.checkpoint();
    }

    fn discard_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        ctrace!(STATE, "Checkpoint({}) for shard({}) is discarded", id, self.shard_id);
        self.cache.discard_checkpoint();
    }

    fn revert_to_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        ctrace!(STATE, "Checkpoint({}) for shard({}) is reverted", id, self.shard_id);
        self.cache.revert_to_checkpoint();
    }
}

const TRANSACTION_CHECKPOINT: CheckpointId = 456;

impl<'db> ShardState for ShardLevelState<'db> {
    fn apply<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        sender: &Address,
        shard_users: &[Address],
        approvers: &[Address],
        client: &C,
        parent_block_number: BlockNumber,
        parent_block_timestamp: u64,
    ) -> StateResult<()> {
        ctrace!(TX, "Execute InnerTx {:?}(InnerTxHash:{:?})", transaction, transaction.tracker());

        self.create_checkpoint(TRANSACTION_CHECKPOINT);
        let result = self.apply_internal(
            transaction,
            sender,
            shard_users,
            approvers,
            client,
            parent_block_number,
            parent_block_timestamp,
        );
        match result {
            Ok(_) => {
                self.discard_checkpoint(TRANSACTION_CHECKPOINT);
                Ok(())
            }
            Err(err) => {
                self.revert_to_checkpoint(TRANSACTION_CHECKPOINT);
                Err(err)
            }
        }
    }
}

pub struct ReadOnlyShardLevelState<'db> {
    db: &'db RefCell<StateDB>,
    root: H256,
    cache: ShardCache,
    shard_id: ShardId,
}

impl<'db> ShardStateView for ReadOnlyShardLevelState<'db> {
    fn asset_scheme(&self, asset_type: H160) -> cmerkle::Result<Option<AssetScheme>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset_scheme(&AssetSchemeAddress::new(asset_type, self.shard_id), &trie)
    }

    fn asset(&self, tracker: H256, index: usize) -> Result<Option<OwnedAsset>, TrieError> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset(&OwnedAssetAddress::new(tracker, index, self.shard_id), &trie)
    }
}

#[cfg(test)]
mod tests {
    use ctypes::transaction::AssetOutPoint;

    use super::super::super::StateError;
    use super::super::test_helper::SHARD_ID;
    use super::*;
    use crate::tests::helpers::{get_temp_state_db, get_test_client};

    fn address() -> Address {
        Address::random()
    }

    fn get_temp_shard_state<'d>(
        state_db: &'d mut RefCell<StateDB>,
        shard_id: ShardId,
        cache: &'d mut ShardCache,
    ) -> ShardLevelState<'d> {
        ShardLevelState::try_new(shard_id, state_db, cache).unwrap()
    }

    #[test]
    fn mint_permissioned_asset() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let amount = 100;
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters.clone(), amount),
            metadata.clone(),
            approver: approver
        );

        let transaction_tracker = transaction.tracker();
        let asset_type = Blake::blake(transaction_tracker);
        assert_eq!(Ok(()), state.apply(&transaction, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, approver: approver }),
            (asset: (transaction_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);
    }

    #[test]
    fn mint_infinite_asset() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters: parameters.clone()),
            metadata.clone(),
            approver: approver
        );
        let transaction_tracker = transaction.tracker();
        let asset_type = Blake::blake(transaction_tracker);

        assert_eq!(Ok(()), state.apply(&transaction, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: ::std::u64::MAX, approver: approver }),
            (asset: (transaction_tracker, 0) => { asset_type: asset_type, quantity: ::std::u64::MAX })
        ]);
    }

    #[test]
    fn cannot_mint_twice() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters: parameters.clone()),
            metadata.clone(),
            approver: approver
        );

        let transaction_tracker = transaction.tracker();
        let asset_type = Blake::blake(transaction_tracker);
        assert_eq!(Ok(()), state.apply(&transaction, &sender, &[sender], &[], &get_test_client(), 0, 0));

        assert_eq!(
            Err(StateError::Runtime(RuntimeError::AssetSchemeDuplicated {
                tracker: transaction_tracker,
                shard_id: SHARD_ID
            })),
            state.apply(&transaction, &sender, &[sender], &[], &get_test_client(), 0, 0)
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: ::std::u64::MAX, approver: approver }),
            (asset: (transaction_tracker, 0) => { asset_type: asset_type, quantity: ::std::u64::MAX })
        ]);
    }

    #[test]
    fn invalid_approver() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let approver = Address::random();
        let amount = 30;
        let mint =
            asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone(), approver: approver);
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, approver: approver }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, asset_type, 30)]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(
            Err(StateError::Runtime(RuntimeError::NotApproved(approver))),
            state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0)
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, approver: approver }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount }),
            (asset: (transfer_tracker, 0))
        ]);
    }

    #[test]
    fn mint_and_transfer() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let random_lock_script_hash = H160::random();
        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![
                (lock_script_hash, vec![vec![1]], asset_type, 10),
                (lock_script_hash, asset_type, 5),
                (random_lock_script_hash, asset_type, 15),
            ]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(Ok(()), state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0)),
            (asset: (transfer_tracker, 0) => { asset_type: asset_type, quantity: 10, lock_script_hash: lock_script_hash }),
            (asset: (transfer_tracker, 1) => { asset_type: asset_type, quantity: 5, lock_script_hash: lock_script_hash }),
            (asset: (transfer_tracker, 2) => { asset_type: asset_type, quantity: 15, lock_script_hash: random_lock_script_hash })
        ]);
    }

    #[test]
    fn mint_and_transfer_allowed() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let random_lock_script_hash = H160::random();
        let allowed_script_hashes = vec![lock_script_hash, random_lock_script_hash];

        let amount = 30;
        let mint = asset_mint!(
            asset_mint_output!(lock_script_hash, supply: amount),
            metadata.clone(),
            allowed_script_hashes: allowed_script_hashes.clone()
        );
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));


        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, allowed_script_hashes: allowed_script_hashes}),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![
                (lock_script_hash, vec![vec![1]], asset_type, 10),
                (lock_script_hash, asset_type, 5),
                (random_lock_script_hash, asset_type, 15),
            ]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(Ok(()), state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, allowed_script_hashes: allowed_script_hashes}),
            (asset: (mint_tracker, 0)),
            (asset: (transfer_tracker, 0) => { asset_type: asset_type, quantity: 10, lock_script_hash: lock_script_hash }),
            (asset: (transfer_tracker, 1) => { asset_type: asset_type, quantity: 5, lock_script_hash: lock_script_hash }),
            (asset: (transfer_tracker, 2) => { asset_type: asset_type, quantity: 15, lock_script_hash: random_lock_script_hash })
        ]);
    }

    #[test]
    fn mint_and_transfer_not_allowed() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let allowed_lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let allowed_script_hashes = vec![allowed_lock_script_hash];
        let mint = asset_mint!(
            asset_mint_output!(lock_script_hash, supply: amount),
            metadata.clone(),
            allowed_script_hashes: allowed_script_hashes.clone()
        );
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, allowed_script_hashes: allowed_script_hashes}),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, asset_type, 30)]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(
            Err(StateError::Runtime(RuntimeError::ScriptNotAllowed(lock_script_hash))),
            state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0)
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, allowed_script_hashes: allowed_script_hashes}),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount }),
            (asset: (transfer_tracker, 0))
        ]);
    }

    #[test]
    fn mint_and_burn() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let burn = asset_transfer!(
            burns: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, amount), vec![0x01])]
        );

        let burn_tracker = burn.tracker();

        assert_eq!(Ok(()), state.apply(&burn, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: 0 }),
            (asset: (mint_tracker, 0)),
            (asset: (burn_tracker, 0))
        ]);
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn mint_and_transfer_and_burn() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let burn_amount = 5;
        let lock_script_hash_burn = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let random_lock_script_hash = H160::random();
        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![
                (lock_script_hash, vec![vec![1]], asset_type, 10),
                (lock_script_hash_burn, asset_type, burn_amount),
                (random_lock_script_hash, asset_type, 15),
            ]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(Ok(()), state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0)),
            (asset: (transfer_tracker, 0) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1) => { asset_type: asset_type, quantity: burn_amount }),
            (asset: (transfer_tracker, 2) => { asset_type: asset_type, quantity: 15 })
        ]);

        let burn = asset_transfer!(
            burns: asset_transfer_inputs![(asset_out_point!(transfer_tracker, 1, asset_type, 5), vec![0x01])]
        );
        let burn_tracker = burn.tracker();

        assert_eq!(Ok(()), state.apply(&burn, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount - burn_amount }),
            (asset: (mint_tracker, 0)),
            (asset: (transfer_tracker, 0) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1)),
            (asset: (transfer_tracker, 2) => { asset_type: asset_type, quantity: 15 }),
            (asset: (burn_tracker, 0))
        ]);
    }


    #[test]
    fn registrar_can_transfer() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let registrar = address();
        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint =
            asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone(), registrar: registrar);
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, registrar: registrar }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let lock_script_hash1 = H160::random();
        let lock_script_hash2 = H160::random();
        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30))],
            asset_transfer_outputs![
                (lock_script_hash, vec![vec![1]], asset_type, 10),
                (lock_script_hash1, asset_type, 5),
                (lock_script_hash2, asset_type, 15),
            ]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(Ok(()), state.apply(&transfer, &registrar, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, registrar: registrar }),
            (asset: (mint_tracker, 0)),
            (asset: (transfer_tracker, 0) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1) => { asset_type: asset_type, quantity: 5 }),
            (asset: (transfer_tracker, 2) => { asset_type: asset_type, quantity: 15 })
        ]);
    }


    #[test]
    fn registrar_can_burn() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let registrar = address();
        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint =
            asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone(), registrar: registrar);
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount, registrar: registrar }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let burn =
            asset_transfer!(burns: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, amount))]);
        let burn_tracker = burn.tracker();

        assert_eq!(Ok(()), state.apply(&burn, &registrar, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: 0, registrar: registrar }),
            (asset: (mint_tracker, 0)),
            (asset: (burn_tracker, 0))
        ]);
    }

    #[test]
    fn cannot_transfer_because_prev_out_amount_is_invalid() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 20), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, vec![vec![1]], asset_type, 20)]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(
            Err(StateError::Runtime(RuntimeError::InvalidAssetQuantity {
                shard_id: SHARD_ID,
                tracker: mint_tracker,
                index: 0,
                expected: 30,
                got: 20
            })),
            state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0)
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount }),
            (asset: (transfer_tracker, 0))
        ]);
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn cannot_transfer_because_prev_out_type_is_invalid() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;

        let metadata1 = "metadata".to_string();
        let mint1 = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata1.clone());
        let mint_tracker1 = mint1.tracker();
        let asset_type1 = Blake::blake(mint_tracker1);

        let metadata2 = "metadata2".to_string();
        let mint2 = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata2.clone());
        let mint_tracker2 = mint2.tracker();
        let asset_type2 = Blake::blake(mint_tracker2);

        assert_eq!(Ok(()), state.apply(&mint1, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type1) => { metadata: metadata1, supply: amount }),
            (scheme: (asset_type2)),
            (asset: (mint_tracker1, 0) => { asset_type: asset_type1, quantity: amount })
        ]);

        assert_eq!(Ok(()), state.apply(&mint2, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type1) => { metadata: metadata1, supply: amount }),
            (scheme: (asset_type2) => { metadata: metadata2, supply: amount }),
            (asset: (mint_tracker1, 0) => { asset_type: asset_type1, quantity: amount }),
            (asset: (mint_tracker2, 0) => { asset_type: asset_type2, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker1, 0, asset_type2, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, vec![vec![1]], asset_type2, 30)]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(
            Err(StateError::Runtime(RuntimeError::UnexpectedAssetType {
                index: 0,
                mismatch: Mismatch {
                    found: asset_type2,
                    expected: asset_type1,
                }
            })),
            state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0)
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type1) => { metadata: metadata1, supply: amount }),
            (scheme: (asset_type2) => { metadata: metadata2, supply: amount }),
            (asset: (mint_tracker1, 0) => { asset_type: asset_type1, quantity: amount }),
            (asset: (mint_tracker2, 0) => { asset_type: asset_type2, quantity: amount }),
            (asset: (transfer_tracker, 0))
        ]);
    }

    fn mint_for_transfer(state: &mut ShardLevelState, sender: Address, metadata: String, amount: u64) -> AssetOutPoint {
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);
        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        asset_out_point!(mint_tracker, 0, asset_type, amount)
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn mint_three_times_and_transfer_with_order() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let mint_output_1 = mint_for_transfer(&mut state, sender, "metadata1".to_string(), 30);
        let mint_output_2 = mint_for_transfer(&mut state, sender, "metadata2".to_string(), 30);
        let mint_output_3 = mint_for_transfer(&mut state, sender, "metadata3".to_string(), 30);
        let asset_type_1 = mint_output_1.asset_type;
        let asset_type_2 = mint_output_2.asset_type;
        let asset_type_3 = mint_output_3.asset_type;

        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let order = order!(from: (asset_type_1, 20), to: (asset_type_2, 10), fee: (asset_type_3, 20),
            [mint_output_1.clone(), mint_output_3.clone()],
            10,
            lock_script_hash
        );
        let order_consumed = order.consume(20);
        let order_consumed_hash = order_consumed.hash();

        let transfer = asset_transfer!(
            inputs:
                asset_transfer_inputs![
                    (mint_output_1.clone(), vec![0x30, 0x01]),
                    (mint_output_2.clone(), vec![0x30, 0x01]),
                    (mint_output_3.clone(), vec![0x30, 0x01]),
                ],
            asset_transfer_outputs![
                (lock_script_hash, asset_type_1, 10),
                (lock_script_hash, asset_type_2, 10),
                (lock_script_hash, asset_type_3, 10),
                (lock_script_hash, asset_type_1, 20),
                (lock_script_hash, asset_type_2, 20),
                (lock_script_hash, vec![vec![0x1]], asset_type_3, 20),
            ],
            vec![order_on_transfer! (
                order,
                20,
                input_from_indices: [0],
                input_fee_indices: [2],
                output_from_indices: [0],
                output_to_indices: [1],
                output_owned_fee_indices: [2],
                output_transferred_fee_indices: [5]
            )]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(Ok(()), state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type_1) => { metadata: "metadata1".to_string(), supply: 30 }),
            (scheme: (asset_type_2) => { metadata: "metadata2".to_string(), supply: 30 }),
            (scheme: (asset_type_3) => { metadata: "metadata3".to_string(), supply: 30 }),
            (asset: (mint_output_1.tracker, 0)),
            (asset: (mint_output_2.tracker, 0)),
            (asset: (mint_output_3.tracker, 0)),
            (asset: (transfer_tracker, 0) => { asset_type: asset_type_1, quantity: 10, order: order_consumed_hash }),
            (asset: (transfer_tracker, 1) => { asset_type: asset_type_2, quantity: 10, order: order_consumed_hash }),
            (asset: (transfer_tracker, 2) => { asset_type: asset_type_3, quantity: 10, order: order_consumed_hash }),
            (asset: (transfer_tracker, 3) => { asset_type: asset_type_1, quantity: 20, order }),
            (asset: (transfer_tracker, 4) => { asset_type: asset_type_2, quantity: 20, order }),
            (asset: (transfer_tracker, 5) => { asset_type: asset_type_3, quantity: 20, order: order_consumed_hash })
        ]);
    }

    #[test]
    fn wrap_and_unwrap_ccc() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let tx_hash = H256::random();
        let amount = 30;

        let wrap_ccc = asset_wrap_ccc!(tx_hash, asset_wrap_ccc_output!(lock_script_hash, amount));
        let wrap_ccc_tracker = wrap_ccc.tracker();
        let asset_type = H160::zero();

        assert_eq!(wrap_ccc_tracker, tx_hash);
        assert_eq!(Ok(()), state.apply(&wrap_ccc, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { supply: amount }),
            (asset: (wrap_ccc_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let unwrap_amount = 30;
        let unwrap_ccc = asset_unwrap_ccc!(
            asset_transfer_input!(asset_out_point!(wrap_ccc_tracker, 0, asset_type, unwrap_amount), vec![0x01]),
            sender
        );

        assert_eq!(Ok(()), state.apply(&unwrap_ccc, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { supply: amount - unwrap_amount }),
            (asset: (wrap_ccc_tracker, 0))
        ]);
    }

    #[test]
    fn wrap_ccc_and_transfer_and_unwrap_ccc() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let tx_hash = H256::random();
        let amount = 30;

        let wrap_ccc = asset_wrap_ccc!(tx_hash, asset_wrap_ccc_output!(lock_script_hash, amount));
        let wrap_ccc_tracker = wrap_ccc.tracker();

        assert_eq!(wrap_ccc_tracker, tx_hash);
        assert_eq!(Ok(()), state.apply(&wrap_ccc, &sender, &[sender], &[], &get_test_client(), 0, 0));

        let asset_type = H160::zero();

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { supply: amount }),
            (asset: (wrap_ccc_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let lock_script_hash_burn = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let random_lock_script_hash = H160::random();
        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(wrap_ccc_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![
                (lock_script_hash, vec![vec![1]], asset_type, 10),
                (lock_script_hash_burn, asset_type, 5),
                (random_lock_script_hash, asset_type, 15),
            ]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(Ok(()), state.apply(&transfer, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { supply: amount }),
            (asset: (wrap_ccc_tracker, 0)),
            (asset: (transfer_tracker, 0) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1) => { asset_type: asset_type, quantity: 5 }),
            (asset: (transfer_tracker, 2) => { asset_type: asset_type, quantity: 15 })
        ]);

        let unwrap_amount = 5;
        let unwrap_ccc = asset_unwrap_ccc!(
            asset_transfer_input!(asset_out_point!(transfer_tracker, 1, asset_type, unwrap_amount), vec![0x01]),
            sender
        );

        assert_eq!(Ok(()), state.apply(&unwrap_ccc, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { supply: amount - unwrap_amount }),
            (asset: (wrap_ccc_tracker, 0)),
            (asset: (transfer_tracker, 0) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1)),
            (asset: (transfer_tracker, 2) => { asset_type: asset_type, quantity: 15 })
        ]);
    }

    #[test]
    fn mint_and_failed_transfer_and_successful_transfer() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let failed_lock_script = vec![0x30];
        let failed_transfer = asset_transfer!(
            inputs:
                asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), failed_lock_script.clone())],
            asset_transfer_outputs![(lock_script_hash, vec![vec![1]], asset_type, 30)]
        );
        let failed_transfer_tracker = failed_transfer.tracker();

        let sender = address();
        assert_eq!(
            Err(StateError::Runtime(RuntimeError::ScriptHashMismatch(Mismatch {
                expected: lock_script_hash,
                found: Blake::blake(&failed_lock_script),
            }))),
            state.apply(&failed_transfer, &sender, &[sender], &[], &get_test_client(), 0, 0)
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount }),
            (asset: (failed_transfer_tracker, 0))
        ]);

        let random_lock_script_hash = H160::random();
        let successful_transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![
                (lock_script_hash, vec![vec![1]], asset_type, 10),
                (lock_script_hash, asset_type, 5),
                (random_lock_script_hash, asset_type, 15),
            ]
        );
        let successful_transfer_tracker = successful_transfer.tracker();

        assert_eq!(Ok(()), state.apply(&successful_transfer, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0)),
            (asset: (failed_transfer_tracker, 0)),
            (asset: (successful_transfer_tracker, 0) => { asset_type: asset_type, quantity: 10 }),
            (asset: (successful_transfer_tracker, 1) => { asset_type: asset_type, quantity: 5 }),
            (asset: (successful_transfer_tracker, 2) => { asset_type: asset_type, quantity: 15 })
        ]);
    }

    #[test]
    fn users_can_mint_asset() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters: parameters.clone()),
            metadata.clone(),
            approver: approver
        );
        let transaction_tracker = transaction.tracker();
        let asset_type = Blake::blake(transaction_tracker);

        assert_eq!(Ok(()), state.apply(&transaction, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: ::std::u64::MAX }),
            (asset: (transaction_tracker, 0) => { asset_type: asset_type, quantity: ::std::u64::MAX })
        ]);
    }

    #[test]
    fn mint_is_failed_when_shard_users_are_disjoint_to_sender_and_approvers() {
        let shard_users = [address(), address(), address()];
        let sender = address();
        let approvers = [address(), address(), address()];

        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters: parameters.clone()),
            metadata.clone(),
            approver: approver
        );

        let transaction_tracker = transaction.tracker();
        let asset_type = Blake::blake(transaction_tracker);

        assert_eq!(
            Err(StateError::Runtime(RuntimeError::InsufficientPermission)),
            state.apply(&transaction, &sender, &shard_users, &approvers, &get_test_client(), 0, 0)
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type)),
            (asset: (transaction_tracker, 0))
        ]);
    }

    #[test]
    fn can_mint_when_shard_user_sent_a_transaction() {
        let shard_users = [address(), address(), address()];
        let sender = shard_users[0];
        let approvers = [address(), address(), address()];

        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters: parameters.clone()),
            metadata.clone(),
            approver: approver
        );

        let transaction_tracker = transaction.tracker();
        let asset_type = Blake::blake(transaction_tracker);

        check_shard_level_state!(state, [
            (scheme: (asset_type)),
            (asset: (transaction_tracker, 0))
        ]);

        assert_eq!(Ok(()), state.apply(&transaction, &sender, &shard_users, &approvers, &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: ::std::u64::MAX }),
            (asset: (transaction_tracker, 0) => { asset_type: asset_type, quantity: ::std::u64::MAX })
        ]);
    }

    #[test]
    fn can_mint_when_shard_user_approves_a_transaction() {
        let shard_users = [address(), address(), address()];
        let sender = address();
        let approvers = [shard_users[0], address(), address()];

        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters: parameters.clone()),
            metadata.clone(),
            approver: approver
        );

        let transaction_tracker = transaction.tracker();
        let asset_type = Blake::blake(transaction_tracker);

        check_shard_level_state!(state, [
            (scheme: (asset_type)),
            (asset: (transaction_tracker, 0))
        ]);

        assert_eq!(Ok(()), state.apply(&transaction, &sender, &shard_users, &approvers, &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: ::std::u64::MAX }),
            (asset: (transaction_tracker, 0) => { asset_type: asset_type, quantity: ::std::u64::MAX })
        ]);
    }

    #[test]
    fn anyone_can_mint_if_no_users() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Address::random();
        let transaction = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters: parameters.clone()),
            metadata.clone(),
            approver: approver
        );

        let transaction_tracker = transaction.tracker();
        let asset_type = Blake::blake(transaction_tracker);

        assert_eq!(Ok(()), state.apply(&transaction, &sender, &[], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: ::std::u64::MAX, approver: approver }),
            (asset: (transaction_tracker, 0) => { asset_type: asset_type, quantity: ::std::u64::MAX })
        ]);
    }

    #[test]
    fn change_asset_scheme() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let amount = 100;
        let registrar = Address::random();
        let mint = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters.clone(), amount),
            metadata.clone(),
            registrar: registrar
        );

        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: amount, approver, registrar: registrar }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let approver = Address::random();
        let change_asset_scheme = ShardTransaction::ChangeAssetScheme {
            network_id: "tc".into(),
            shard_id: SHARD_ID,
            asset_type,
            seq: 0,
            metadata: "New metadata".to_string(),
            approver: Some(approver),
            registrar: None,
            allowed_script_hashes: Vec::new(),
        };
        assert_eq!(Ok(()), state.apply(&change_asset_scheme, &sender, &[], &[registrar], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: "New metadata".to_string(), supply: amount, approver: approver, registrar }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);
    }

    #[test]
    fn increase_asset_amount() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let amount = 100;
        let registrar = Address::random();
        let mint = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters.clone(), amount),
            metadata.clone(),
            registrar: registrar
        );

        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(()), state.apply(&mint, &sender, &[sender], &[], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: metadata.clone(), supply: amount, approver, registrar: registrar }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount })
        ]);

        let new_supply = 200;
        let increase_supply = ShardTransaction::IncreaseAssetSupply {
            network_id: "tc".into(),
            shard_id: SHARD_ID,
            asset_type,
            seq: 0,
            output: AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                supply: new_supply,
            },
        };
        let supply_tracker = increase_supply.tracker();

        assert_eq!(Ok(()), state.apply(&increase_supply, &sender, &[], &[registrar], &get_test_client(), 0, 0));

        check_shard_level_state!(state, [
            (scheme: (asset_type) => { metadata: "metadata".to_string(), supply: amount + new_supply, approver, registrar: registrar }),
            (asset: (mint_tracker, 0) => { asset_type: asset_type, quantity: amount }),
            (asset: (supply_tracker, 0) => { asset_type: asset_type, quantity: new_supply })
        ]);
    }
}
