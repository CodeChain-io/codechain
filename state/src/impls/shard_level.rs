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
use std::collections::HashMap;

use ccrypto::{Blake, BLAKE_NULL_RLP};
use ckey::Address;
use cmerkle::{self, TrieError, TrieFactory};
use ctypes::errors::{RuntimeError, UnlockFailureReason};
use ctypes::invoice::Invoice;
use ctypes::transaction::{
    AssetMintOutput, AssetTransferInput, AssetTransferOutput, AssetWrapCCCOutput, Order, OrderOnTransfer,
    PartialHashing, ShardTransaction,
};
use ctypes::util::unexpected::Mismatch;
use ctypes::ShardId;
use cvm::{decode, execute, ChainTimeInfo, ScriptResult, VMConfig};
use hashdb::AsHashDB;
use primitives::{Bytes, H160, H256};

use crate::cache::ShardCache;
use crate::checkpoint::{CheckpointId, StateWithCheckpoint};
use crate::traits::{ShardState, ShardStateView};
use crate::{Asset, AssetScheme, AssetSchemeAddress, OwnedAsset, OwnedAssetAddress, StateDB, StateError, StateResult};


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
    pub fn read_only(db: &RefCell<StateDB>, root: H256, cache: ShardCache) -> cmerkle::Result<ReadOnlyShardLevelState> {
        if !db.borrow().as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        Ok(ReadOnlyShardLevelState {
            db,
            root,
            cache,
        })
    }

    fn apply_internal<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        sender: &Address,
        shard_users: &[Address],
        approvers: &[Address],
        client: &C,
    ) -> StateResult<()> {
        match transaction {
            ShardTransaction::MintAsset {
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        supply,
                        parameters,
                    },
                ..
            } => {
                self.mint_asset(
                    transaction.tracker(),
                    metadata,
                    lock_script_hash,
                    &parameters,
                    supply,
                    approver,
                    administrator,
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
                self.transfer_asset(&transaction, sender, approvers, burns, inputs, outputs, orders, client)
            }
            ShardTransaction::ChangeAssetScheme {
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                ..
            } => self.change_asset_scheme(
                sender,
                approvers,
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
            ),
            ShardTransaction::ComposeAsset {
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output,
                ..
            } => self.compose_asset(
                &transaction,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output,
                sender,
                approvers,
                shard_users,
                client,
            ),
            ShardTransaction::DecomposeAsset {
                input,
                outputs,
                ..
            } => self.decompose_asset(&transaction, input, outputs, sender, approvers, client),
            ShardTransaction::UnwrapCCC {
                burn,
                ..
            } => self.unwrap_ccc(&transaction, sender, burn, client),
            ShardTransaction::WrapCCC {
                tx_hash,
                output:
                    AssetWrapCCCOutput {
                        lock_script_hash,
                        quantity,
                        parameters,
                    },
                ..
            } => self.wrap_ccc(tx_hash, lock_script_hash, &parameters, *quantity),
        }
    }

    // FIXME: Remove this clippy config
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::too_many_arguments))]
    fn mint_asset(
        &mut self,
        transaction_tracker: H256,
        metadata: &str,
        lock_script_hash: &H160,
        parameters: &[Bytes],
        supply: &Option<u64>,
        approver: &Option<Address>,
        administrator: &Option<Address>,
        allowed_script_hashes: &[H160],
        sender: &Address,
        shard_users: &[Address],
        pool: Vec<Asset>,
    ) -> StateResult<()> {
        if !shard_users.is_empty() && !shard_users.contains(sender) {
            return Err(RuntimeError::InsufficientPermission.into())
        }

        let asset_type = Blake::blake(transaction_tracker);
        let asset_scheme_address = AssetSchemeAddress::new(asset_type, self.shard_id);
        if self.asset_scheme(&asset_scheme_address)?.is_some() {
            return Err(RuntimeError::AssetSchemeDuplicated(transaction_tracker).into())
        }
        let supply = supply.unwrap_or(::std::u64::MAX);
        let asset_scheme = self.create_asset_scheme(
            &asset_scheme_address,
            metadata.to_string(),
            supply,
            *approver,
            *administrator,
            allowed_script_hashes.to_vec(),
            pool,
        )?;

        ctrace!(TX, "{:?} is minted on {:?}", asset_scheme, asset_scheme_address);

        let asset_address = OwnedAssetAddress::new(transaction_tracker, 0, self.shard_id);
        let asset =
            self.create_asset(&asset_address, asset_type, *lock_script_hash, parameters.to_vec(), supply, None)?;
        ctrace!(TX, "{:?} is generated on {:?}", asset, asset_address);
        Ok(())
    }

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
    ) -> StateResult<()> {
        let mut values_to_hash = vec![None; inputs.len()];
        for order_tx in orders {
            let order = &order_tx.order;
            for input_idx in order_tx.input_indices.iter() {
                values_to_hash[*input_idx] = Some(order);
            }
        }

        for (input, transaction, order, burn) in inputs
            .iter()
            .enumerate()
            .map(|(index, input)| (input, transaction, values_to_hash[index], false))
            .chain(burns.iter().map(|input| (input, transaction, None, true)))
        {
            self.check_and_run_input_script(input, transaction, order, burn, sender, approvers, client)?;
        }

        self.check_orders(orders, inputs)?;
        let mut output_order_hashes = vec![None; outputs.len()];
        for order_tx in orders {
            let order = &order_tx.order;
            for output_idx in order_tx.output_indices.iter() {
                output_order_hashes[*output_idx] = Some(order.consume(order_tx.spent_quantity).hash());
            }
        }

        let mut deleted_asset = Vec::with_capacity(inputs.len() + burns.len());
        for input in inputs.iter().chain(burns) {
            let (_, asset_address) = self.check_input_asset(input, sender, approvers)?;
            self.kill_asset(&asset_address);
            deleted_asset.push((asset_address, input.prev_out.quantity));
        }
        let mut created_asset = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.iter().enumerate() {
            self.check_output_script_hash(output)?;

            let asset_address = OwnedAssetAddress::new(transaction.tracker(), index, self.shard_id);
            let _asset = self.create_asset(
                &asset_address,
                output.asset_type,
                output.lock_script_hash,
                output.parameters.clone(),
                output.quantity,
                output_order_hashes[index],
            )?;
            created_asset.push((asset_address, output.quantity));
        }
        ctrace!(TX, "Deleted assets {:?}", deleted_asset);
        ctrace!(TX, "Created assets {:?}", created_asset);
        Ok(())
    }

    fn check_orders(&self, orders: &[OrderOnTransfer], inputs: &[AssetTransferInput]) -> StateResult<()> {
        for order_tx in orders {
            let order = &order_tx.order;
            let mut counter: usize = 0;
            for input_idx in order_tx.input_indices.iter() {
                let input = &inputs[*input_idx];
                let transaction_tracker = input.prev_out.tracker;
                let index = input.prev_out.index;
                let address = OwnedAssetAddress::new(transaction_tracker, index, self.shard_id);
                let asset = self.asset(&address)?.ok_or_else(|| RuntimeError::AssetNotFound(address.into()))?;

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

    fn change_asset_scheme(
        &mut self,
        sender: &Address,
        approvers: &[Address],
        asset_type: &H160,
        metadata: &str,
        approver: &Option<Address>,
        administrator: &Option<Address>,
        allowed_script_hashes: &[H160],
    ) -> StateResult<()> {
        let asset_scheme_address = AssetSchemeAddress::new(*asset_type, self.shard_id);
        {
            let asset_scheme = self
                .asset_scheme(&asset_scheme_address)?
                .ok_or_else(|| RuntimeError::AssetSchemeNotFound(asset_scheme_address.into()))?;

            if !asset_scheme.is_centralized() {
                return Err(RuntimeError::InsufficientPermission.into())
            }
            let administrator = asset_scheme.administrator().as_ref().expect("Centralized asset has administrator");
            if administrator != sender && !approvers.contains(administrator) {
                return Err(RuntimeError::InsufficientPermission.into())
            }
        }
        let mut asset_scheme = self.get_asset_scheme_mut(&asset_scheme_address)?;
        asset_scheme.change_data(
            metadata.to_string(),
            approver.clone(),
            administrator.clone(),
            allowed_script_hashes.to_vec(),
        );

        Ok(())
    }

    fn check_input_asset(
        &self,
        input: &AssetTransferInput,
        sender: &Address,
        approvers: &[Address],
    ) -> StateResult<(OwnedAsset, OwnedAssetAddress)> {
        let asset_address = OwnedAssetAddress::new(input.prev_out.tracker, input.prev_out.index, self.shard_id);
        let asset_scheme_address = AssetSchemeAddress::new(input.prev_out.asset_type, input.prev_out.shard_id);

        let asset_scheme = self
            .asset_scheme(&asset_scheme_address)?
            .ok_or_else(|| RuntimeError::AssetSchemeNotFound(asset_scheme_address.into()))?;

        if let Some(approver) = asset_scheme.approver().as_ref() {
            if sender != approver && !approvers.contains(approver) {
                return Err(RuntimeError::NotApproved(*approver).into())
            }
        }

        match self.asset(&asset_address)? {
            Some(asset) => {
                if asset.quantity() != input.prev_out.quantity {
                    return Err(RuntimeError::InvalidAssetQuantity {
                        address: asset_address.into(),
                        expected: asset.quantity(),
                        got: input.prev_out.quantity,
                    }
                    .into())
                }
                if *asset.asset_type() != input.prev_out.asset_type {
                    return Err(RuntimeError::InvalidAssetType(input.prev_out.asset_type).into())
                }
                Ok((asset, asset_address))
            }
            None => Err(RuntimeError::AssetNotFound(asset_address.into()).into()),
        }
    }

    fn check_output_script_hash(&self, output: &AssetTransferOutput) -> StateResult<()> {
        let asset_scheme = {
            let asset_scheme_address = AssetSchemeAddress::new(output.asset_type, output.shard_id);
            self.asset_scheme(&asset_scheme_address)?.expect("AssetScheme must exist when the asset exist")
        };
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
    ) -> StateResult<()> {
        debug_assert!(!burn || order.is_none());

        let (address, asset) = {
            let index = input.prev_out.index;
            let address = OwnedAssetAddress::new(input.prev_out.tracker, index, self.shard_id);
            match self.asset(&address)? {
                Some(asset) => (address.into(), asset),
                None => return Err(RuntimeError::AssetNotFound(address.into()).into()),
            }
        };
        let asset_scheme = {
            let asset_scheme_address = AssetSchemeAddress::new(input.prev_out.asset_type, input.prev_out.shard_id);
            self.asset_scheme(&asset_scheme_address)?.expect("AssetScheme must exist when the asset exist")
        };
        if asset_scheme.is_centralized() {
            let administrator = asset_scheme.administrator().as_ref().expect("Centralized asset has administrator");
            if administrator == sender || approvers.contains(administrator) {
                return Ok(())
            } else if burn {
                // Only the administrator can burn the centralized asset
                return Err(RuntimeError::CannotBurnCentralizedAsset.into())
            }
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
                address,
                reason,
            }
            .into()
        })
    }

    // FIXME: Remove this clippy config
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::too_many_arguments))]
    fn compose_asset<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        metadata: &str,
        approver: &Option<Address>,
        administrator: &Option<Address>,
        allowed_script_hashes: &[H160],
        inputs: &[AssetTransferInput],
        output: &AssetMintOutput,
        sender: &Address,
        approvers: &[Address],
        shard_users: &[Address],
        client: &C,
    ) -> StateResult<()> {
        let mut sum: HashMap<(H160, ShardId), u64> = HashMap::new();

        let mut deleted_assets: Vec<(H256, _)> = Vec::with_capacity(inputs.len());
        for input in inputs.iter() {
            let (_, asset_address) = self.check_input_asset(input, sender, approvers)?;
            self.check_and_run_input_script(input, transaction, None, false, sender, approvers, client)?;

            let asset_scheme_address = AssetSchemeAddress::new(input.prev_out.asset_type, input.prev_out.shard_id);
            let shard_asset_type = (input.prev_out.asset_type, input.prev_out.shard_id);
            let asset_scheme =
                self.asset_scheme(&asset_scheme_address)?.expect("AssetScheme must exist when the asset exist");
            if asset_scheme.is_centralized() {
                return Err(RuntimeError::CannotComposeCentralizedAsset.into())
            }

            self.kill_asset(&asset_address);
            deleted_assets.push((asset_address.into(), input.prev_out.quantity));

            *sum.entry(shard_asset_type).or_insert_with(Default::default) += input.prev_out.quantity;
        }
        ctrace!(TX, "Deleted assets {:?}", deleted_assets);

        let pool = sum.into_iter().map(|((asset_type, _), quantity)| Asset::new(asset_type, quantity)).collect();

        self.mint_asset(
            transaction.tracker(),
            metadata,
            &output.lock_script_hash,
            &output.parameters,
            &output.supply,
            approver,
            administrator,
            allowed_script_hashes,
            sender,
            shard_users,
            pool,
        )
    }

    fn decompose_asset<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        input: &AssetTransferInput,
        outputs: &[AssetTransferOutput],
        sender: &Address,
        approvers: &[Address],
        client: &C,
    ) -> StateResult<()> {
        let asset_type = input.prev_out.asset_type;
        let shard_id = input.prev_out.shard_id;
        let asset_scheme_address = AssetSchemeAddress::new(asset_type, shard_id);
        let asset_scheme = self
            .asset_scheme(&asset_scheme_address)?
            .ok_or_else(|| RuntimeError::AssetSchemeNotFound(asset_scheme_address.into()))?;
        // The input asset should be composed asset
        if asset_scheme.pool().is_empty() {
            return Err(RuntimeError::InvalidDecomposedInput {
                asset_type,
                shard_id,
                got: 0,
            }
            .into())
        }

        // Check that the outputs are match with pool
        let mut sum: HashMap<H160, u64> = HashMap::new();
        for output in outputs {
            let output_type = output.asset_type;

            *sum.entry(output_type).or_insert_with(Default::default) += output.quantity;
        }
        for asset in asset_scheme.pool() {
            let asset_type = asset.asset_type();
            match sum.remove(asset_type) {
                None => {
                    return Err(RuntimeError::InvalidDecomposedOutput {
                        asset_type: *asset_type,
                        shard_id: self.shard_id,
                        expected: asset.quantity(),
                        got: 0,
                    }
                    .into())
                }
                Some(value) => {
                    if value != asset.quantity() {
                        return Err(RuntimeError::InvalidDecomposedOutput {
                            asset_type: *asset_type,
                            shard_id: self.shard_id,
                            expected: asset.quantity(),
                            got: value,
                        }
                        .into())
                    }
                }
            }
        }
        if !sum.is_empty() {
            let mut invalid_assets: Vec<Asset> =
                sum.into_iter().map(|(asset_type, quantity)| Asset::new(asset_type, quantity)).collect();
            let invalid_asset = invalid_assets.pop().unwrap();
            return Err(RuntimeError::InvalidDecomposedOutput {
                asset_type: *invalid_asset.asset_type(),
                shard_id: self.shard_id,
                expected: 0,
                got: invalid_asset.quantity(),
            }
            .into())
        }

        let (_, asset_address) = self.check_input_asset(input, sender, approvers)?;
        self.check_and_run_input_script(input, transaction, None, false, sender, approvers, client)?;

        self.kill_asset(&asset_address);

        ctrace!(TX, "Deleted assets {:?} {:?}", asset_type, input.prev_out.quantity);

        // Put asset into DB
        for (index, output) in outputs.iter().enumerate() {
            let asset_address = OwnedAssetAddress::new(transaction.tracker(), index, self.shard_id);
            let _asset = self.create_asset(
                &asset_address,
                output.asset_type,
                output.lock_script_hash,
                output.parameters.clone(),
                output.quantity,
                None,
            )?;
        }

        Ok(())
    }

    fn wrap_ccc(
        &mut self,
        tx_hash: &H256,
        lock_script_hash: &H160,
        parameters: &[Bytes],
        quantity: u64,
    ) -> StateResult<()> {
        let asset_type = H160::zero();
        let asset_scheme_address = AssetSchemeAddress::new(asset_type, self.shard_id);
        if self.asset_scheme(&asset_scheme_address)?.is_none() {
            let asset_scheme = self.create_asset_scheme(
                &asset_scheme_address,
                format!("{{\"name\":\"Wrapped CCC\",\"description\":\"Wrapped CCC in shard {}\"}}", self.shard_id),
                ::std::u64::MAX,
                None,
                None,
                Vec::new(),
                Vec::new(),
            );
            // FIXME: Wrapped CCC is minted in here, but the metadata is not well-defined.
            ctrace!(
                TX,
                "Wrapped CCC in shard {} ({:?}) is minted on {:?}",
                self.shard_id,
                asset_scheme,
                asset_scheme_address
            );
        }

        let asset_address = OwnedAssetAddress::new(*tx_hash, 0, self.shard_id);
        let asset =
            self.create_asset(&asset_address, asset_type, *lock_script_hash, parameters.to_vec(), quantity, None)?;
        ctrace!(TX, "Created Wrapped CCC {:?} on {:?}", asset, asset_address);
        Ok(())
    }

    fn unwrap_ccc<C: ChainTimeInfo>(
        &mut self,
        transaction: &ShardTransaction,
        sender: &Address,
        burn: &AssetTransferInput,
        client: &C,
    ) -> StateResult<()> {
        // WCCC has no approvers
        let approvers = [];
        self.check_and_run_input_script(burn, transaction, None, true, sender, &approvers, client)?;

        let (_, asset_address) = self.check_input_asset(burn, sender, &approvers)?;
        self.kill_asset(&asset_address);
        ctrace!(TX, "Removed Wrapped CCC asset {:?}, quantity {:?}", asset_address, burn.prev_out.quantity);
        Ok(())
    }

    fn kill_asset(&mut self, account: &OwnedAssetAddress) {
        self.cache.remove_asset(account);
    }

    pub fn create_asset_scheme(
        &self,
        a: &AssetSchemeAddress,
        metadata: String,
        supply: u64,
        approver: Option<Address>,
        administrator: Option<Address>,
        allowed_script_hashes: Vec<H160>,
        pool: Vec<Asset>,
    ) -> cmerkle::Result<AssetScheme> {
        let mut asset_scheme = self.get_asset_scheme_mut(a)?;
        asset_scheme.init(metadata, supply, approver, administrator, allowed_script_hashes, pool);
        Ok(asset_scheme.clone())
    }

    fn get_asset_scheme_mut(&self, a: &AssetSchemeAddress) -> cmerkle::Result<RefMut<AssetScheme>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset_scheme_mut(a, &trie)
    }

    fn get_asset_mut(&self, a: &OwnedAssetAddress) -> cmerkle::Result<RefMut<OwnedAsset>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset_mut(a, &trie)
    }

    pub fn create_asset(
        &self,
        a: &OwnedAssetAddress,
        asset_type: H160,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        quantity: u64,
        order_hash: Option<H256>,
    ) -> cmerkle::Result<OwnedAsset> {
        let mut asset = self.get_asset_mut(a)?;
        asset.init(asset_type, lock_script_hash, parameters, quantity, order_hash);
        Ok(asset.clone())
    }
}

