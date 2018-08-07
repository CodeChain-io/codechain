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
use std::fmt;

use ccrypto::{Blake, BLAKE_NULL_RLP};
use ckey::Address;
use cmerkle::{self, Result as TrieResult, Trie, TrieError, TrieFactory};
use ctypes::invoice::TransactionInvoice;
use ctypes::transaction::{
    AssetMintOutput, AssetTransferInput, AssetTransferOutput, Error as TransactionError, Transaction,
};
use ctypes::util::unexpected::Mismatch;
use ctypes::{ShardId, WorldId};
use cvm::{decode, execute, ScriptResult, VMConfig};
use primitives::{Bytes, H256};
use rlp::Encodable;

use super::super::backend::{Backend, ShardBackend};
use super::super::checkpoint::{CheckpointId, StateWithCheckpoint};
use super::super::item::cache::Cache;
use super::super::traits::{ShardState, ShardStateInfo, StateWithCache};
use super::super::{
    Asset, AssetAddress, AssetScheme, AssetSchemeAddress, ShardMetadata, ShardMetadataAddress, World, WorldAddress,
};
use super::super::{StateDB, StateError, StateResult};


pub struct ShardLevelState<B> {
    db: B,
    root: H256,
    metadata: Cache<ShardMetadata>,
    world: Cache<World>,
    asset_scheme: Cache<AssetScheme>,
    asset: Cache<Asset>,
    id_of_checkpoints: Vec<CheckpointId>,
    shard_id: ShardId,
}

