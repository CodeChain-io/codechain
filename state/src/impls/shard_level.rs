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

use std::cell::RefMut;
use std::collections::HashMap;
use std::fmt;

use ccrypto::{Blake, BLAKE_NULL_RLP};
use ckey::Address;
use cmerkle::{self, Result as TrieResult, TrieError, TrieFactory};
use ctypes::invoice::TransactionInvoice;
use ctypes::transaction::{
    AssetMintOutput, AssetOutPoint, AssetTransferInput, AssetTransferOutput, Error as TransactionError, PartialHashing,
    Transaction,
};
use ctypes::util::unexpected::Mismatch;
use ctypes::ShardId;
use cvm::{decode, execute, ScriptResult, VMConfig};
use primitives::{Bytes, H160, H256};

use super::super::backend::{Backend, ShardBackend};
use super::super::checkpoint::{CheckpointId, StateWithCheckpoint};
use super::super::item::local_cache::{CacheableItem, LocalCache};
use super::super::traits::{ShardState, ShardStateInfo, StateWithCache};
use super::super::{Asset, AssetScheme, AssetSchemeAddress, OwnedAsset, OwnedAssetAddress};
use super::super::{StateError, StateResult};


pub struct ShardLevelState<B> {
    db: B,
    root: H256,
    asset_scheme: LocalCache<AssetScheme>,
    asset: LocalCache<OwnedAsset>,
    id_of_checkpoints: Vec<CheckpointId>,
    shard_id: ShardId,
}

impl<B: Backend + ShardBackend> ShardLevelState<B> {
    /// Creates new state with empty state root
    pub fn try_new(shard_id: ShardId, db: B) -> StateResult<ShardLevelState<B>> {
        let root = BLAKE_NULL_RLP;
        Ok(ShardLevelState {
            db,
            root,
            asset_scheme: LocalCache::new(),
            asset: LocalCache::new(),
            id_of_checkpoints: Default::default(),
            shard_id,
        })
    }

