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

use ccrypto::Blake;
use ctypes::{Address, Bytes, H256, U128};
use cvm::{decode, execute, ScriptResult, VMConfig};
use error::Error;
use trie::{self, Result as TrieResult, Trie, TrieError, TrieFactory};
use unexpected::Mismatch;

use super::super::parcel::{AssetTransferInput, AssetTransferOutput};
use super::super::state_db::StateDB;
use super::super::{Transaction, TransactionError};
use super::cache::{Cache, CacheableItem};
use super::traits::{CheckpointId, StateWithCache, StateWithCheckpoint};
use super::{Asset, AssetAddress, AssetScheme, AssetSchemeAddress, Backend, ShardBackend, ShardState, ShardStateInfo};

pub struct ShardLevelState<B> {
    db: B,
    root: H256,
    asset_scheme: Cache<AssetScheme>,
    asset: Cache<Asset>,
    id_of_checkpoints: Vec<CheckpointId>,
    trie_factory: TrieFactory,
}

impl<B: Backend + ShardBackend> ShardLevelState<B> {
    /// Creates new state with empty state root
    /// Used for tests.
    #[cfg(test)]
    pub fn new(mut db: B, trie_factory: TrieFactory) -> ShardLevelState<B> {
        let mut root = H256::new();

        // init trie and reset root too null
        let _ = trie_factory.create(db.as_hashdb_mut(), &mut root);

        ShardLevelState {
            db,
            root,
            asset_scheme: Cache::new(),
            asset: Cache::new(),
            id_of_checkpoints: Default::default(),
            trie_factory,
        }
    }

    /// Creates new state with existing state root
    pub fn from_existing(db: B, root: H256, trie_factory: TrieFactory) -> Result<ShardLevelState<B>, TrieError> {
        if !db.as_hashdb().contains(&root) {
            return Err(TrieError::InvalidStateRoot(root))
        }

        Ok(ShardLevelState {
            db,
            root,
            asset_scheme: Cache::new(),
            asset: Cache::new(),
            id_of_checkpoints: Default::default(),
            trie_factory,
        })
    }

    /// Destroy the current object and return root and database.
    pub fn drop(mut self) -> (H256, B) {
        self.propagate_to_global_cache();
        (self.root, self.db)
    }
}

impl<B: Backend + ShardBackend> ShardStateInfo for ShardLevelState<B> {
    fn asset_scheme(&self, a: &AssetSchemeAddress) -> trie::Result<Option<AssetScheme>> {
        let cached_asset = self.db.get_cached_asset_scheme(&a).and_then(|asset_scheme| asset_scheme);
        if cached_asset.is_some() {
            return Ok(cached_asset)
        }

        let trie = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        Ok(trie.get_with(a.as_ref(), AssetScheme::from_rlp)?)
    }

    fn asset(&self, a: &AssetAddress) -> trie::Result<Option<Asset>> {
        let cached_asset = self.db.get_cached_asset(&a).and_then(|asset| asset);
        if cached_asset.is_some() {
            return Ok(cached_asset)
        }

        let trie = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        Ok(trie.get_with(a.as_ref(), Asset::from_rlp)?)
    }
}

impl<B> StateWithCheckpoint for ShardLevelState<B> {
    fn create_checkpoint(&mut self, id: CheckpointId) {
        self.id_of_checkpoints.push(id);
        self.asset_scheme.checkpoint();
        self.asset.checkpoint();
    }

    fn discard_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.asset_scheme.discard_checkpoint();
        self.asset.discard_checkpoint();
    }

    fn revert_to_checkpoint(&mut self, id: CheckpointId) {
        let expected = self.id_of_checkpoints.pop().expect("The checkpoint must exist");
        assert_eq!(expected, id);

        self.asset_scheme.revert_to_checkpoint();
        self.asset.revert_to_checkpoint();
    }
}

impl<B: Backend + ShardBackend> StateWithCache for ShardLevelState<B> {
    fn commit(&mut self) -> TrieResult<()> {
        let mut trie = self.trie_factory.from_existing(self.db.as_hashdb_mut(), &mut self.root)?;
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

trait ShardStateInternal {
    fn mint_asset(
        &mut self,
        transaction_hash: H256,
        metadata: &String,
        lock_script_hash: &H256,
        parameters: &Vec<Bytes>,
        amount: &Option<u64>,
        registrar: &Option<Address>,
    ) -> Result<(), Error>;

    fn transfer_asset(
        &mut self,
        transaction: &Transaction,
        burns: &[AssetTransferInput],
        inputs: &[AssetTransferInput],
        outputs: &[AssetTransferOutput],
    ) -> Result<(), Error>;

    fn kill_asset(&mut self, account: &AssetAddress);


    fn require_asset_scheme<'a, F>(
        &'a self,
        a: &AssetSchemeAddress,
        default: F,
    ) -> trie::Result<RefMut<'a, AssetScheme>>
    where
        F: FnOnce() -> AssetScheme;

    fn require_asset<'a, F>(&'a self, a: &AssetAddress, default: F) -> trie::Result<RefMut<'a, Asset>>
    where
        F: FnOnce() -> Asset;
}