impl<B: Backend + ShardBackend> ShardLevelState<B> {
    /// Creates new state with empty state root
    pub fn try_new(shard_id: ShardId, mut db: B) -> cmerkle::Result<ShardLevelState<B>> {
        let mut root = BLAKE_NULL_RLP;

        {
            let mut t = TrieFactory::from_existing(db.as_hashdb_mut(), &mut root)?;

            let metadata = ShardMetadata::new(0);
            let address = ShardMetadataAddress::new(shard_id);

            let r = t.insert(&*address, &metadata.rlp_bytes());
            debug_assert_eq!(Ok(None), r);
            r?;
        }

        Ok(ShardLevelState {
            db,
            root,
            metadata: Cache::new(),
            world: Cache::new(),
            asset_scheme: Cache::new(),
            asset: Cache::new(),
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
            metadata: Cache::new(),
            world: Cache::new(),
            asset_scheme: Cache::new(),
            asset: Cache::new(),
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
        shard_id: ShardId,
        transaction: &Transaction,
        sender: &Address,
        shard_owner: &Address,
    ) -> StateResult<()> {
        debug_assert_eq!(Ok(()), transaction.verify());
        match transaction {
            Transaction::CreateWorld {
                nonce,
                owners,
                ..
            } => Ok(self.create_world(shard_id, nonce, owners)?),
            Transaction::SetWorldOwners {
                shard_id,
                world_id,
                nonce,
                owners,
                ..
            } => Ok(self.set_world_owners(*shard_id, *world_id, *nonce, &owners, sender, shard_owner)?),
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
            } => Ok(self.mint_asset(transaction.hash(), metadata, lock_script_hash, parameters, amount, registrar)?),
            Transaction::AssetTransfer {
                burns,
                inputs,
                outputs,
                ..
            } => self.transfer_asset(&transaction, burns, inputs, outputs),
        }
    }

    fn create_world(&mut self, shard_id: ShardId, nonce: &u64, owners: &Vec<Address>) -> StateResult<()> {
        let metadata_address = ShardMetadataAddress::new(shard_id);
        let mut metadata = self.require_metadata(&metadata_address, || unreachable!("Shard must have metadata"))?;

        let current_nonce = *metadata.nonce();
        if *nonce != current_nonce {
            return Err(TransactionError::InvalidShardNonce(Mismatch {
                expected: current_nonce,
                found: *nonce,
            }).into())
        }

        let world_id = *metadata.number_of_worlds();
        let world_address = WorldAddress::new(shard_id, world_id);

        metadata.increase_nonce();
        metadata.increase_number_of_worlds();

        self.require_world(&world_address, || World::new(owners.clone()))?;
        Ok(())
    }

    fn set_world_owners(
        &mut self,
        shard_id: ShardId,
        world_id: WorldId,
        nonce: u64,
        owners: &[Address],
        sender: &Address,
        shard_owner: &Address,
    ) -> StateResult<()> {
        let world: World = self.world(world_id)?.ok_or_else(|| TransactionError::InvalidWorldId(world_id))?;

        if shard_owner != sender && !world.world_owners().contains(sender) {
            return Err(TransactionError::InsufficientPermission.into())
        }

        let current_nonce = world.nonce();
        if current_nonce != &nonce {
            return Err(TransactionError::InvalidWorldNonce(Mismatch {
                expected: *current_nonce,
                found: nonce,
            }).into())
        }

        let mut world = self.require_world(&WorldAddress::new(shard_id, world_id), || unreachable!())?;
        world.inc_nonce();
        world.set_owners(owners.to_vec());
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
    ) -> StateResult<()> {
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, self.shard_id);
        let amount = amount.unwrap_or(::std::u64::MAX);
        let asset_scheme = self.require_asset_scheme(&asset_scheme_address, || {
            AssetScheme::new(metadata.clone(), amount, registrar.clone())
        })?;
        ctrace!(TX, "{:?} is minted on {:?}", asset_scheme, asset_scheme_address);

        let asset_address = AssetAddress::new(transaction_hash, 0, self.shard_id);
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
    ) -> StateResult<()> {
        for (input, burn) in inputs.iter().map(|input| (input, false)).chain(burns.iter().map(|input| (input, true))) {
            let (address_hash, asset) = {
                let index = input.prev_out.index;
                let address = AssetAddress::new(input.prev_out.transaction_hash, index, self.shard_id);
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
            let address = AssetAddress::new(input.prev_out.transaction_hash, index, self.shard_id);

            let asset_type = input.prev_out.asset_type.clone();
            let asset_scheme_address = AssetSchemeAddress::from_hash(asset_type)
                .ok_or(TransactionError::AssetSchemeNotFound(asset_type.into()))?;
            let _asset_scheme = self
                .asset_scheme((&asset_scheme_address).into())?
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
            let asset_address = AssetAddress::new(transaction.hash(), index, self.shard_id);
            let asset =
                Asset::new(output.asset_type, output.lock_script_hash, output.parameters.clone(), output.amount);
            self.require_asset(&asset_address, || asset)?;
            created_asset.push((asset_address, output.amount));
        }
        ctrace!(TX, "Deleted assets {:?}", deleted_asset);
        ctrace!(TX, "Created assets {:?}", created_asset);
        Ok(())
    }

    fn kill_asset(&mut self, account: &AssetAddress) {
        self.asset.remove(account);
    }

    fn require_metadata<'a, F>(
        &'a self,
        a: &ShardMetadataAddress,
        default: F,
    ) -> cmerkle::Result<RefMut<'a, ShardMetadata>>
    where
        F: FnOnce() -> ShardMetadata, {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_shard_metadata(a);
        self.metadata.require_item_or_from(a, default, db, from_db)
    }

    fn require_world<'a, F>(&'a self, a: &WorldAddress, default: F) -> cmerkle::Result<RefMut<'a, World>>
    where
        F: FnOnce() -> World, {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_world(a);
        self.world.require_item_or_from(a, default, db, from_db)
    }

    fn require_asset_scheme<'a, F>(
        &'a self,
        a: &AssetSchemeAddress,
        default: F,
    ) -> cmerkle::Result<RefMut<'a, AssetScheme>>
    where
        F: FnOnce() -> AssetScheme, {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_asset_scheme(a);
        self.asset_scheme.require_item_or_from(a, default, db, from_db)
    }

    fn require_asset<'a, F>(&'a self, a: &AssetAddress, default: F) -> cmerkle::Result<RefMut<'a, Asset>>
    where
        F: FnOnce() -> Asset, {
        let db = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_asset(a);
        self.asset.require_item_or_from(a, default, db, from_db)
    }
}