    /// Creates new state with existing state root
    pub fn from_existing(shard_id: ShardId, db: B, root: H256) -> cmerkle::Result<ShardLevelState<B>> {
        if !db.as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root).into())
        }

        Ok(ShardLevelState {
            db,
            root,
            asset_scheme: LocalCache::new(),
            asset: LocalCache::new(),
            id_of_checkpoints: Default::default(),
            shard_id,
        })
    }

    /// Destroy the current object and return root and database.
    pub fn drop(mut self) -> (H256, B) {
        self.propagate_to_global_cache();
        (self.root, self.db)
    }

    fn apply_internal(
        &mut self,
        transaction: &Transaction,
        sender: &Address,
        shard_users: &[Address],
    ) -> StateResult<()> {
        debug_assert_eq!(Ok(()), transaction.verify());
        match transaction {
            Transaction::AssetMint {
                metadata,
                registrar,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        amount,
                        parameters,
                    },
                ..
            } => Ok(self.mint_asset(
                transaction.hash(),
                metadata,
                lock_script_hash,
                parameters,
                amount,
                registrar,
                sender,
                shard_users,
                Vec::new(),
            )?),
            Transaction::AssetTransfer {
                burns,
                inputs,
                outputs,
                ..
            } => {
                debug_assert!(outputs.len() <= 512);
                self.transfer_asset(&transaction, sender, burns, inputs, outputs)
            }

            Transaction::AssetCompose {
                metadata,
                registrar,
                inputs,
                output,
                ..
            } => self.compose_asset(&transaction, metadata, registrar, inputs, output, sender, shard_users),
            Transaction::AssetDecompose {
                input,
                outputs,
                ..
            } => self.decompose_asset(&transaction, input, outputs, sender),
        }
    }

    fn mint_asset(
        &mut self,
        transaction_hash: H256,
        metadata: &String,
        lock_script_hash: &H160,
        parameters: &Vec<Bytes>,
        amount: &Option<u64>,
        registrar: &Option<Address>,
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
        asset_scheme.init(metadata.clone(), amount, registrar.clone(), pool);

        ctrace!(TX, "{:?} is minted on {:?}", asset_scheme, asset_scheme_address);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, self.shard_id);
        let mut asset = self.get_asset_mut(&asset_address)?;
        asset.init(asset_scheme_address.into(), *lock_script_hash, parameters.clone(), amount);
        ctrace!(TX, "{:?} is generated on {:?}", asset, asset_address);
        Ok(())
    }

    fn transfer_asset(
        &mut self,
        transaction: &Transaction,
        sender: &Address,
        burns: &[AssetTransferInput],
        inputs: &[AssetTransferInput],
        outputs: &[AssetTransferOutput],
    ) -> StateResult<()> {
        for (input, burn) in inputs.iter().map(|input| (input, false)).chain(burns.iter().map(|input| (input, true))) {
            let address = OwnedAssetAddress::new(input.prev_out.transaction_hash, input.prev_out.index, self.shard_id);
            let script_result = self.check_and_run_input_script(input, transaction, &input.prev_out, burn)?;
            match (script_result, burn) {
                (ScriptResult::Unlocked, false) => {}
                (ScriptResult::Burnt, true) => {}
                _ => return Err(TransactionError::FailedToUnlock(address.into()).into()),
            }
        }

        let mut deleted_asset = Vec::with_capacity(inputs.len());
        for input in inputs {
            let (_, asset_address) = self.check_input_asset(input, sender)?;
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
    ) -> StateResult<(OwnedAsset, OwnedAssetAddress)> {
        let asset_address =
            OwnedAssetAddress::new(input.prev_out.transaction_hash, input.prev_out.index, self.shard_id);
        let asset_scheme_address = AssetSchemeAddress::from_hash(input.prev_out.asset_type)
            .ok_or(TransactionError::AssetSchemeNotFound(input.prev_out.asset_type.into()))?;

        let asset_scheme = self
            .asset_scheme((&asset_scheme_address).into())?
            .ok_or(TransactionError::AssetSchemeNotFound(asset_scheme_address.into()))?;

        if let Some(ref registrar) = asset_scheme.registrar() {
            if registrar != sender {
                return Err(TransactionError::NotRegistrar(Mismatch {
                    expected: *registrar,
                    found: *sender,
                }).into())
            }
        }

        match self.asset(&asset_address)? {
            Some(asset) => {
                if asset.amount() != &input.prev_out.amount {
                    return Err(TransactionError::InvalidAssetAmount {
                        address: asset_address.into(),
                        expected: *asset.amount(),
                        got: input.prev_out.amount,
                    }.into())
                }
                Ok((asset, asset_address))
            }
            None => Err(TransactionError::AssetNotFound(asset_address.into()).into()),
        }
    }

    fn check_and_run_input_script(
        &self,
        input: &AssetTransferInput,
        transaction_hash: &PartialHashing,
        cur: &AssetOutPoint,
        burn: bool,
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
            }).into())
        }

        let script_result = match (decode(&input.lock_script), decode(&input.unlock_script)) {
            (Ok(lock_script), Ok(unlock_script)) => execute(
                &unlock_script,
                &asset.parameters(),
                &lock_script,
                transaction_hash,
                VMConfig::default(),
                cur,
                burn,
            ),
            // FIXME : Deliver full decode error
            _ => return Err(TransactionError::InvalidScript.into()),
        }.map_err(|err| {
            ctrace!(TX, "Cannot run unlock/lock script {:?}", err);
            TransactionError::FailedToUnlock(address_hash)
        })?;
        Ok(script_result)
    }

    fn compose_asset(
        &mut self,
        transaction: &Transaction,
        metadata: &String,
        registrar: &Option<Address>,
        inputs: &[AssetTransferInput],
        output: &AssetMintOutput,
        sender: &Address,
        shard_users: &[Address],
    ) -> StateResult<()> {
        let mut sum: HashMap<H256, u64> = HashMap::new();

        let mut deleted_assets: Vec<(H256, _)> = Vec::with_capacity(inputs.len());
        for input in inputs.iter() {
            let (_, asset_address) = self.check_input_asset(input, sender)?;
            let script_result = self.check_and_run_input_script(input, transaction, &input.prev_out, false)?;

            match script_result {
                ScriptResult::Unlocked => {}
                _ => return Err(TransactionError::FailedToUnlock(asset_address.into()).into()),
            }

            self.kill_asset(&asset_address);
            deleted_assets.push((asset_address.into(), input.prev_out.amount));

            let asset_type = input.prev_out.asset_type;
            let current_amount = sum.get(&asset_type).cloned().unwrap_or(0);
            sum.insert(asset_type.clone(), current_amount + input.prev_out.amount);
        }
        ctrace!(TX, "Deleted assets {:?}", deleted_assets);

        let pool = sum.into_iter().map(|(asset_type, amount)| Asset::new(asset_type, amount)).collect();

        self.mint_asset(
            transaction.hash(),
            metadata,
            &output.lock_script_hash,
            &output.parameters,
            &output.amount,
            registrar,
            sender,
            shard_users,
            pool,
        )
    }

    fn decompose_asset(
        &mut self,
        transaction: &Transaction,
        input: &AssetTransferInput,
        outputs: &[AssetTransferOutput],
        sender: &Address,
    ) -> StateResult<()> {
        let asset_type = input.prev_out.asset_type;
        let asset_scheme_address =
            AssetSchemeAddress::from_hash(asset_type).ok_or(TransactionError::AssetSchemeNotFound(asset_type.into()))?;
        let asset_scheme = self
            .asset_scheme((&asset_scheme_address).into())?
            .ok_or(TransactionError::AssetSchemeNotFound(asset_scheme_address.clone().into()))?;
        // The input asset should be composed asset
        if asset_scheme.pool().is_empty() {
            return Err(TransactionError::InvalidDecomposedInput {
                address: asset_type.clone(),
                got: 0,
            }.into())
        }

        // Check that the outputs are match with pool
        let mut sum: HashMap<H256, u64> = HashMap::new();
        for output in outputs {
            let output_type = output.asset_type;
            let current_amount = sum.get(&output_type).cloned().unwrap_or(0);
            sum.insert(output_type.clone(), current_amount + output.amount);
        }
        for asset in asset_scheme.pool() {
            match sum.remove(asset.asset_type()) {
                None => {
                    return Err(TransactionError::InvalidDecomposedOutput {
                        address: asset.asset_type().clone(),
                        expected: *asset.amount(),
                        got: 0,
                    }.into())
                }
                Some(value) => {
                    if value != *asset.amount() {
                        return Err(TransactionError::InvalidDecomposedOutput {
                            address: asset.asset_type().clone(),
                            expected: *asset.amount(),
                            got: value,
                        }.into())
                    }
                }
            }
        }
        if sum.len() != 0 {
            let mut invalid_assets: Vec<Asset> =
                sum.into_iter().map(|(asset_type, amount)| Asset::new(asset_type, amount)).collect();
            let invalid_asset = invalid_assets.pop().unwrap();
            return Err(TransactionError::InvalidDecomposedOutput {
                address: invalid_asset.asset_type().clone(),
                expected: 0,
                got: *invalid_asset.amount(),
            }.into())
        }


        let (_, asset_address) = self.check_input_asset(input, sender)?;
        let script_result = self.check_and_run_input_script(input, transaction, &input.prev_out, false)?;

        match script_result {
            ScriptResult::Unlocked => {}
            _ => return Err(TransactionError::FailedToUnlock(asset_address.into()).into()),
        }

        self.kill_asset(&asset_address);
        self.kill_asset_scheme(&asset_scheme_address);

        ctrace!(TX, "Deleted assets {:?} {:?}", asset_type.clone(), input.prev_out.amount);

        // Put asset into DB
        for (index, output) in outputs.iter().enumerate() {
            let asset_address = OwnedAssetAddress::new(transaction.hash(), index, self.shard_id);
            let mut asset = self.get_asset_mut(&asset_address)?;
            asset.init(output.asset_type, output.lock_script_hash, output.parameters.clone(), output.amount);
        }

        Ok(())
    }

    fn kill_asset(&mut self, account: &OwnedAssetAddress) {
        self.asset.remove(account);
    }

    fn kill_asset_scheme(&mut self, account: &AssetSchemeAddress) {
        self.asset_scheme.remove(account);
    }

    fn get_asset_scheme(&self, a: &AssetSchemeAddress) -> cmerkle::Result<Option<AssetScheme>> {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_global_cache = || self.db.get_cached_asset_scheme(a);
        self.asset_scheme.get(a, db, from_global_cache)
    }

    fn get_asset_scheme_mut(&self, a: &AssetSchemeAddress) -> cmerkle::Result<RefMut<AssetScheme>> {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_global_cache = || self.db.get_cached_asset_scheme(a);
        self.asset_scheme.get_mut(a, db, from_global_cache)
    }

    fn get_asset(&self, a: &OwnedAssetAddress) -> cmerkle::Result<Option<OwnedAsset>> {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_global_cache = || self.db.get_cached_asset(a);
        self.asset.get(a, db, from_global_cache)
    }

    fn get_asset_mut(&self, a: &OwnedAssetAddress) -> cmerkle::Result<RefMut<OwnedAsset>> {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_global_cache = || self.db.get_cached_asset(a);
        self.asset.get_mut(a, db, from_global_cache)
    }
}