impl<B: Backend + ShardBackend> ShardStateInternal for ShardLevelState<B> {
    fn mint_asset(
        &mut self,
        transaction_hash: H256,
        metadata: &String,
        lock_script_hash: &H256,
        parameters: &Vec<Bytes>,
        amount: &Option<u64>,
        registrar: &Option<Address>,
    ) -> Result<(), Error> {
        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash);
        let amount = amount.unwrap_or(::std::u64::MAX);
        let asset_scheme = self.require_asset_scheme(&asset_scheme_address, || {
            AssetScheme::new(metadata.clone(), amount, registrar.clone())
        })?;
        ctrace!(TX, "{:?} is minted on {:?}", asset_scheme, asset_scheme_address);

        let asset_address = AssetAddress::new(transaction_hash, 0);
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
    ) -> Result<(), Error> {
        debug_assert!(is_input_and_output_consistent(inputs, outputs));

        for (input, burn) in inputs.iter().map(|input| (input, false)).chain(burns.iter().map(|input| (input, true))) {
            let (address_hash, asset) = {
                let index = input.prev_out.index;
                let address = AssetAddress::new(input.prev_out.transaction_hash, index);
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
            let address = AssetAddress::new(input.prev_out.transaction_hash, index);

            let asset_type = input.prev_out.asset_type.clone();
            let asset_scheme_address = AssetSchemeAddress::from_hash(asset_type)
                .ok_or(TransactionError::AssetSchemeNotFound(asset_type.into()))?;
            let _asset_scheme = self.asset_scheme((&asset_scheme_address).into())?
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
            let asset_address = AssetAddress::new(transaction.hash(), index);
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

    fn require_asset_scheme<'a, F>(
        &'a self,
        a: &AssetSchemeAddress,
        default: F,
    ) -> trie::Result<RefMut<'a, AssetScheme>>
    where
        F: FnOnce() -> AssetScheme, {
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_asset_scheme(a);
        self.asset_scheme.require_item_or_from(a, default, db, from_db)
    }

    fn require_asset<'a, F>(&'a self, a: &AssetAddress, default: F) -> trie::Result<RefMut<'a, Asset>>
    where
        F: FnOnce() -> Asset, {
        let db = self.trie_factory.readonly(self.db.as_hashdb(), &self.root)?;
        let from_db = || self.db.get_cached_asset(a);
        self.asset.require_item_or_from(a, default, db, from_db)
    }
}

impl<B: ShardBackend> fmt::Debug for ShardLevelState<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "asset_scheme: {:?} asset: {:?}", self.asset_scheme, self.asset)
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
            asset_scheme: self.asset_scheme.clone(),
            asset: self.asset.clone(),
            trie_factory: self.trie_factory.clone(),
        }
    }
}

fn is_input_and_output_consistent(inputs: &[AssetTransferInput], outputs: &[AssetTransferOutput]) -> bool {
    let mut sum: HashMap<H256, U128> = HashMap::new();

    for input in inputs {
        let ref asset_type = input.prev_out.asset_type;
        let ref amount = input.prev_out.amount;
        let current_amount = sum.get(&asset_type).cloned().unwrap_or(U128::zero());
        sum.insert(asset_type.clone(), current_amount + U128::from(*amount));
    }
    for output in outputs {
        let ref asset_type = output.asset_type;
        let ref amount = output.amount;
        let current_amount = if let Some(current_amount) = sum.get(&asset_type) {
            if current_amount < &U128::from(*amount) {
                return false
            }
            current_amount.clone()
        } else {
            return false
        };
        let t = sum.insert(asset_type.clone(), current_amount - From::from(*amount));
        debug_assert!(t.is_some());
    }

    sum.iter().all(|(_, sum)| sum.is_zero())
}