impl<'db> ShardStateView for ShardLevelState<'db> {
    fn asset_scheme(&self, a: &AssetSchemeAddress) -> cmerkle::Result<Option<AssetScheme>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset_scheme(a, &trie)
    }

    fn asset(&self, a: &OwnedAssetAddress) -> cmerkle::Result<Option<OwnedAsset>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset(a, &trie)
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
    ) -> StateResult<Invoice> {
        ctrace!(TX, "Execute InnerTx {:?}(InnerTxHash:{:?})", transaction, transaction.tracker());

        self.create_checkpoint(TRANSACTION_CHECKPOINT);
        let result = self.apply_internal(transaction, sender, shard_users, approvers, client);
        match result {
            Ok(_) => {
                cinfo!(TX, "InnerTx({}) is applied", transaction.tracker());
                self.discard_checkpoint(TRANSACTION_CHECKPOINT);
                Ok(Invoice::Success)
            }
            Err(StateError::Runtime(err)) => {
                cinfo!(TX, "Cannot apply InnerTx({}): {:?}", transaction.tracker(), err);
                self.revert_to_checkpoint(TRANSACTION_CHECKPOINT);
                Ok(Invoice::Failure(err))
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
}

impl<'db> ShardStateView for ReadOnlyShardLevelState<'db> {
    fn asset_scheme(&self, a: &AssetSchemeAddress) -> cmerkle::Result<Option<AssetScheme>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset_scheme(a, &trie)
    }

    fn asset(&self, a: &OwnedAssetAddress) -> cmerkle::Result<Option<OwnedAsset>> {
        let db = self.db.borrow();
        let trie = TrieFactory::readonly(db.as_hashdb(), &self.root)?;
        self.cache.asset(a, &trie)
    }
}

#[cfg(test)]
mod tests {
    use ctypes::transaction::AssetOutPoint;

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
        assert_eq!(Ok(Invoice::Success), state.apply(&transaction.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, approver: approver }),
            (asset: (transaction_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&transaction.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: ::std::u64::MAX, approver: approver }),
            (asset: (transaction_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: ::std::u64::MAX })
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
        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&transaction.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        assert_eq!(
            Ok(Invoice::Failure(RuntimeError::AssetSchemeDuplicated(transaction_tracker).into())),
            state.apply(&transaction.into(), &sender, &[sender], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: ::std::u64::MAX, approver: approver }),
            (asset: (transaction_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: ::std::u64::MAX })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, approver: approver }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, asset_type, 30)]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(
            Ok(Invoice::Failure(RuntimeError::NotApproved(approver).into())),
            state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, approver: approver }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount }),
            (asset: (transfer_tracker, 0, SHARD_ID))
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (asset: (transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: 10, lock_script_hash: lock_script_hash }),
            (asset: (transfer_tracker, 1, SHARD_ID) => { asset_type: asset_type, quantity: 5, lock_script_hash: lock_script_hash }),
            (asset: (transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type, quantity: 15, lock_script_hash: random_lock_script_hash })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));


        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, allowed_script_hashes: allowed_script_hashes}),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, allowed_script_hashes: allowed_script_hashes}),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (asset: (transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: 10, lock_script_hash: lock_script_hash }),
            (asset: (transfer_tracker, 1, SHARD_ID) => { asset_type: asset_type, quantity: 5, lock_script_hash: lock_script_hash }),
            (asset: (transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type, quantity: 15, lock_script_hash: random_lock_script_hash })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, allowed_script_hashes: allowed_script_hashes}),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, asset_type, 30)]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(
            Ok(Invoice::Failure(RuntimeError::ScriptNotAllowed(lock_script_hash).into())),
            state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, allowed_script_hashes: allowed_script_hashes}),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount }),
            (asset: (transfer_tracker, 0, SHARD_ID))
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let burn = asset_transfer!(
            burns: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, amount), vec![0x01])]
        );

        let burn_tracker = burn.tracker();

        assert_eq!(Ok(Invoice::Success), state.apply(&burn.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (asset: (burn_tracker, 0, SHARD_ID))
        ]);
    }

    #[test]
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let lock_script_hash_burn = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let random_lock_script_hash = H160::random();
        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![
                (lock_script_hash, vec![vec![1]], asset_type, 10),
                (lock_script_hash_burn, asset_type, 5),
                (random_lock_script_hash, asset_type, 15),
            ]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(Ok(Invoice::Success), state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (asset: (transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1, SHARD_ID) => { asset_type: asset_type, quantity: 5 }),
            (asset: (transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type, quantity: 15 })
        ]);

        let burn = asset_transfer!(
            burns: asset_transfer_inputs![(asset_out_point!(transfer_tracker, 1, asset_type, 5), vec![0x01])]
        );
        let burn_tracker = burn.tracker();

        assert_eq!(Ok(Invoice::Success), state.apply(&burn.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (asset: (transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1, SHARD_ID)),
            (asset: (transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type, quantity: 15 }),
            (asset: (burn_tracker, 0, SHARD_ID))
        ]);
    }


    #[test]
    fn administrator_can_transfer() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let administrator = address();
        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(
            asset_mint_output!(lock_script_hash, supply: amount),
            metadata.clone(),
            administrator: administrator
        );
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, administrator: administrator }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
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

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&transfer.into(), &administrator, &[sender], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, administrator: administrator }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (asset: (transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1, SHARD_ID) => { asset_type: asset_type, quantity: 5 }),
            (asset: (transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type, quantity: 15 })
        ]);
    }


    #[test]
    fn administrator_can_burn() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let administrator = address();
        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(
            asset_mint_output!(lock_script_hash, supply: amount),
            metadata.clone(),
            administrator: administrator
        );
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, administrator: administrator }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let burn = asset_transfer!(burns: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30))]);
        let burn_tracker = burn.tracker();

        assert_eq!(Ok(Invoice::Success), state.apply(&burn.into(), &administrator, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount, administrator: administrator }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (asset: (burn_tracker, 0, SHARD_ID))
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 20), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, vec![vec![1]], asset_type, 20)]
        );
        let transfer_tracker = transfer.tracker();

        let asset_address = OwnedAssetAddress::new(mint_tracker, 0, SHARD_ID).into();

        assert_eq!(
            Ok(Invoice::Failure(
                RuntimeError::InvalidAssetQuantity {
                    address: asset_address,
                    expected: 30,
                    got: 20
                }
                .into()
            )),
            state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata, supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount }),
            (asset: (transfer_tracker, 0, SHARD_ID))
        ]);
    }

    #[test]
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint1.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type1, SHARD_ID) => { metadata: metadata1, supply: amount }),
            (scheme: (asset_type2, SHARD_ID)),
            (asset: (mint_tracker1, 0, SHARD_ID) => { asset_type: asset_type1, quantity: amount })
        ]);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint2.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type1, SHARD_ID) => { metadata: metadata1, supply: amount }),
            (scheme: (asset_type2, SHARD_ID) => { metadata: metadata2, supply: amount }),
            (asset: (mint_tracker1, 0, SHARD_ID) => { asset_type: asset_type1, quantity: amount }),
            (asset: (mint_tracker2, 0, SHARD_ID) => { asset_type: asset_type2, quantity: amount })
        ]);

        let transfer = asset_transfer!(
            inputs: asset_transfer_inputs![(asset_out_point!(mint_tracker1, 0, asset_type2, 30), vec![0x30, 0x01])],
            asset_transfer_outputs![(lock_script_hash, vec![vec![1]], asset_type2, 30)]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(
            Ok(Invoice::Failure(RuntimeError::InvalidAssetType(asset_type2).into())),
            state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type1, SHARD_ID) => { metadata: metadata1, supply: amount }),
            (scheme: (asset_type2, SHARD_ID) => { metadata: metadata2, supply: amount }),
            (asset: (mint_tracker1, 0, SHARD_ID) => { asset_type: asset_type1, quantity: amount }),
            (asset: (mint_tracker2, 0, SHARD_ID) => { asset_type: asset_type2, quantity: amount }),
            (asset: (transfer_tracker, 0, SHARD_ID))
        ]);
    }

    fn mint_for_transfer(
        state: &mut ShardLevelState,
        shard_id: u16,
        sender: Address,
        metadata: String,
        amount: u64,
    ) -> AssetOutPoint {
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);
        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, shard_id) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, shard_id) => { asset_type: asset_type, quantity: amount })
        ]);

        asset_out_point!(mint_tracker, 0, asset_type, amount)
    }

    #[test]
    fn mint_three_times_and_transfer_with_order() {
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);

        let mint_output_1 = mint_for_transfer(&mut state, SHARD_ID, sender, "metadata1".to_string(), 30);
        let mint_output_2 = mint_for_transfer(&mut state, SHARD_ID, sender, "metadata2".to_string(), 30);
        let mint_output_3 = mint_for_transfer(&mut state, SHARD_ID, sender, "metadata3".to_string(), 30);
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
                input_indices: [0, 2],
                output_indices: [0, 1, 2, 5]
            )]
        );
        let transfer_tracker = transfer.tracker();

        assert_eq!(Ok(Invoice::Success), state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type_1, SHARD_ID) => { metadata: "metadata1".to_string(), supply: 30 }),
            (scheme: (asset_type_2, SHARD_ID) => { metadata: "metadata2".to_string(), supply: 30 }),
            (scheme: (asset_type_3, SHARD_ID) => { metadata: "metadata3".to_string(), supply: 30 }),
            (asset: (mint_output_1.tracker, 0, SHARD_ID)),
            (asset: (mint_output_2.tracker, 0, SHARD_ID)),
            (asset: (mint_output_3.tracker, 0, SHARD_ID)),
            (asset: (transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type_1, quantity: 10, order: order_consumed_hash }),
            (asset: (transfer_tracker, 1, SHARD_ID) => { asset_type: asset_type_2, quantity: 10, order: order_consumed_hash }),
            (asset: (transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type_3, quantity: 10, order: order_consumed_hash }),
            (asset: (transfer_tracker, 3, SHARD_ID) => { asset_type: asset_type_1, quantity: 20, order }),
            (asset: (transfer_tracker, 4, SHARD_ID) => { asset_type: asset_type_2, quantity: 20, order }),
            (asset: (transfer_tracker, 5, SHARD_ID) => { asset_type: asset_type_3, quantity: 20, order: order_consumed_hash })
        ]);
    }

    #[test]
    fn mint_and_compose() {
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);
        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let random_lock_script_hash = H160::random();
        let compose = asset_compose!(
            "composed".to_string(),
            asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, 30), vec![0x30, 0x01])],
            asset_mint_output!(random_lock_script_hash, supply: 1)
        );
        let compose_tracker = compose.tracker();
        let composed_asset_type = Blake::blake(compose_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (scheme: (composed_asset_type, SHARD_ID) => { metadata: "composed".to_string(), supply: 1, pool: [Asset::new(asset_type, amount)] }),
            (asset: (compose_tracker, 0, SHARD_ID) => { asset_type: composed_asset_type, quantity: 1 })
        ]);
    }

    #[test]
    fn mint_and_compose_and_decompose() {
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);
        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let compose = asset_compose!(
            "composed".to_string(),
            asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, amount), vec![0x30, 0x01])],
            asset_mint_output!(lock_script_hash, supply: 1)
        );
        let compose_tracker = compose.tracker();
        let composed_asset_type = Blake::blake(compose_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (scheme: (composed_asset_type, SHARD_ID) => { metadata: "composed".to_string(), supply: 1, pool: [Asset::new(asset_type, amount)] }),
            (asset: (compose_tracker, 0, SHARD_ID) => { asset_type: composed_asset_type, quantity: 1 })
        ]);

        let random_lock_script_hash = H160::random();
        let decompose = asset_decompose!(
            asset_transfer_input!(asset_out_point!(compose_tracker, 0, composed_asset_type, 1), vec![0x30, 0x01]),
            asset_transfer_outputs![(random_lock_script_hash, asset_type, amount)]
        );
        let decompose_tracker = decompose.tracker();

        assert_eq!(Ok(Invoice::Success), state.apply(&decompose.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (scheme: (composed_asset_type, SHARD_ID)  => { metadata: "composed".to_string(), supply: 1, pool: [Asset::new(asset_type, amount)] }),
            (asset: (compose_tracker, 0, SHARD_ID)),
            (asset: (decompose_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);
    }

    #[test]
    fn decompose_fail_invalid_input_different_asset_type() {
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let mint2 = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), "invalid_asset".to_string());
        let mint2_tracker = mint2.tracker();
        let asset_type2 = Blake::blake(mint2_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint2.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount }),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: amount }),
            (asset: (mint2_tracker, 0, SHARD_ID) => { asset_type: asset_type2, quantity: amount })
        ]);

        let compose = asset_compose!(
            "composed".to_string(),
            asset_transfer_inputs![(asset_out_point!(mint_tracker, 0, asset_type, amount), vec![0x30, 0x01])],
            asset_mint_output!(lock_script_hash, supply: 1)
        );
        let compose_tracker = compose.tracker();
        let composed_asset_type = Blake::blake(compose_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: amount }),
            (asset: (mint2_tracker, 0, SHARD_ID) => { asset_type: asset_type2, quantity: amount }),
            (scheme: (composed_asset_type, SHARD_ID) => { metadata: "composed".to_string(), supply: 1, pool: [Asset::new(asset_type, amount)] }),
            (asset: (compose_tracker, 0, SHARD_ID) => { asset_type: composed_asset_type, quantity: 1 })
        ]);

        let random_lock_script_hash = H160::random();
        let decompose = asset_decompose!(
            asset_transfer_input!(asset_out_point!(mint2_tracker, 0, asset_type2, 1), vec![0x30, 0x01]),
            asset_transfer_outputs![(random_lock_script_hash, asset_type, amount)]
        );

        assert_eq!(
            Ok(Invoice::Failure(
                RuntimeError::InvalidDecomposedInput {
                    asset_type: asset_type2,
                    shard_id: SHARD_ID,
                    got: 0,
                }
                .into()
            )),
            state.apply(&decompose.into(), &sender, &[], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) ),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: amount }),
            (asset: (mint2_tracker, 0, SHARD_ID) => { asset_type: asset_type2, quantity: amount }),
            (scheme: (composed_asset_type, SHARD_ID) => { metadata: "composed".to_string(), supply: 1, pool: [Asset::new(asset_type, amount)] }),
            (asset: (compose_tracker, 0, SHARD_ID) => { asset_type: composed_asset_type, quantity: 1 })
        ]);
    }

    #[test]
    fn decompose_fail_invalid_output_insufficient_output() {
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let mint2 = asset_mint!(asset_mint_output!(lock_script_hash, supply: 1), "invalid_asset".to_string());
        let mint2_tracker = mint2.tracker();
        let asset_type2 = Blake::blake(mint2_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint2.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount }),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: 1 }),
            (asset: (mint2_tracker, 0, SHARD_ID) => { asset_type: asset_type2, quantity: 1 })
        ]);

        let compose = asset_compose!(
            "composed".to_string(),
            asset_transfer_inputs![
                (asset_out_point!(mint_tracker, 0, asset_type, amount), vec![0x30, 0x01]),
                (asset_out_point!(mint2_tracker, 0, asset_type2, 1), vec![0x30, 0x01]),
            ],
            asset_mint_output!(lock_script_hash, supply: 1)
        );
        let compose_tracker = compose.tracker();
        let composed_asset_type = Blake::blake(compose_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: 1 }),
            (asset: (mint2_tracker, 0, SHARD_ID)),
            (scheme: (composed_asset_type, SHARD_ID) => { metadata: "composed".to_string(), supply: 1 }),
            (asset: (compose_tracker, 0, SHARD_ID) => { asset_type: composed_asset_type, quantity: 1 })
        ]);

        let random_lock_script_hash = H160::random();
        let decompose = asset_decompose!(
            asset_transfer_input!(asset_out_point!(compose_tracker, 0, composed_asset_type, 1), vec![0x30, 0x01]),
            asset_transfer_outputs![(random_lock_script_hash, asset_type, amount)]
        );

        assert_eq!(
            Ok(Invoice::Failure(
                RuntimeError::InvalidDecomposedOutput {
                    asset_type: asset_type2,
                    shard_id: SHARD_ID,
                    expected: 1,
                    got: 0,
                }
                .into()
            )),
            state.apply(&decompose.into(), &sender, &[], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: 1 }),
            (asset: (mint2_tracker, 0, SHARD_ID)),
            (scheme: (composed_asset_type, SHARD_ID) => { metadata: "composed".to_string(), supply: 1 }),
            (asset: (compose_tracker, 0, SHARD_ID) => { asset_type: composed_asset_type, quantity: 1 })
        ]);
    }


    #[test]
    fn decompose_fail_invalid_output_insufficient_amount() {
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, SHARD_ID, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let amount = 30;
        let mint = asset_mint!(asset_mint_output!(lock_script_hash, supply: amount), metadata.clone());
        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let mint2 = asset_mint!(asset_mint_output!(lock_script_hash, supply: 1), "invalid_asset".to_string());
        let mint2_tracker = mint2.tracker();
        let asset_type2 = Blake::blake(mint2_tracker);
        assert_eq!(Ok(Invoice::Success), state.apply(&mint2.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount }),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: 1 }),
            (asset: (mint2_tracker, 0, SHARD_ID) => { asset_type: asset_type2, quantity: 1 })
        ]);

        let compose = asset_compose!(
            "composed".to_string(),
            asset_transfer_inputs![
                (asset_out_point!(mint_tracker, 0, asset_type, amount), vec![0x30, 0x01]),
                (asset_out_point!(mint2_tracker, 0, asset_type2, 1), vec![0x30, 0x01]),
            ],
            asset_mint_output!(lock_script_hash, supply: 1)
        );
        let compose_tracker = compose.tracker();
        let composed_asset_type = Blake::blake(compose_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: 1 }),
            (asset: (mint2_tracker, 0, SHARD_ID)),
            (scheme: (composed_asset_type, SHARD_ID) => { metadata: "composed".to_string(), supply: 1 }),
            (asset: (compose_tracker, 0, SHARD_ID) => { asset_type: composed_asset_type, quantity: 1 })
        ]);

        let random_lock_script_hash = H160::random();
        let decompose = asset_decompose!(
            asset_transfer_input!(asset_out_point!(compose_tracker, 0, composed_asset_type, 1), vec![0x30, 0x01]),
            asset_transfer_outputs![
                (random_lock_script_hash, asset_type, 10),
                (random_lock_script_hash, asset_type2, 1),
            ]
        );

        assert_eq!(
            Ok(Invoice::Failure(
                RuntimeError::InvalidDecomposedOutput {
                    asset_type,
                    shard_id: SHARD_ID,
                    expected: 30,
                    got: 10,
                }
                .into()
            )),
            state.apply(&decompose.into(), &sender, &[], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, SHARD_ID) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (scheme: (asset_type2, SHARD_ID) => { metadata: "invalid_asset".to_string(), supply: 1 }),
            (asset: (mint2_tracker, 0, SHARD_ID)),
            (scheme: (composed_asset_type, SHARD_ID) => { metadata: "composed".to_string(), supply: 1 }),
            (asset: (compose_tracker, 0, SHARD_ID) => { asset_type: composed_asset_type, quantity: 1 })
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
        let asset_scheme_address = AssetSchemeAddress::new(asset_type, SHARD_ID);

        assert_eq!(wrap_ccc_tracker, tx_hash);
        assert_eq!(Ok(Invoice::Success), state.apply(&wrap_ccc, &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_scheme_address) => { supply: std::u64::MAX }),
            (asset: (wrap_ccc_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let unwrap_ccc =
            asset_unwrap_ccc!(asset_transfer_input!(asset_out_point!(wrap_ccc_tracker, 0, asset_type, 30), vec![0x01]));

        assert_eq!(Ok(Invoice::Success), state.apply(&unwrap_ccc.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_scheme_address) => { supply: std::u64::MAX }),
            (asset: (wrap_ccc_tracker, 0, SHARD_ID))
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
        assert_eq!(Ok(Invoice::Success), state.apply(&wrap_ccc, &sender, &[sender], &[], &get_test_client()));

        let asset_type = H160::zero();
        let asset_scheme_address = AssetSchemeAddress::new(asset_type, SHARD_ID);

        check_shard_level_state!(state, [
            (scheme: (asset_scheme_address) => { supply: std::u64::MAX }),
            (asset: (wrap_ccc_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&transfer.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_scheme_address) => { supply: std::u64::MAX }),
            (asset: (wrap_ccc_tracker, 0, SHARD_ID)),
            (asset: (transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1, SHARD_ID) => { asset_type: asset_type, quantity: 5 }),
            (asset: (transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type, quantity: 15 })
        ]);

        let unwrap_ccc =
            asset_unwrap_ccc!(asset_transfer_input!(asset_out_point!(transfer_tracker, 1, asset_type, 5), vec![0x01]));

        assert_eq!(Ok(Invoice::Success), state.apply(&unwrap_ccc.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_scheme_address) => { supply: std::u64::MAX }),
            (asset: (wrap_ccc_tracker, 0, SHARD_ID)),
            (asset: (transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: 10 }),
            (asset: (transfer_tracker, 1, SHARD_ID)),
            (asset: (transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type, quantity: 15 })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, 0) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
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
            Ok(Invoice::Failure(
                RuntimeError::ScriptHashMismatch(Mismatch {
                    expected: lock_script_hash,
                    found: Blake::blake(&failed_lock_script),
                })
                .into()
            )),
            state.apply(&failed_transfer.into(), &sender, &[sender], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, 0) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount }),
            (asset: (failed_transfer_tracker, 0, SHARD_ID))
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

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&successful_transfer.into(), &sender, &[sender], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, 0) => { metadata: metadata.clone(), supply: amount }),
            (asset: (mint_tracker, 0, SHARD_ID)),
            (asset: (failed_transfer_tracker, 0, SHARD_ID)),
            (asset: (successful_transfer_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: 10 }),
            (asset: (successful_transfer_tracker, 1, SHARD_ID) => { asset_type: asset_type, quantity: 5 }),
            (asset: (successful_transfer_tracker, 2, SHARD_ID) => { asset_type: asset_type, quantity: 15 })
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

        assert_eq!(Ok(Invoice::Success), state.apply(&transaction.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, 0) => { metadata: metadata.clone(), supply: ::std::u64::MAX }),
            (asset: (transaction_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: ::std::u64::MAX })
        ]);
    }

    #[test]
    fn mint_is_failed_when_the_sender_is_not_user() {
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
        let shard_user = address();

        assert_eq!(
            Ok(Invoice::Failure(RuntimeError::InsufficientPermission.into())),
            state.apply(&transaction.into(), &sender, &[shard_user], &[], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, 0)),
            (asset: (transaction_tracker, 0, SHARD_ID))
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

        assert_eq!(Ok(Invoice::Success), state.apply(&transaction.into(), &sender, &[], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, 0) => { metadata: metadata.clone(), supply: ::std::u64::MAX, approver: approver }),
            (asset: (transaction_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: ::std::u64::MAX })
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
        let administrator = Address::random();
        let mint = asset_mint!(
            asset_mint_output!(lock_script_hash, parameters.clone(), amount),
            metadata.clone(),
            administrator: administrator
        );

        let mint_tracker = mint.tracker();
        let asset_type = Blake::blake(mint_tracker);

        assert_eq!(Ok(Invoice::Success), state.apply(&mint.into(), &sender, &[sender], &[], &get_test_client()));

        check_shard_level_state!(state, [
            (scheme: (asset_type, 0) => { metadata: metadata.clone(), supply: amount, approver, administrator: administrator }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);

        let approver = Address::random();
        let change_asset_scheme = ShardTransaction::ChangeAssetScheme {
            network_id: "tc".into(),
            shard_id: SHARD_ID,
            asset_type,
            metadata: "New metadata".to_string(),
            approver: Some(approver),
            administrator: None,
            allowed_script_hashes: Vec::new(),
        };
        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&change_asset_scheme.into(), &sender, &[], &[administrator], &get_test_client())
        );

        check_shard_level_state!(state, [
            (scheme: (asset_type, 0) => { metadata: "New metadata".to_string(), supply: amount, approver: approver, administrator }),
            (asset: (mint_tracker, 0, SHARD_ID) => { asset_type: asset_type, quantity: amount })
        ]);
    }
}