impl<B: Backend + ShardBackend> ShardStateInfo for ShardLevelState<B> {
    fn root(&self) -> &H256 {
        &self.root
    }

    fn asset_scheme(&self, a: &AssetSchemeAddress) -> cmerkle::Result<Option<AssetScheme>> {
        self.get_asset_scheme(a)
    }

    fn asset(&self, a: &OwnedAssetAddress) -> cmerkle::Result<Option<OwnedAsset>> {
        self.get_asset(a)
    }
}

impl<B> StateWithCheckpoint for ShardLevelState<B> {
    fn create_checkpoint(&mut self, id: CheckpointId) {
        ctrace!(STATE, "Checkpoint({}) for shard({}) is created", id, self.shard_id);
        self.id_of_checkpoints.push(id);
        self.asset_scheme.checkpoint();
        self.asset.checkpoint();
    }

    fn discard_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        ctrace!(STATE, "Checkpoint({}) for shard({}) is discarded", id, self.shard_id);
        self.asset_scheme.discard_checkpoint();
        self.asset.discard_checkpoint();
    }

    fn revert_to_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        ctrace!(STATE, "Checkpoint({}) for shard({}) is reverted", id, self.shard_id);
        self.asset_scheme.revert_to_checkpoint();
        self.asset.revert_to_checkpoint();
    }
}