impl<B: Backend + ShardBackend> ShardState<B> for ShardLevelState<B> {
    fn execute_transaction(&mut self, transaction: &Transaction, parcel_network_id: &u64) -> Result<(), Error> {
        ctrace!(TX, "Execute {:?}(TxHash:{:?})", transaction, transaction.hash());
        match transaction {
            Transaction::AssetMint {
                metadata,
                lock_script_hash,
                amount,
                parameters,
                registrar,
                ..
            } => Ok(self.mint_asset(transaction.hash(), metadata, lock_script_hash, parameters, amount, registrar)?),
            Transaction::AssetTransfer {
                burns,
                inputs,
                outputs,
                network_id,
                ..
            } => {
                if parcel_network_id != network_id {
                    return Err(TransactionError::InvalidNetworkId(Mismatch {
                        expected: *parcel_network_id,
                        found: *network_id,
                    }).into())
                }
                self.transfer_asset(&transaction, burns, inputs, outputs)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::parcel::AssetOutPoint;
    use super::super::super::tests::helpers::get_temp_state_db;
    use super::*;

    #[test]
    fn mint_permissioned_asset() {
        let mut state = {
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.clone_canon(&root_parent);
            ShardLevelState::new(state_db, Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let amount = 100;
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters,
            amount: Some(amount),
            registrar,
            nonce: 0,
        };
        let transaction_hash = transaction.hash();

        let minted_result =
            state.mint_asset(transaction_hash, &metadata, &lock_script_hash, &vec![], &Some(amount), &registrar);
        assert!(minted_result.is_ok());

        let commit = state.commit();
        assert!(commit.is_ok());

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert!(asset_scheme.is_ok());
        let asset_scheme = asset_scheme.unwrap();
        assert!(asset_scheme.is_some());
        let asset_scheme = asset_scheme.unwrap();

        assert_eq!(&metadata, asset_scheme.metadata());
        assert_eq!(&amount, asset_scheme.amount());
        assert_eq!(&registrar, asset_scheme.registrar());
        assert!(asset_scheme.is_permissioned());
    }

    #[test]
    fn mint_infinite_asset() {
        let mut state = {
            let state_db = get_temp_state_db();
            let root_parent = H256::random();

            let state_db = state_db.clone_canon(&root_parent);
            ShardLevelState::new(state_db, Default::default())
        };

        let metadata = "metadata".to_string();
        let lock_script_hash = H256::random();
        let parameters = vec![];
        let registrar = Some(Address::random());
        let transaction = Transaction::AssetMint {
            metadata: metadata.clone(),
            lock_script_hash,
            parameters: vec![],
            amount: None,
            registrar,
            nonce: 0,
        };
        let transaction_hash = transaction.hash();

        let minted_result =
            state.mint_asset(transaction_hash, &metadata, &lock_script_hash, &parameters, &None, &registrar);
        assert!(minted_result.is_ok());

        let commit = state.commit();
        assert!(commit.is_ok());

        let asset_scheme_address = AssetSchemeAddress::new(transaction_hash);
        let asset_scheme = state.asset_scheme(&asset_scheme_address);
        assert!(asset_scheme.is_ok());
        let asset_scheme = asset_scheme.unwrap();
        assert!(asset_scheme.is_some());
        let asset_scheme = asset_scheme.unwrap();

        assert_eq!(&metadata, asset_scheme.metadata());
        assert_eq!(&::std::u64::MAX, asset_scheme.amount());
        assert_eq!(&registrar, asset_scheme.registrar());
        assert!(asset_scheme.is_permissioned());
    }

    #[test]
    fn test_is_input_and_output_consistent() {
        let asset_type = H256::random();
        let amount = 100;

        assert!(is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount,
            }]
        ));
    }

    #[test]
    fn multiple_asset_is_input_and_output_consistent() {
        let asset_type1 = H256::random();
        let asset_type2 = {
            let mut asset_type = H256::random();
            while asset_type == asset_type1 {
                asset_type = H256::random();
            }
            asset_type
        };
        let amount1 = 100;
        let amount2 = 200;

        assert!(is_input_and_output_consistent(
            &[
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        amount: amount1,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type2,
                        amount: amount2,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
            ],
            &[
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type1,
                    amount: amount1,
                },
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type2,
                    amount: amount2,
                },
            ]
        ));
    }

    #[test]
    fn multiple_asset_different_order_is_input_and_output_consistent() {
        let asset_type1 = H256::random();
        let asset_type2 = {
            let mut asset_type = H256::random();
            while asset_type == asset_type1 {
                asset_type = H256::random();
            }
            asset_type
        };
        let amount1 = 100;
        let amount2 = 200;

        assert!(is_input_and_output_consistent(
            &[
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        amount: amount1,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type2,
                        amount: amount2,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
            ],
            &[
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type2,
                    amount: amount2,
                },
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type1,
                    amount: amount1,
                },
            ]
        ));
    }

    #[test]
    fn empty_is_input_and_output_consistent() {
        assert!(is_input_and_output_consistent(&[], &[]));
    }

    #[test]
    fn fail_if_output_has_more_asset() {
        let asset_type = H256::random();
        let output_amount = 100;
        assert!(!is_input_and_output_consistent(
            &[],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount: output_amount,
            }]
        ));
    }

    #[test]
    fn fail_if_input_has_more_asset() {
        let asset_type = H256::random();
        let input_amount = 100;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[]
        ));
    }

    #[test]
    fn fail_if_input_is_larger_than_output() {
        let asset_type = H256::random();
        let input_amount = 100;
        let output_amount = 80;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount: output_amount,
            }]
        ));
    }
}
