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

use std::cell::{RefCell, RefMut};
use std::collections::HashMap;

use ccrypto::{Blake, BLAKE_NULL_RLP};
use ckey::Address;
use cmerkle::{self, TrieError, TrieFactory};
use ctypes::invoice::Invoice;
use ctypes::transaction::{
    AssetMintOutput, AssetTransferInput, AssetTransferOutput, AssetWrapCCCOutput, Error as TransactionError,
    InnerTransaction, PartialHashing, Transaction,
};
use ctypes::util::unexpected::Mismatch;
use ctypes::ShardId;
use cvm::{decode, execute, ChainTimeInfo, ScriptResult, VMConfig};
use hashdb::AsHashDB;
use primitives::{Bytes, H160, H256};

use crate::cache::ShardCache;
use crate::checkpoint::{CheckpointId, StateWithCheckpoint};
use crate::traits::{ShardState, ShardStateView};
use crate::{
    Asset, AssetScheme, AssetSchemeAddress, CacheableItem, OwnedAsset, OwnedAssetAddress, StateDB, StateError,
    StateResult,
};


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
        transaction: &InnerTransaction,
        sender: &Address,
        shard_users: &[Address],
        approvers: &[Address],
        client: &C,
    ) -> StateResult<()> {
        debug_assert_eq!(Ok(()), transaction.verify());
        match transaction {
            InnerTransaction::General(transaction) => match transaction {
                Transaction::AssetMint {
                    metadata,
                    approver,
                    output:
                        AssetMintOutput {
                            lock_script_hash,
                            amount,
                            parameters,
                        },
                    ..
                } => {
                    self.mint_asset(
                        transaction.hash(),
                        metadata,
                        lock_script_hash,
                        &parameters,
                        amount,
                        approver,
                        sender,
                        shard_users,
                        Vec::new(),
                    )?;
                    Ok(())
                }
                Transaction::AssetTransfer {
                    burns,
                    inputs,
                    outputs,
                    ..
                } => {
                    debug_assert!(outputs.len() <= 512);
                    self.transfer_asset(&transaction, sender, approvers, burns, inputs, outputs, client)
                }

                Transaction::AssetCompose {
                    metadata,
                    approver,
                    inputs,
                    output,
                    ..
                } => self.compose_asset(
                    &transaction,
                    metadata,
                    approver,
                    inputs,
                    output,
                    sender,
                    approvers,
                    shard_users,
                    client,
                ),
                Transaction::AssetDecompose {
                    input,
                    outputs,
                    ..
                } => self.decompose_asset(&transaction, input, outputs, sender, approvers, client),
                Transaction::AssetUnwrapCCC {
                    burn,
                    ..
                } => self.unwrap_ccc(&transaction, sender, burn, client),
            },
            InnerTransaction::AssetWrapCCC {
                parcel_hash,
                output:
                    AssetWrapCCCOutput {
                        lock_script_hash,
                        amount,
                        parameters,
                    },
                ..
            } => self.wrap_ccc(parcel_hash, lock_script_hash, &parameters, *amount),
        }
    }

    fn mint_asset(
        &mut self,
        transaction_hash: H256,
        metadata: &str,
        lock_script_hash: &H160,
        parameters: &[Bytes],
        amount: &Option<u64>,
        approver: &Option<Address>,
        sender: &Address,
        shard_users: &[Address],
        pool: Vec<Asset>,
    ) -> StateResult<()> {
        if !shard_users.is_empty() && !shard_users.contains(sender) {
            return Err(TransactionError::InsufficientPermission.into())
        }

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, self.shard_id);
        let amount = amount.unwrap_or(::std::u64::MAX);
        let mut asset_scheme = self.get_asset_scheme_mut(&asset_scheme_address)?;
        if !asset_scheme.is_null() {
            return Err(TransactionError::AssetSchemeDuplicated(transaction_hash).into())
        }
        asset_scheme.init(metadata.to_string(), amount, *approver, pool);

        ctrace!(TX, "{:?} is minted on {:?}", asset_scheme, asset_scheme_address);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, self.shard_id);
        let mut asset = self.get_asset_mut(&asset_address)?;
        asset.init(asset_scheme_address.into(), *lock_script_hash, parameters.to_vec(), amount);
        ctrace!(TX, "{:?} is generated on {:?}", asset, asset_address);
        Ok(())
    }

    fn transfer_asset<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        sender: &Address,
        approvers: &[Address],
        burns: &[AssetTransferInput],
        inputs: &[AssetTransferInput],
        outputs: &[AssetTransferOutput],
        client: &C,
    ) -> StateResult<()> {
        for (input, burn) in inputs.iter().map(|input| (input, false)).chain(burns.iter().map(|input| (input, true))) {
            let address = OwnedAssetAddress::new(input.prev_out.transaction_hash, input.prev_out.index, self.shard_id);
            let script_result = self.check_and_run_input_script(input, transaction, burn, client)?;
            match (script_result, burn) {
                (ScriptResult::Unlocked, false) => {}
                (ScriptResult::Burnt, true) => {}
                _ => return Err(TransactionError::FailedToUnlock(address.into()).into()),
            }
        }

        let mut deleted_asset = Vec::with_capacity(inputs.len() + burns.len());
        for input in inputs.iter().chain(burns) {
            let (_, asset_address) = self.check_input_asset(input, sender, approvers)?;
            self.kill_asset(&asset_address);
            deleted_asset.push((asset_address, input.prev_out.amount));
        }
        let mut created_asset = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.iter().enumerate() {
            let asset_address = OwnedAssetAddress::new(transaction.hash(), index, self.shard_id);
            let mut asset = self.get_asset_mut(&asset_address)?;
            asset.init(output.asset_type, output.lock_script_hash, output.parameters.clone(), output.amount);
            created_asset.push((asset_address, output.amount));
        }
        ctrace!(TX, "Deleted assets {:?}", deleted_asset);
        ctrace!(TX, "Created assets {:?}", created_asset);
        Ok(())
    }

    fn check_input_asset(
        &self,
        input: &AssetTransferInput,
        sender: &Address,
        approvers: &[Address],
    ) -> StateResult<(OwnedAsset, OwnedAssetAddress)> {
        let asset_address =
            OwnedAssetAddress::new(input.prev_out.transaction_hash, input.prev_out.index, self.shard_id);
        let asset_scheme_address = AssetSchemeAddress::from_hash(input.prev_out.asset_type)
            .ok_or_else(|| TransactionError::AssetSchemeNotFound(input.prev_out.asset_type))?;

        let asset_scheme = self
            .asset_scheme(&asset_scheme_address)?
            .ok_or_else(|| TransactionError::AssetSchemeNotFound(asset_scheme_address.into()))?;

        if let Some(approver) = asset_scheme.approver().as_ref() {
            if sender != approver && !approvers.contains(approver) {
                return Err(TransactionError::NotApproved(*approver).into())
            }
        }

        match self.asset(&asset_address)? {
            Some(asset) => {
                if asset.amount() != input.prev_out.amount {
                    return Err(TransactionError::InvalidAssetAmount {
                        address: asset_address.into(),
                        expected: asset.amount(),
                        got: input.prev_out.amount,
                    }
                    .into())
                }
                if *asset.asset_type() != input.prev_out.asset_type {
                    return Err(TransactionError::InvalidAssetType(input.prev_out.asset_type).into())
                }
                Ok((asset, asset_address))
            }
            None => Err(TransactionError::AssetNotFound(asset_address.into()).into()),
        }
    }

    fn check_and_run_input_script<C: ChainTimeInfo>(
        &self,
        input: &AssetTransferInput,
        transaction: &PartialHashing,
        burn: bool,
        client: &C,
    ) -> StateResult<ScriptResult> {
        let (address_hash, asset) = {
            let index = input.prev_out.index;
            let address = OwnedAssetAddress::new(input.prev_out.transaction_hash, index, self.shard_id);
            match self.asset(&address)? {
                Some(asset) => (address.into(), asset),
                None => return Err(TransactionError::AssetNotFound(address.into()).into()),
            }
        };

        if *asset.lock_script_hash() != Blake::blake(&input.lock_script) {
            return Err(TransactionError::ScriptHashMismatch(Mismatch {
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
                transaction,
                VMConfig::default(),
                input,
                burn,
                client,
            ),
            // FIXME : Deliver full decode error
            _ => return Err(TransactionError::InvalidScript.into()),
        }
        .map_err(|err| {
            ctrace!(TX, "Cannot run unlock/lock script {:?}", err);
            TransactionError::FailedToUnlock(address_hash)
        })?;
        Ok(script_result)
    }

    fn compose_asset<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        metadata: &str,
        approver: &Option<Address>,
        inputs: &[AssetTransferInput],
        output: &AssetMintOutput,
        sender: &Address,
        approvers: &[Address],
        shard_users: &[Address],
        client: &C,
    ) -> StateResult<()> {
        let mut sum: HashMap<H256, u64> = HashMap::new();

        let mut deleted_assets: Vec<(H256, _)> = Vec::with_capacity(inputs.len());
        for input in inputs.iter() {
            let (_, asset_address) = self.check_input_asset(input, sender, approvers)?;
            let script_result = self.check_and_run_input_script(input, transaction, false, client)?;

            match script_result {
                ScriptResult::Unlocked => {}
                _ => return Err(TransactionError::FailedToUnlock(asset_address.into()).into()),
            }

            self.kill_asset(&asset_address);
            deleted_assets.push((asset_address.into(), input.prev_out.amount));

            let asset_type = input.prev_out.asset_type;
            let current_amount = sum.get(&asset_type).cloned().unwrap_or_default();
            sum.insert(asset_type, current_amount + input.prev_out.amount);
        }
        ctrace!(TX, "Deleted assets {:?}", deleted_assets);

        let pool = sum.into_iter().map(|(asset_type, amount)| Asset::new(asset_type, amount)).collect();

        self.mint_asset(
            transaction.hash(),
            metadata,
            &output.lock_script_hash,
            &output.parameters,
            &output.amount,
            approver,
            sender,
            shard_users,
            pool,
        )
    }

    fn decompose_asset<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        input: &AssetTransferInput,
        outputs: &[AssetTransferOutput],
        sender: &Address,
        approvers: &[Address],
        client: &C,
    ) -> StateResult<()> {
        let asset_type = input.prev_out.asset_type;
        let asset_scheme_address = AssetSchemeAddress::from_hash(asset_type)
            .ok_or_else(|| TransactionError::AssetSchemeNotFound(asset_type))?;
        let asset_scheme = self
            .asset_scheme(&asset_scheme_address)?
            .ok_or_else(|| TransactionError::AssetSchemeNotFound(asset_scheme_address.into()))?;
        // The input asset should be composed asset
        if asset_scheme.pool().is_empty() {
            return Err(TransactionError::InvalidDecomposedInput {
                address: asset_type,
                got: 0,
            }
            .into())
        }

        // Check that the outputs are match with pool
        let mut sum: HashMap<H256, u64> = HashMap::new();
        for output in outputs {
            let output_type = output.asset_type;
            let current_amount = sum.get(&output_type).cloned().unwrap_or_default();
            sum.insert(output_type, current_amount + output.amount);
        }
        for asset in asset_scheme.pool() {
            match sum.remove(asset.asset_type()) {
                None => {
                    return Err(TransactionError::InvalidDecomposedOutput {
                        address: *asset.asset_type(),
                        expected: asset.amount(),
                        got: 0,
                    }
                    .into())
                }
                Some(value) => {
                    if value != asset.amount() {
                        return Err(TransactionError::InvalidDecomposedOutput {
                            address: *asset.asset_type(),
                            expected: asset.amount(),
                            got: value,
                        }
                        .into())
                    }
                }
            }
        }
        if !sum.is_empty() {
            let mut invalid_assets: Vec<Asset> =
                sum.into_iter().map(|(asset_type, amount)| Asset::new(asset_type, amount)).collect();
            let invalid_asset = invalid_assets.pop().unwrap();
            return Err(TransactionError::InvalidDecomposedOutput {
                address: *invalid_asset.asset_type(),
                expected: 0,
                got: invalid_asset.amount(),
            }
            .into())
        }


        let (_, asset_address) = self.check_input_asset(input, sender, approvers)?;
        let script_result = self.check_and_run_input_script(input, transaction, false, client)?;

        match script_result {
            ScriptResult::Unlocked => {}
            _ => return Err(TransactionError::FailedToUnlock(asset_address.into()).into()),
        }

        self.kill_asset(&asset_address);
        self.kill_asset_scheme(&asset_scheme_address);

        ctrace!(TX, "Deleted assets {:?} {:?}", asset_type, input.prev_out.amount);

        // Put asset into DB
        for (index, output) in outputs.iter().enumerate() {
            let asset_address = OwnedAssetAddress::new(transaction.hash(), index, self.shard_id);
            let mut asset = self.get_asset_mut(&asset_address)?;
            asset.init(output.asset_type, output.lock_script_hash, output.parameters.clone(), output.amount);
        }

        Ok(())
    }

    fn wrap_ccc(
        &mut self,
        parcel_hash: &H256,
        lock_script_hash: &H160,
        parameters: &[Bytes],
        amount: u64,
    ) -> StateResult<()> {
        let asset_scheme_address = AssetSchemeAddress::new_with_zero_suffix(self.shard_id);
        let mut asset_scheme = self.get_asset_scheme_mut(&asset_scheme_address)?;
        if asset_scheme.is_null() {
            // FIXME: Wrapped CCC is minted in here, but the metadata is not well-defined.
            asset_scheme.init(
                format!("{{\"name\":\"Wrapped CCC\",\"description\":\"Wrapped CCC in shard {}\"}}", self.shard_id),
                ::std::u64::MAX,
                None,
                Vec::new(),
            );
            ctrace!(
                TX,
                "Wrapped CCC in shard {} ({:?}) is minted on {:?}",
                self.shard_id,
                asset_scheme,
                asset_scheme_address
            );
        }

        let asset_address = OwnedAssetAddress::new(*parcel_hash, 0, self.shard_id);
        let mut asset = self.get_asset_mut(&asset_address)?;
        asset.init(asset_scheme_address.into(), *lock_script_hash, parameters.to_vec(), amount);
        ctrace!(TX, "Created Wrapped CCC {:?} on {:?}", asset, asset_address);
        Ok(())
    }

    fn unwrap_ccc<C: ChainTimeInfo>(
        &mut self,
        transaction: &Transaction,
        sender: &Address,
        burn: &AssetTransferInput,
        client: &C,
    ) -> StateResult<()> {
        let address = OwnedAssetAddress::new(burn.prev_out.transaction_hash, burn.prev_out.index, self.shard_id);
        let script_result = self.check_and_run_input_script(burn, transaction, true, client)?;
        if script_result != ScriptResult::Burnt {
            return Err(TransactionError::FailedToUnlock(address.into()).into())
        }

        // WCCC has no approvers
        let (_, asset_address) = self.check_input_asset(burn, sender, &[])?;
        self.kill_asset(&asset_address);
        ctrace!(TX, "Removed Wrapped CCC asset {:?}, amount {:?}", asset_address, burn.prev_out.amount);
        Ok(())
    }

    fn kill_asset(&mut self, account: &OwnedAssetAddress) {
        self.cache.remove_asset(account);
    }

    fn kill_asset_scheme(&mut self, account: &AssetSchemeAddress) {
        self.cache.remove_asset_scheme(account);
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
        transaction: &InnerTransaction,
        sender: &Address,
        shard_users: &[Address],
        approvers: &[Address],
        client: &C,
    ) -> StateResult<Invoice> {
        ctrace!(TX, "Execute InnerTx {:?}(InnerTxHash:{:?})", transaction, transaction.hash());

        self.create_checkpoint(TRANSACTION_CHECKPOINT);
        let result = self.apply_internal(transaction, sender, shard_users, approvers, client);
        match result {
            Ok(_) => {
                cinfo!(TX, "InnerTx({}) is applied", transaction.hash());
                self.discard_checkpoint(TRANSACTION_CHECKPOINT);
                Ok(Invoice::Success)
            }
            Err(StateError::Transaction(err)) => {
                cinfo!(TX, "Cannot apply InnerTx({}): {:?}", transaction.hash(), err);
                self.revert_to_checkpoint(TRANSACTION_CHECKPOINT);
                Ok(Invoice::Failure(err.into()))
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
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let amount = 100;
        let approver = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: Some(amount),
            },
            approver,
        };

        let result = state.apply(&transaction.clone().into(), &sender, &[sender], &[], &get_test_client());
        assert_eq!(Ok(Invoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_scheme_address.into(), lock_script_hash, parameters, amount))), asset);
    }

    #[test]
    fn mint_infinite_asset() {
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            approver,
        };

        let result = state.apply(&transaction.clone().into(), &sender, &[sender], &[], &get_test_client());
        assert_eq!(Ok(Invoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(
            Ok(Some(OwnedAsset::new(asset_scheme_address.into(), lock_script_hash, parameters, ::std::u64::MAX))),
            asset
        );
    }

    #[test]
    fn cannot_mint_twice() {
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            approver,
        };

        let result = state.apply(&transaction.clone().into(), &sender, &[sender], &[], &get_test_client());
        assert_eq!(Ok(Invoice::Success), result);

        let result = state.apply(&transaction.clone().into(), &sender, &[sender], &[], &get_test_client());
        assert_eq!(Ok(Invoice::Failure(TransactionError::AssetSchemeDuplicated(transaction.hash()).into())), result);
    }

    #[test]
    fn invalid_approver() {
        let shard_id = 0;
        let network_id = "tc".into();
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let approver = Some(Address::random());
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

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
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash,
                parameters: vec![],
                asset_type,
                amount: 30,
            }],
            orders: vec![],
        };

        assert_eq!(
            Ok(Invoice::Failure(TransactionError::NotApproved(approver.unwrap()).into())),
            state.apply(&transfer.clone().into(), &sender, &[sender], &[], &get_test_client())
        );
    }

    #[test]
    fn mint_and_transfer() {
        let network_id = "tc".into();
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();

        let network_id = "tc".into();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let random_lock_script_hash = H160::random();
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
                timelock: None,
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
            orders: vec![],
        };
        let transfer_hash = transfer.hash();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&transfer.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset0_address = OwnedAssetAddress::new(transfer_hash, 0, shard_id);
        let asset0 = state.asset(&asset0_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = OwnedAssetAddress::new(transfer_hash, 1, shard_id);
        let asset1 = state.asset(&asset1_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], 5))), asset1);

        let asset2_address = OwnedAssetAddress::new(transfer_hash, 2, shard_id);
        let asset2 = state.asset(&asset2_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);
    }

    #[test]
    fn mint_and_burn() {
        let network_id = "tc".into();
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();

        let network_id = "tc".into();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let burn = Transaction::AssetTransfer {
            network_id,
            burns: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount,
                },
                timelock: None,
                lock_script: vec![0x01],
                unlock_script: vec![],
            }],
            inputs: vec![],
            outputs: vec![],
            orders: vec![],
        };

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&burn.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);
        let asset_burnt = state.asset(&asset_address);
        assert_eq!(Ok(None), asset_burnt);
    }

    #[test]
    fn mint_and_transfer_and_burn() {
        let network_id = "tc".into();
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();

        let network_id = "tc".into();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let lock_script_hash_burn = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let random_lock_script_hash = H160::random();
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
                timelock: None,
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
                    lock_script_hash: lock_script_hash_burn,
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
            orders: vec![],
        };
        let transfer_hash = transfer.hash();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&transfer.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset0_address = OwnedAssetAddress::new(transfer_hash, 0, shard_id);
        let asset0 = state.asset(&asset0_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = OwnedAssetAddress::new(transfer_hash, 1, shard_id);
        let asset1 = state.asset(&asset1_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash_burn, vec![], 5))), asset1);

        let asset2_address = OwnedAssetAddress::new(transfer_hash, 2, shard_id);
        let asset2 = state.asset(&asset2_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);

        let burn = Transaction::AssetTransfer {
            network_id,
            burns: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: transfer_hash,
                    index: 1,
                    asset_type,
                    amount: 5,
                },
                timelock: None,
                lock_script: vec![0x01],
                unlock_script: vec![],
            }],
            inputs: vec![],
            outputs: vec![],
            orders: vec![],
        };

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&burn.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset1_address = OwnedAssetAddress::new(transfer_hash, 1, shard_id);
        let asset1_burnt = state.asset(&asset1_address);
        assert_eq!(Ok(None), asset1_burnt);
    }

    #[test]
    fn cannot_transfer_because_prev_out_amount_is_invalid() {
        let network_id = "tc".into();
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 20,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash,
                parameters: vec![vec![1]],
                asset_type,
                amount: 20,
            }],
            orders: vec![],
        };

        assert_eq!(
            Ok(Invoice::Failure(
                TransactionError::InvalidAssetAmount {
                    address: asset_address.into(),
                    expected: 30,
                    got: 20
                }
                .into()
            )),
            state.apply(&transfer.clone().into(), &sender, &[sender], &[], &get_test_client())
        );
    }

    #[test]
    fn cannot_transfer_because_prev_out_type_is_invalid() {
        let network_id = "tc".into();
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;

        let metadata1 = "metadata".to_string();
        let mint1 = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata1.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash1 = mint1.hash();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint1.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_scheme_address1 = AssetSchemeAddress::new(mint_hash1, shard_id);
        let asset_scheme1 = state.asset_scheme(&asset_scheme_address1);
        let asset_type1 = asset_scheme_address1.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata1.clone(), amount, approver))), asset_scheme1);

        let asset_address1 = OwnedAssetAddress::new(mint_hash1, 0, shard_id);
        let asset1 = state.asset(&asset_address1);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type1, lock_script_hash, vec![], amount))), asset1);

        let metadata2 = "metadata2".to_string();
        let mint2 = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata2.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash2 = mint2.hash();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint2.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_scheme_address2 = AssetSchemeAddress::new(mint_hash2, shard_id);
        let asset_scheme2 = state.asset_scheme(&asset_scheme_address2);
        let asset_type2 = asset_scheme_address2.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata2.clone(), amount, approver))), asset_scheme2);

        let asset_address2 = OwnedAssetAddress::new(mint_hash2, 0, shard_id);
        let asset2 = state.asset(&asset_address2);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type2, lock_script_hash, vec![], amount))), asset2);

        let transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash1,
                    index: 0,
                    asset_type: asset_type2,
                    amount: 30,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash,
                parameters: vec![vec![1]],
                asset_type: asset_type2,
                amount: 30,
            }],
            orders: vec![],
        };

        assert_eq!(
            Ok(Invoice::Failure(TransactionError::InvalidAssetType(asset_type2).into())),
            state.apply(&transfer.clone().into(), &sender, &[sender], &[], &get_test_client())
        )
    }

    #[test]
    fn mint_and_compose() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(Invoice::Success), state.apply(&mint.clone().into(), &sender, &[], &[], &get_test_client()));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.into();

        let random_lock_script_hash = H160::random();
        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            metadata: "composed".to_string(),
            approver,
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 30,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            output: AssetMintOutput {
                lock_script_hash: random_lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
        };
        let compose_hash = compose.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.clone().into(), &sender, &[], &[], &get_test_client()));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_scheme = state.asset_scheme(&composed_asset_scheme_address);
        let composed_asset_type = composed_asset_scheme_address.into();

        assert_eq!(
            Ok(Some(AssetScheme::new_with_pool("composed".to_string(), 1, approver, vec![Asset::new(asset_type, 30)]))),
            composed_asset_scheme
        );

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, random_lock_script_hash, vec![], 1))), composed_asset);
    }

    #[test]
    fn mint_and_compose_and_decompose() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(Invoice::Success), state.apply(&mint.clone().into(), &sender, &[], &[], &get_test_client()));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.into();

        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            metadata: "composed".to_string(),
            approver,
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
        };
        let compose_hash = compose.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.clone().into(), &sender, &[], &[], &get_test_client()));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_scheme = state.asset_scheme(&composed_asset_scheme_address);
        let composed_asset_type = composed_asset_scheme_address.into();

        assert_eq!(
            Ok(Some(AssetScheme::new_with_pool("composed".to_string(), 1, approver, vec![Asset::new(asset_type, 30)]))),
            composed_asset_scheme
        );

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, lock_script_hash, vec![], 1))), composed_asset);

        let random_lock_script_hash = H160::random();
        let decompose = Transaction::AssetDecompose {
            network_id,
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: compose_hash,
                    index: 0,
                    asset_type: composed_asset_type,
                    amount: 1,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            outputs: vec![AssetTransferOutput {
                lock_script_hash: random_lock_script_hash,
                parameters: vec![],
                asset_type,
                amount,
            }],
        };
        let decompose_hash = decompose.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&decompose.clone().into(), &sender, &[], &[], &get_test_client()));

        let asset_scheme = state.asset_scheme(&asset_scheme_address);

        assert_eq!(Ok(Some(AssetScheme::new("metadata".to_string(), 30, approver))), asset_scheme);

        let decomposed_asset_address = OwnedAssetAddress::new(decompose_hash, 0, shard_id);
        let decomposed_asset = state.asset(&decomposed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, random_lock_script_hash, vec![], 30))), decomposed_asset);
    }

    #[test]
    fn decompose_fail_invalid_input_different_asset_type() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(Invoice::Success), state.apply(&mint.clone().into(), &sender, &[], &[], &get_test_client()));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.into();

        let mint2 = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: "invalid_asset".to_string(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
            approver,
        };
        let mint2_hash = mint2.hash();
        let asset_scheme_address2 = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type2 = asset_scheme_address2.into();
        assert_eq!(Ok(Invoice::Success), state.apply(&mint2.into(), &sender, &[], &[], &get_test_client()));

        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            metadata: "composed".to_string(),
            approver,
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
        };
        let compose_hash = compose.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.clone().into(), &sender, &[], &[], &get_test_client()));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_scheme = state.asset_scheme(&composed_asset_scheme_address);
        let composed_asset_type = composed_asset_scheme_address.into();

        assert_eq!(
            Ok(Some(AssetScheme::new_with_pool("composed".to_string(), 1, approver, vec![Asset::new(asset_type, 30)]))),
            composed_asset_scheme
        );

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, lock_script_hash, vec![], 1))), composed_asset);

        let random_lock_script_hash = H160::random();
        let decompose = Transaction::AssetDecompose {
            network_id,
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint2_hash,
                    index: 0,
                    asset_type: asset_type2,
                    amount: 1,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            outputs: vec![AssetTransferOutput {
                lock_script_hash: random_lock_script_hash,
                parameters: vec![],
                asset_type,
                amount,
            }],
        };

        assert_eq!(
            Ok(Invoice::Failure(
                TransactionError::InvalidDecomposedInput {
                    address: asset_type,
                    got: 0,
                }
                .into()
            )),
            state.apply(&decompose.clone().into(), &sender, &[], &[], &get_test_client())
        );
    }

    #[test]
    fn decompose_fail_invalid_output_insufficient_output() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(Invoice::Success), state.apply(&mint.clone().into(), &sender, &[], &[], &get_test_client()));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.into();

        let mint2 = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: "invalid_asset".to_string(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
            approver,
        };
        let mint2_hash = mint2.hash();
        let asset_scheme_address2 = AssetSchemeAddress::new(mint2_hash, shard_id);
        let asset_type2 = asset_scheme_address2.into();
        assert_eq!(Ok(Invoice::Success), state.apply(&mint2.into(), &sender, &[], &[], &get_test_client()));

        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            metadata: "composed".to_string(),
            approver,
            inputs: vec![
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: mint_hash,
                        index: 0,
                        asset_type,
                        amount,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: mint2_hash,
                        index: 0,
                        asset_type: asset_type2,
                        amount: 1,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
        };
        let compose_hash = compose.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.clone().into(), &sender, &[], &[], &get_test_client()));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_type = composed_asset_scheme_address.into();

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, lock_script_hash, vec![], 1))), composed_asset);

        let random_lock_script_hash = H160::random();
        let decompose = Transaction::AssetDecompose {
            network_id,
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: compose_hash,
                    index: 0,
                    asset_type: composed_asset_type,
                    amount: 1,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            outputs: vec![AssetTransferOutput {
                lock_script_hash: random_lock_script_hash,
                parameters: vec![],
                asset_type,
                amount,
            }],
        };

        assert_eq!(
            Ok(Invoice::Failure(
                TransactionError::InvalidDecomposedOutput {
                    address: asset_type2,
                    expected: 1,
                    got: 0,
                }
                .into()
            )),
            state.apply(&decompose.clone().into(), &sender, &[], &[], &get_test_client())
        );
    }


    #[test]
    fn decompose_fail_invalid_output_insufficient_amount() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(Invoice::Success), state.apply(&mint.clone().into(), &sender, &[], &[], &get_test_client()));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.into();

        let mint2 = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: "invalid_asset".to_string(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
            approver,
        };
        let mint2_hash = mint2.hash();
        let asset_scheme_address2 = AssetSchemeAddress::new(mint2_hash, shard_id);
        let asset_type2 = asset_scheme_address2.into();
        assert_eq!(Ok(Invoice::Success), state.apply(&mint2.into(), &sender, &[], &[], &get_test_client()));

        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            metadata: "composed".to_string(),
            approver,
            inputs: vec![
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: mint_hash,
                        index: 0,
                        asset_type,
                        amount,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: mint2_hash,
                        index: 0,
                        asset_type: asset_type2,
                        amount: 1,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
        };
        let compose_hash = compose.hash();

        assert_eq!(Ok(Invoice::Success), state.apply(&compose.clone().into(), &sender, &[], &[], &get_test_client()));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_type = composed_asset_scheme_address.into();

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, lock_script_hash, vec![], 1))), composed_asset);

        let random_lock_script_hash = H160::random();
        let decompose = Transaction::AssetDecompose {
            network_id,
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: compose_hash,
                    index: 0,
                    asset_type: composed_asset_type,
                    amount: 1,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash: random_lock_script_hash,
                    parameters: vec![],
                    asset_type,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash: random_lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type2,
                    amount: 1,
                },
            ],
        };

        assert_eq!(
            Ok(Invoice::Failure(
                TransactionError::InvalidDecomposedOutput {
                    address: asset_type,
                    expected: 30,
                    got: 10,
                }
                .into()
            )),
            state.apply(&decompose.clone().into(), &sender, &[], &[], &get_test_client())
        );
    }

    #[test]
    fn wrap_and_unwrap_ccc() {
        let network_id = "tc".into();
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let lock_script_hash = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let parcel_hash = H256::random();
        let amount = 30;

        let wrap_ccc = InnerTransaction::AssetWrapCCC {
            network_id,
            shard_id,
            parcel_hash,
            output: AssetWrapCCCOutput {
                lock_script_hash,
                parameters: vec![],
                amount,
            },
        };
        let wrap_ccc_hash = wrap_ccc.hash();

        assert_eq!(wrap_ccc_hash, parcel_hash);
        assert_eq!(Ok(Invoice::Success), state.apply(&wrap_ccc, &sender, &[sender], &[], &get_test_client()));

        let asset_scheme_address = AssetSchemeAddress::new_with_zero_suffix(shard_id);
        let asset_type = asset_scheme_address.into();
        let asset_address = OwnedAssetAddress::new(wrap_ccc_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let unwrap_ccc = Transaction::AssetUnwrapCCC {
            network_id,
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: wrap_ccc_hash,
                    index: 0,
                    asset_type,
                    amount: 30,
                },
                timelock: None,
                lock_script: vec![0x01],
                unlock_script: vec![],
            },
        };

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&unwrap_ccc.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_address = OwnedAssetAddress::new(wrap_ccc_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(None), asset);
    }

    #[test]
    fn wrap_ccc_and_transfer_and_unwrap_ccc() {
        let network_id = "tc".into();
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let parcel_hash = H256::random();
        let amount = 30;

        let wrap_ccc = InnerTransaction::AssetWrapCCC {
            network_id,
            shard_id,
            parcel_hash,
            output: AssetWrapCCCOutput {
                lock_script_hash,
                parameters: vec![],
                amount,
            },
        };
        let wrap_ccc_hash = wrap_ccc.hash();

        assert_eq!(wrap_ccc_hash, parcel_hash);
        assert_eq!(Ok(Invoice::Success), state.apply(&wrap_ccc, &sender, &[sender], &[], &get_test_client()));

        let asset_scheme_address = AssetSchemeAddress::new_with_zero_suffix(shard_id);
        let asset_type = asset_scheme_address.into();
        let asset_address = OwnedAssetAddress::new(wrap_ccc_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let lock_script_hash_burn = H160::from("ca5d3fa0a6887285ef6aa85cb12960a2b6706e00");
        let random_lock_script_hash = H160::random();
        let transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: wrap_ccc_hash,
                    index: 0,
                    asset_type,
                    amount: 30,
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
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash: lock_script_hash_burn,
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
            orders: vec![],
        };
        let transfer_hash = transfer.hash();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&transfer.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset0_address = OwnedAssetAddress::new(transfer_hash, 0, shard_id);
        let asset0 = state.asset(&asset0_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = OwnedAssetAddress::new(transfer_hash, 1, shard_id);
        let asset1 = state.asset(&asset1_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash_burn, vec![], 5))), asset1);

        let asset2_address = OwnedAssetAddress::new(transfer_hash, 2, shard_id);
        let asset2 = state.asset(&asset2_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);

        let unwrap_ccc = Transaction::AssetUnwrapCCC {
            network_id,
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: transfer_hash,
                    index: 1,
                    asset_type,
                    amount: 5,
                },
                timelock: None,
                lock_script: vec![0x01],
                unlock_script: vec![],
            },
        };

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&unwrap_ccc.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset1_address = OwnedAssetAddress::new(transfer_hash, 1, shard_id);
        let asset1 = state.asset(&asset1_address);
        assert_eq!(Ok(None), asset1);
    }

    #[test]
    fn mint_and_failed_transfer_and_successful_transfer() {
        let network_id = "tc".into();
        let shard_id = 0;

        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let approver = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(amount),
            },
            approver,
        };
        let mint_hash = mint.hash();

        let network_id = "tc".into();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&mint.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(mint_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let failed_lock_script = vec![0x30];
        let failed_transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 30,
                },
                timelock: None,
                lock_script: failed_lock_script.clone(),
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash,
                parameters: vec![vec![1]],
                asset_type,
                amount: 30,
            }],
            orders: vec![],
        };

        let sender = address();
        let failed_invoice =
            state.apply(&failed_transfer.clone().into(), &sender, &[sender], &[], &get_test_client()).unwrap();
        assert_eq!(
            Invoice::Failure(
                TransactionError::ScriptHashMismatch(Mismatch {
                    expected: lock_script_hash,
                    found: Blake::blake(&failed_lock_script),
                })
                .into()
            ),
            failed_invoice
        );

        let random_lock_script_hash = H160::random();
        let successful_transfer = Transaction::AssetTransfer {
            network_id,
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 30,
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
            orders: vec![],
        };
        let successful_transfer_hash = successful_transfer.hash();

        assert_eq!(
            Ok(Invoice::Success),
            state.apply(&successful_transfer.clone().into(), &sender, &[sender], &[], &get_test_client())
        );

        let asset0_address = OwnedAssetAddress::new(successful_transfer_hash, 0, shard_id);
        let asset0 = state.asset(&asset0_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = OwnedAssetAddress::new(successful_transfer_hash, 1, shard_id);
        let asset1 = state.asset(&asset1_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, lock_script_hash, vec![], 5))), asset1);

        let asset2_address = OwnedAssetAddress::new(successful_transfer_hash, 2, shard_id);
        let asset2 = state.asset(&asset2_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);
    }

    #[test]
    fn users_can_mint_asset() {
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            approver,
        };

        let result = state.apply(&transaction.clone().into(), &sender, &[sender], &[], &get_test_client());
        assert_eq!(Ok(Invoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(
            Ok(Some(OwnedAsset::new(asset_scheme_address.into(), lock_script_hash, parameters, ::std::u64::MAX))),
            asset
        );
    }

    #[test]
    fn mint_is_failed_when_the_sender_is_not_user() {
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            approver,
        };

        let shard_user = address();
        let result = state.apply(&transaction.clone().into(), &sender, &[shard_user], &[], &get_test_client());
        assert_eq!(Ok(Invoice::Failure(TransactionError::InsufficientPermission.into())), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(None), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(None), asset);
    }

    #[test]
    fn anyone_can_mint_if_no_users() {
        let shard_id = 0;
        let sender = address();
        let mut state_db = RefCell::new(get_temp_state_db());
        let mut shard_cache = ShardCache::default();
        let mut state = get_temp_shard_state(&mut state_db, shard_id, &mut shard_cache);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let approver = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            approver,
        };

        let result = state.apply(&transaction.clone().into(), &sender, &[], &[], &get_test_client());
        assert_eq!(Ok(Invoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, approver))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(
            Ok(Some(OwnedAsset::new(asset_scheme_address.into(), lock_script_hash, parameters, ::std::u64::MAX))),
            asset
        );
    }
}