impl<B: Backend + ShardBackend> ShardStateInfo for ShardLevelState<B> {
    fn root(&self) -> &H256 {
        &self.root
    }

    fn metadata(&self) -> cmerkle::Result<Option<ShardMetadata>> {
        let a = ShardMetadataAddress::new(self.shard_id);
        let cached_metadata = self.db.get_cached_shard_metadata(&a).and_then(|metadata| metadata);
        if cached_metadata.is_some() {
            return Ok(cached_metadata)
        }

        let trie = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        Ok(trie.get_with(a.as_ref(), ::rlp::decode::<ShardMetadata>)?)
    }

    fn world(&self, world_id: WorldId) -> cmerkle::Result<Option<World>> {
        let a = WorldAddress::new(self.shard_id, world_id);
        let cached_world = self.db.get_cached_world(&a).and_then(|world| world);
        if cached_world.is_some() {
            return Ok(cached_world)
        }

        let trie = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        Ok(trie.get_with(a.as_ref(), ::rlp::decode::<World>)?)
    }

    fn asset_scheme(&self, a: &AssetSchemeAddress) -> cmerkle::Result<Option<AssetScheme>> {
        let cached_asset = self.db.get_cached_asset_scheme(&a).and_then(|asset_scheme| asset_scheme);
        if cached_asset.is_some() {
            return Ok(cached_asset)
        }

        let trie = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        Ok(trie.get_with(a.as_ref(), ::rlp::decode::<AssetScheme>)?)
    }

    fn asset(&self, a: &AssetAddress) -> cmerkle::Result<Option<Asset>> {
        let cached_asset = self.db.get_cached_asset(&a).and_then(|asset| asset);
        if cached_asset.is_some() {
            return Ok(cached_asset)
        }

        let trie = TrieFactory::readonly(self.db.as_hashdb(), &self.root)?;
        Ok(trie.get_with(a.as_ref(), ::rlp::decode::<Asset>)?)
    }
}

impl<B> StateWithCheckpoint for ShardLevelState<B> {
    fn create_checkpoint(&mut self, id: CheckpointId) {
        self.id_of_checkpoints.push(id);
        self.metadata.checkpoint();
        self.world.checkpoint();
        self.asset_scheme.checkpoint();
        self.asset.checkpoint();
    }

    fn discard_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.metadata.discard_checkpoint();
        self.world.discard_checkpoint();
        self.asset_scheme.discard_checkpoint();
        self.asset.discard_checkpoint();
    }

    fn revert_to_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.metadata.revert_to_checkpoint();
        self.world.revert_to_checkpoint();
        self.asset_scheme.revert_to_checkpoint();
        self.asset.revert_to_checkpoint();
    }
}

impl<B: Backend + ShardBackend> StateWithCache for ShardLevelState<B> {
    fn commit(&mut self) -> TrieResult<()> {
        let mut trie = TrieFactory::from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
        self.metadata.commit(&mut trie)?;
        self.world.commit(&mut trie)?;
        self.asset_scheme.commit(&mut trie)?;
        self.asset.commit(&mut trie)?;
        Ok(())
    }

    fn propagate_to_global_cache(&mut self) {
        let ref mut db = self.db;
        self.metadata.propagate_to_global_cache(|address, item, modified| {
            db.add_to_shard_metadata_cache(address, item, modified);
        });
        self.world.propagate_to_global_cache(|address, item, modified| {
            db.add_to_world_cache(address, item, modified);
        });
        self.asset_scheme.propagate_to_global_cache(|address, item, modified| {
            db.add_to_asset_scheme_cache(address, item, modified);
        });
        self.asset.propagate_to_global_cache(|address, item, modified| {
            db.add_to_asset_cache(address, item, modified);
        });
    }