impl<B: Backend + ShardBackend> StateWithCache for ShardLevelState<B> {
    fn commit(&mut self) -> TrieResult<()> {
        let mut trie = TrieFactory::from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
        self.asset_scheme.commit(&mut trie)?;
        self.asset.commit(&mut trie)?;
        Ok(())
    }

    fn propagate_to_global_cache(&mut self) {
        let ref mut db = self.db;
        self.asset_scheme.propagate_to_global_cache(|address, item, modified| {
            db.add_to_asset_scheme_cache(address, item, modified);
        });
        self.asset.propagate_to_global_cache(|address, item, modified| {
            db.add_to_asset_cache(address, item, modified);
        });
    }

    fn clear(&mut self) {
        self.asset_scheme.clear();
        self.asset.clear();
    }
}

impl<B: ShardBackend> fmt::Debug for ShardLevelState<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "asset_scheme: {:?} asset: {:?}", self.asset_scheme, self.asset)
    }
}

const TRANSACTION_CHECKPOINT: CheckpointId = 456;

impl<B: Backend + ShardBackend> ShardState<B> for ShardLevelState<B> {
    fn apply(
        &mut self,
        transaction: &Transaction,
        sender: &Address,
        shard_users: &[Address],
    ) -> StateResult<TransactionInvoice> {
        ctrace!(TX, "Execute {:?}(TxHash:{:?})", transaction, transaction.hash());

        self.create_checkpoint(TRANSACTION_CHECKPOINT);
        let result = self.apply_internal(transaction, sender, shard_users);
        match result {
            Ok(_) => {
                cinfo!(TX, "Tx({}) is applied", transaction.hash());
                self.discard_checkpoint(TRANSACTION_CHECKPOINT);
                self.commit()?; // FIXME: Remove early commit.
                Ok(TransactionInvoice::Success)
            }
            Err(StateError::Transaction(err)) => {
                cinfo!(TX, "Cannot apply Tx({}): {:?}", transaction.hash(), err);
                self.revert_to_checkpoint(TRANSACTION_CHECKPOINT);
                Ok(TransactionInvoice::Fail(err))
            }
            Err(err) => {
                self.revert_to_checkpoint(TRANSACTION_CHECKPOINT);
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::helpers::get_temp_state_db;
    use super::super::super::StateDB;
    use ctypes::transaction::{AssetOutPoint, AssetTransferInput, AssetTransferOutput, Error as TransactionError};

    use super::*;

    fn address() -> Address {
        Address::random()
    }

    fn get_temp_shard_state(shard_id: ShardId) -> ShardLevelState<StateDB> {
        let state_db = get_temp_state_db();
        let root_parent = H256::random();

        let state_db = state_db.clone_canon(&root_parent);
        ShardLevelState::try_new(shard_id, state_db).unwrap()
    }

    #[test]
    fn mint_permissioned_asset() {
        let shard_id = 0;
        let sender = address();
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let amount = 100;
        let registrar = Some(Address::random());
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
            nonce: 0,
        };

        let result = state.apply(&transaction, &sender, &[sender]);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_scheme_address.into(), lock_script_hash, parameters, amount))), asset);
    }

    #[test]
    fn mint_infinite_asset() {
        let shard_id = 0;
        let sender = address();
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            registrar,
            nonce: 0,
        };

        let result = state.apply(&transaction, &sender, &[sender]);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, registrar))), asset_scheme);

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
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            registrar,
            nonce: 0,
        };

        let result = state.apply(&transaction, &sender, &[sender]);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let result = state.apply(&transaction, &sender, &[sender]);
        assert_eq!(Ok(TransactionInvoice::Fail(TransactionError::AssetSchemeDuplicated(transaction.hash()))), result);
    }

    #[test]
    fn invalid_registrar() {
        let shard_id = 0;
        let network_id = "tc".into();
        let sender = address();
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let registrar = Some(Address::random());
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
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint, &sender, &[sender]));

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

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
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash,
                parameters: vec![],
                asset_type,
                amount: 30,
            }],
            nonce: 0,
        };

        assert_eq!(
            Ok(TransactionInvoice::Fail(TransactionError::NotRegistrar(Mismatch {
                expected: registrar.unwrap(),
                found: sender,
            }))),
            state.apply(&transfer, &sender, &[sender])
        );
    }

    #[test]
    fn mint_and_transfer() {
        let network_id = "tc".into();
        let shard_id = 0;
        let sender = address();
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
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
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();

        let network_id = "tc".into();

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint, &sender, &[sender]));

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

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
            nonce: 0,
        };
        let transfer_hash = transfer.hash();

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&transfer, &sender, &[sender]));

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
    fn mint_and_compose() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state = get_temp_shard_state(shard_id);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
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
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint, &sender, &[]));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.into();

        let random_lock_script_hash = H160::random();
        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            nonce: 0,
            metadata: "composed".to_string(),
            registrar,
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount: 30,
                },
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

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&compose, &sender, &[]));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_scheme = state.asset_scheme(&composed_asset_scheme_address);
        let composed_asset_type = composed_asset_scheme_address.into();

        assert_eq!(
            Ok(Some(AssetScheme::new_with_pool(
                "composed".to_string(),
                1,
                registrar,
                vec![Asset::new(asset_type, 30)]
            ))),
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
        let mut state = get_temp_shard_state(shard_id);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
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
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint, &sender, &[]));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.clone().into();

        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            nonce: 0,
            metadata: "composed".to_string(),
            registrar,
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount,
                },
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

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&compose, &sender, &[]));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_scheme = state.asset_scheme(&composed_asset_scheme_address);
        let composed_asset_type = composed_asset_scheme_address.into();

        assert_eq!(
            Ok(Some(AssetScheme::new_with_pool(
                "composed".to_string(),
                1,
                registrar,
                vec![Asset::new(asset_type, 30)]
            ))),
            composed_asset_scheme
        );

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, lock_script_hash, vec![], 1))), composed_asset);

        let random_lock_script_hash = H160::random();
        let decompose = Transaction::AssetDecompose {
            network_id,
            nonce: 0,
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: compose_hash,
                    index: 0,
                    asset_type: composed_asset_type,
                    amount: 1,
                },
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

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&decompose, &sender, &[]));

        let asset_scheme = state.asset_scheme(&asset_scheme_address);

        assert_eq!(Ok(Some(AssetScheme::new("metadata".to_string(), 30, registrar))), asset_scheme);

        let decomposed_asset_address = OwnedAssetAddress::new(decompose_hash, 0, shard_id);
        let decomposed_asset = state.asset(&decomposed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(asset_type, random_lock_script_hash, vec![], 30))), decomposed_asset);
    }

    #[test]
    fn decompose_fail_invalid_input_different_asset_type() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state = get_temp_shard_state(shard_id);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
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
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint, &sender, &[]));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.clone().into();

        let mint2 = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: "invalid_asset".to_string(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
            registrar,
            nonce: 0,
        };
        let mint2_hash = mint2.hash();
        let asset_scheme_address2 = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type2 = asset_scheme_address2.clone().into();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint2, &sender, &[]));

        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            nonce: 0,
            metadata: "composed".to_string(),
            registrar,
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint_hash,
                    index: 0,
                    asset_type,
                    amount,
                },
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

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&compose, &sender, &[]));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_scheme = state.asset_scheme(&composed_asset_scheme_address);
        let composed_asset_type = composed_asset_scheme_address.into();

        assert_eq!(
            Ok(Some(AssetScheme::new_with_pool(
                "composed".to_string(),
                1,
                registrar,
                vec![Asset::new(asset_type, 30)]
            ))),
            composed_asset_scheme
        );

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, lock_script_hash, vec![], 1))), composed_asset);

        let random_lock_script_hash = H160::random();
        let decompose = Transaction::AssetDecompose {
            network_id,
            nonce: 0,
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: mint2_hash,
                    index: 0,
                    asset_type: asset_type2,
                    amount: 1,
                },
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
            Ok(TransactionInvoice::Fail(TransactionError::InvalidDecomposedInput {
                address: asset_type,
                got: 0
            })),
            state.apply(&decompose, &sender, &[])
        );
    }

    #[test]
    fn decompose_fail_invalid_output_insufficient_output() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state = get_temp_shard_state(shard_id);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
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
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint, &sender, &[]));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.clone().into();

        let mint2 = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: "invalid_asset".to_string(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
            registrar,
            nonce: 0,
        };
        let mint2_hash = mint2.hash();
        let asset_scheme_address2 = AssetSchemeAddress::new(mint2_hash, shard_id);
        let asset_type2 = asset_scheme_address2.clone().into();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint2, &sender, &[]));

        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            nonce: 0,
            metadata: "composed".to_string(),
            registrar,
            inputs: vec![
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: mint_hash,
                        index: 0,
                        asset_type,
                        amount,
                    },
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

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&compose, &sender, &[]));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_type = composed_asset_scheme_address.into();

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, lock_script_hash, vec![], 1))), composed_asset);

        let random_lock_script_hash = H160::random();
        let decompose = Transaction::AssetDecompose {
            network_id,
            nonce: 0,
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: compose_hash,
                    index: 0,
                    asset_type: composed_asset_type,
                    amount: 1,
                },
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
            Ok(TransactionInvoice::Fail(TransactionError::InvalidDecomposedOutput {
                address: asset_type2,
                expected: 1,
                got: 0
            })),
            state.apply(&decompose, &sender, &[])
        );
    }


    #[test]
    fn decompose_fail_invalid_output_insufficient_amount() {
        let network_id = "tc".into();
        let shard_id = 0;
        let mut state = get_temp_shard_state(shard_id);
        let sender = address();

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("0xb042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
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
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint, &sender, &[]));
        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_type = asset_scheme_address.clone().into();

        let mint2 = Transaction::AssetMint {
            network_id,
            shard_id,
            metadata: "invalid_asset".to_string(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: vec![],
                amount: Some(1),
            },
            registrar,
            nonce: 0,
        };
        let mint2_hash = mint2.hash();
        let asset_scheme_address2 = AssetSchemeAddress::new(mint2_hash, shard_id);
        let asset_type2 = asset_scheme_address2.clone().into();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint2, &sender, &[]));

        let compose = Transaction::AssetCompose {
            network_id,
            shard_id,
            nonce: 0,
            metadata: "composed".to_string(),
            registrar,
            inputs: vec![
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: mint_hash,
                        index: 0,
                        asset_type,
                        amount,
                    },
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

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&compose, &sender, &[]));

        let composed_asset_scheme_address = AssetSchemeAddress::new(compose_hash, shard_id);
        let composed_asset_type = composed_asset_scheme_address.into();

        let composed_asset_address = OwnedAssetAddress::new(compose_hash, 0, shard_id);
        let composed_asset = state.asset(&composed_asset_address);
        assert_eq!(Ok(Some(OwnedAsset::new(composed_asset_type, lock_script_hash, vec![], 1))), composed_asset);

        let random_lock_script_hash = H160::random();
        let decompose = Transaction::AssetDecompose {
            network_id,
            nonce: 0,
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: compose_hash,
                    index: 0,
                    asset_type: composed_asset_type,
                    amount: 1,
                },
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
            Ok(TransactionInvoice::Fail(TransactionError::InvalidDecomposedOutput {
                address: asset_type,
                expected: 30,
                got: 10
            })),
            state.apply(&decompose, &sender, &[])
        );
    }

    #[test]
    fn mint_and_failed_transfer_and_successful_transfer() {
        let network_id = "tc".into();
        let shard_id = 0;

        let sender = address();
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::from("b042ad154a3359d276835c903587ebafefea22af");
        let registrar = None;
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
            registrar,
            nonce: 0,
        };
        let mint_hash = mint.hash();

        let network_id = "tc".into();

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&mint, &sender, &[sender]));

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

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
                lock_script: failed_lock_script.clone(),
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash,
                parameters: vec![vec![1]],
                asset_type,
                amount: 30,
            }],
            nonce: 0,
        };

        let sender = address();
        let failed_invoice = state.apply(&failed_transfer, &sender, &[sender]).unwrap();
        assert_eq!(
            TransactionInvoice::Fail(TransactionError::ScriptHashMismatch(Mismatch {
                expected: lock_script_hash,
                found: Blake::blake(&failed_lock_script),
            })),
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
            nonce: 0,
        };
        let successful_transfer_hash = successful_transfer.hash();

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(&successful_transfer, &sender, &[sender]));

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
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            registrar,
            nonce: 0,
        };

        let result = state.apply(&transaction, &sender, &[sender]);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, registrar))), asset_scheme);

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
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            registrar,
            nonce: 0,
        };

        let shard_user = address();
        let result = state.apply(&transaction, &sender, &[shard_user]);
        assert_eq!(Ok(TransactionInvoice::Fail(TransactionError::InsufficientPermission)), result);

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
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H160::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id,
            metadata: metadata.clone(),
            output: AssetMintOutput {
                lock_script_hash,
                parameters: parameters.clone(),
                amount: None,
            },
            registrar,
            nonce: 0,
        };

        let result = state.apply(&transaction, &sender, &[]);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, registrar))), asset_scheme);

        let asset_address = OwnedAssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(
            Ok(Some(OwnedAsset::new(asset_scheme_address.into(), lock_script_hash, parameters, ::std::u64::MAX))),
            asset
        );
    }
}