    fn clear(&mut self) {
        self.metadata.clear();
        self.world.clear();
        self.asset_scheme.clear();
        self.asset.clear();
    }
}

impl<B: ShardBackend> fmt::Debug for ShardLevelState<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "metadata: {:?} world: {:?} asset_scheme: {:?} asset: {:?}",
            self.metadata, self.world, self.asset_scheme, self.asset
        )
    }
}

// TODO: cloning for `ShardLevelState` shouldn't be possible in general; Remove this and use
// checkpoints where possible.
impl Clone for ShardLevelState<StateDB> {
    fn clone(&self) -> ShardLevelState<StateDB> {
        ShardLevelState {
            db: self.db.clone(),
            root: self.root.clone(),
            id_of_checkpoints: self.id_of_checkpoints.clone(),
            metadata: self.metadata.clone(),
            world: self.world.clone(),
            asset_scheme: self.asset_scheme.clone(),
            asset: self.asset.clone(),
            shard_id: self.shard_id,
        }
    }
}

const TRANSACTION_CHECKPOINT: CheckpointId = 456;

impl<B: Backend + ShardBackend> ShardState<B> for ShardLevelState<B> {
    fn apply(
        &mut self,
        shard_id: ShardId,
        transaction: &Transaction,
        sender: &Address,
        shard_owner: &Address,
    ) -> StateResult<TransactionInvoice> {
        ctrace!(TX, "Execute {:?}(TxHash:{:?})", transaction, transaction.hash());

        self.create_checkpoint(TRANSACTION_CHECKPOINT);
        let result = self.apply_internal(shard_id, transaction, sender, shard_owner);
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
    fn create_world_without_owners() {
        let network_id = 0xDEADBEEF;
        let shard_id = 0xCAFE;
        let mut state = get_temp_shard_state(shard_id);

        let nonce = 0;
        let owners = vec![];

        let transaction = Transaction::CreateWorld {
            network_id,
            shard_id,
            nonce,
            owners: owners.clone(),
        };

        let sender = address();
        let shard_owner = address();
        let result = state.apply(shard_id, &transaction, &sender, &shard_owner);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let metadata = state.metadata();
        assert_eq!(Ok(Some(ShardMetadata::new_with_nonce(1, 1))), metadata);

        let world_id = 0;
        let world = state.world(world_id);
        assert_eq!(Ok(Some(World::new(owners))), world);
    }

    #[test]
    fn create_world_with_owners() {
        let network_id = 0xDEADBEEF;
        let shard_id = 0xCAFE;
        let mut state = get_temp_shard_state(shard_id);

        let nonce = 0;
        let owners = vec![Address::random(), Address::random(), Address::random()];

        let transaction = Transaction::CreateWorld {
            network_id,
            shard_id,
            nonce,
            owners: owners.clone(),
        };

        let sender = address();
        let shard_owner = address();
        let result = state.apply(shard_id, &transaction, &sender, &shard_owner);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let metadata = state.metadata();
        assert_eq!(Ok(Some(ShardMetadata::new_with_nonce(1, 1))), metadata);

        let world_id = 0;
        let world = state.world(world_id);
        assert_eq!(Ok(Some(World::new(owners))), world);
    }

    #[test]
    fn create_world_fail_if_nonce_is_not_matched() {
        let network_id = 0xDEADBEEF;
        let shard_id = 0xCAFE;
        let mut state = get_temp_shard_state(shard_id);

        let nonce = 1;
        let owners = vec![];

        let transaction = Transaction::CreateWorld {
            network_id,
            shard_id,
            nonce,
            owners: owners.clone(),
        };

        let sender = address();
        let shard_owner = address();
        assert_eq!(
            Ok(TransactionInvoice::Fail(TransactionError::InvalidShardNonce(Mismatch {
                expected: 0,
                found: 1
            }))),
            state.apply(shard_id, &transaction, &sender, &shard_owner)
        );

        let metadata = state.metadata();
        assert_eq!(Ok(Some(ShardMetadata::new_with_nonce(0, 0))), metadata);

        let world_id = 0;
        let world = state.world(world_id);
        assert_eq!(Ok(None), world);
    }

    #[test]
    fn mint_permissioned_asset() {
        let shard_id = 0;
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let amount = 100;
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: 200,
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

        let sender = address();
        let shard_owner = address();
        let result = state.apply(shard_id, &transaction, &sender, &shard_owner);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset_address = AssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(Asset::new(asset_scheme_address.into(), lock_script_hash, parameters, amount))), asset);
    }

    #[test]
    fn mint_infinite_asset() {
        let shard_id = 0;
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            network_id: 200,
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

        let sender = address();
        let shard_owner = address();
        let result = state.apply(shard_id, &transaction, &sender, &shard_owner);
        assert_eq!(Ok(TransactionInvoice::Success), result);

        let transaction_hash = transaction.hash();
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), ::std::u64::MAX, registrar))), asset_scheme);

        let asset_address = AssetAddress::new(transaction_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(
            Ok(Some(Asset::new(asset_scheme_address.into(), lock_script_hash, parameters, ::std::u64::MAX))),
            asset
        );
    }

    #[test]
    fn mint_and_transfer() {
        let shard_id = 0;
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050");
        let registrar = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id: 200,
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

        let network_id = 0xCafe;

        let sender = address();
        let shard_owner = address();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(shard_id, &mint, &sender, &shard_owner));

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset_address = AssetAddress::new(mint_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], amount))), asset);

        let random_lock_script_hash = H256::random();
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

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(shard_id, &transfer, &sender, &shard_owner));

        let asset0_address = AssetAddress::new(transfer_hash, 0, shard_id);
        let asset0 = state.asset(&asset0_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = AssetAddress::new(transfer_hash, 1, shard_id);
        let asset1 = state.asset(&asset1_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], 5))), asset1);

        let asset2_address = AssetAddress::new(transfer_hash, 2, shard_id);
        let asset2 = state.asset(&asset2_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);
    }

    #[test]
    fn mint_and_failed_transfer_and_successful_transfer() {
        let shard_id = 0;
        let mut state = get_temp_shard_state(shard_id);

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::from("07feab4c39250abf60b77d7589a5b61fdf409bd837e936376381d19db1e1f050");
        let registrar = None;
        let amount = 30;
        let mint = Transaction::AssetMint {
            network_id: 200,
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

        let network_id = 0xCafe;

        let sender = address();
        let shard_owner = address();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(shard_id, &mint, &sender, &shard_owner));

        let asset_scheme_address = AssetSchemeAddress::new(mint_hash, shard_id);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        let asset_type = asset_scheme_address.into();

        assert_eq!(Ok(Some(AssetScheme::new(metadata.clone(), amount, registrar))), asset_scheme);

        let asset_address = AssetAddress::new(mint_hash, 0, shard_id);
        let asset = state.asset(&asset_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], amount))), asset);

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
        let shard_owner = address();
        let failed_invoice = state.apply(shard_id, &failed_transfer, &sender, &shard_owner).unwrap();
        assert_eq!(
            TransactionInvoice::Fail(TransactionError::ScriptHashMismatch(Mismatch {
                expected: lock_script_hash,
                found: Blake::blake(&failed_lock_script),
            })),
            failed_invoice
        );

        let random_lock_script_hash = H256::random();
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

        assert_eq!(Ok(TransactionInvoice::Success), state.apply(shard_id, &successful_transfer, &sender, &shard_owner));

        let asset0_address = AssetAddress::new(successful_transfer_hash, 0, shard_id);
        let asset0 = state.asset(&asset0_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![vec![1]], 10))), asset0);

        let asset1_address = AssetAddress::new(successful_transfer_hash, 1, shard_id);
        let asset1 = state.asset(&asset1_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, lock_script_hash, vec![], 5))), asset1);

        let asset2_address = AssetAddress::new(successful_transfer_hash, 2, shard_id);
        let asset2 = state.asset(&asset2_address);
        assert_eq!(Ok(Some(Asset::new(asset_type, random_lock_script_hash, vec![], 15))), asset2);
    }

    #[test]
    fn shard_owner_can_set_world_owners() {
        let network_id = 0xDEADBEEF;
        let shard_id = 0xCAFE;
        let mut state = get_temp_shard_state(shard_id);

        let owners = vec![Address::random(), Address::random()];
        assert_eq!(Ok(()), state.create_world(shard_id, &0, &owners));
        assert_eq!(Ok(()), state.commit());

        let metadata = state.metadata();
        assert_eq!(Ok(Some(ShardMetadata::new_with_nonce(1, 1))), metadata);

        let world_id = 0;
        let world = state.world(world_id);
        assert_eq!(Ok(Some(World::new(owners))), world);

        let nonce = 0;

        let new_owners = vec![Address::random(), Address::random(), Address::random()];
        let transaction = Transaction::SetWorldOwners {
            network_id,
            shard_id,
            world_id,
            nonce,
            owners: new_owners.clone(),
        };

        let shard_owner = {
            loop {
                let owner = address();
                if !new_owners.contains(&owner) {
                    break owner
                }
            }
        };
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(shard_id, &transaction, &shard_owner, &shard_owner));

        let world = state.world(world_id);
        assert_eq!(Ok(Some(World::new_with_nonce(new_owners, 1))), world);
    }

    #[test]
    fn world_owner_can_set_world_owners() {
        let network_id = 0xDEADBEEF;
        let shard_id = 0xCAFE;
        let mut state = get_temp_shard_state(shard_id);

        let sender = Address::random();
        let old_owners = vec![sender, Address::random()];
        assert_eq!(Ok(()), state.create_world(shard_id, &0, &old_owners));
        assert_eq!(Ok(()), state.commit());

        let metadata = state.metadata();
        assert_eq!(Ok(Some(ShardMetadata::new_with_nonce(1, 1))), metadata);

        let world_id = 0;
        let world = state.world(world_id);
        assert_eq!(Ok(Some(World::new(old_owners.clone()))), world);

        let nonce = 0;

        let owners = vec![Address::random(), Address::random(), Address::random()];
        let transaction = Transaction::SetWorldOwners {
            network_id,
            shard_id,
            world_id,
            nonce,
            owners: owners.clone(),
        };

        let shard_owner = Address::random();
        assert_eq!(Ok(TransactionInvoice::Success), state.apply(shard_id, &transaction, &sender, &shard_owner));

        let world = state.world(world_id);
        assert_eq!(Ok(Some(World::new_with_nonce(owners, 1))), world);
    }


    #[test]
    fn insufficient_permission_must_fail_to_set_world_owners() {
        let network_id = 0xDEADBEEF;
        let shard_id = 0xCAFE;
        let mut state = get_temp_shard_state(shard_id);

        let owners = vec![Address::random(), Address::random()];
        assert_eq!(Ok(()), state.create_world(shard_id, &0, &owners));
        assert_eq!(Ok(()), state.commit());

        let metadata = state.metadata();
        assert_eq!(Ok(Some(ShardMetadata::new_with_nonce(1, 1))), metadata);

        let world_id = 0;
        let world = state.world(world_id);
        assert_eq!(Ok(Some(World::new(owners.clone()))), world);

        let nonce = 0;

        let new_owners = vec![Address::random(), Address::random(), Address::random()];
        let transaction = Transaction::SetWorldOwners {
            network_id,
            shard_id,
            world_id,
            nonce,
            owners: new_owners.clone(),
        };

        let sender = {
            loop {
                let owner = address();
                if !new_owners.contains(&owner) {
                    break owner
                }
            }
        };
        let shard_owner = address();
        assert_eq!(
            Ok(TransactionInvoice::Fail(TransactionError::InsufficientPermission)),
            state.apply(shard_id, &transaction, &sender, &shard_owner)
        );
        let world = state.world(world_id);
        assert_eq!(Ok(Some(World::new_with_nonce(owners, 0))), world);
    }
}
