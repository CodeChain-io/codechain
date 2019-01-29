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

use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::vec::Vec;

use ccore::{
    BlockId, DatabaseClient, EngineClient, EngineInfo, MinerService, MiningBlockChainClient, SignedTransaction,
    COL_STATE,
};
use ccrypto::Blake;
use cjson::bytes::Bytes;
use ckey::{Address, KeyPair, Private};
use cnetwork::IntoSocketAddr;
use csync::BlockSyncInfo;
use ctypes::transaction::{
    Action, AssetMintOutput, AssetOutPoint, AssetTransferInput, AssetTransferOutput, Transaction,
};
use jsonrpc_core::Result;
use kvdb::KeyValueDB;
use primitives::{H160, H256};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rlp::UntrustedRlp;
use time::PreciseTime;

use super::super::errors;
use super::super::traits::Devel;
use super::super::types::{TPSTestOption, TPSTestSetting};

pub struct DevelClient<C, M, B>
where
    C: DatabaseClient + EngineInfo + EngineClient + MiningBlockChainClient,
    M: MinerService,
    B: BlockSyncInfo, {
    client: Arc<C>,
    db: Arc<KeyValueDB>,
    miner: Arc<M>,
    block_sync: Option<Arc<B>>,
}

impl<C, M, B> DevelClient<C, M, B>
where
    C: DatabaseClient + EngineInfo + EngineClient + MiningBlockChainClient,
    M: MinerService,
    B: BlockSyncInfo,
{
    pub fn new(client: Arc<C>, miner: Arc<M>, block_sync: Option<Arc<B>>) -> Self {
        let db = client.database();
        Self {
            client,
            db,
            miner,
            block_sync,
        }
    }
}

impl<C, M, B> Devel for DevelClient<C, M, B>
where
    C: DatabaseClient + EngineInfo + EngineClient + MiningBlockChainClient + 'static,
    M: MinerService + 'static,
    B: BlockSyncInfo + 'static,
{
    fn get_state_trie_keys(&self, offset: usize, limit: usize) -> Result<Vec<H256>> {
        let iter = self.db.iter(COL_STATE);
        Ok(iter.skip(offset).take(limit).map(|val| H256::from(val.0.deref())).collect())
    }

    fn get_state_trie_value(&self, key: H256) -> Result<Vec<Bytes>> {
        match self.db.get(COL_STATE, &key).map_err(|e| errors::kvdb(&e))? {
            Some(value) => {
                let rlp = UntrustedRlp::new(&value);
                Ok(rlp.as_list::<Vec<u8>>().map_err(|e| errors::rlp(&e))?.into_iter().map(Bytes::from).collect())
            }
            None => Ok(Vec::new()),
        }
    }

    fn start_sealing(&self) -> Result<()> {
        self.miner.start_sealing(&*self.client);
        Ok(())
    }

    fn stop_sealing(&self) -> Result<()> {
        self.miner.stop_sealing();
        Ok(())
    }

    fn get_block_sync_peers(&self) -> Result<Vec<SocketAddr>> {
        if let Some(block_sync) = self.block_sync.as_ref() {
            Ok(block_sync.get_peers().into_iter().map(|node_id| node_id.into_addr().into()).collect())
        } else {
            Ok(Vec::new())
        }
    }

    fn test_tps(&self, setting: TPSTestSetting) -> Result<f64> {
        let mint_fee = self.client.common_params().min_asset_mint_cost;
        let transfer_fee = self.client.common_params().min_asset_transfer_cost;
        let pay_fee = self.client.common_params().min_pay_transaction_cost;
        let network_id = self.client.common_params().network_id;

        // NOTE: Assuming solo network
        let genesis_secret: Private = "ede1d4ccb4ec9a8bbbae9a13db3f4a7b56ea04189be86ac3a6a439d9a0a1addd".into();
        let genesis_keypair = KeyPair::from_private(genesis_secret).map_err(errors::transaction_core)?;

        let base_seq = self.client.seq(&genesis_keypair.address(), BlockId::Latest).unwrap();
        let lock_script_hash_empty_sig = H160::from("b042ad154a3359d276835c903587ebafefea22af");

        // Helper macros
        macro_rules! pay_tx {
            ($seq:expr, $address:expr) => {
                pay_tx!($seq, $address, 1)
            };
            ($seq:expr, $address:expr, $quantity: expr) => {
                Transaction {
                    seq: $seq,
                    fee: pay_fee,
                    network_id,
                    action: Action::Pay {
                        receiver: $address,
                        quantity: $quantity,
                    },
                }
            };
        }

        macro_rules! mint_tx {
            ($seq:expr, $supply:expr) => {
                Transaction {
                    seq: $seq,
                    fee: mint_fee,
                    network_id,
                    action: Action::MintAsset {
                        network_id,
                        shard_id: 0,
                        metadata: format!("{:?}", Instant::now()),
                        approver: None,
                        administrator: None,
                        allowed_script_hashes: vec![],
                        output: Box::new(AssetMintOutput {
                            lock_script_hash: lock_script_hash_empty_sig,
                            parameters: vec![],
                            supply: Some($supply),
                        }),
                        approvals: vec![],
                    },
                }
            };
        }

        macro_rules! transfer_tx {
            ($seq:expr, $inputs:expr, $outputs:expr) => {
                Transaction {
                    seq: $seq,
                    fee: transfer_fee,
                    network_id,
                    action: Action::TransferAsset {
                        network_id,
                        burns: vec![],
                        inputs: $inputs,
                        outputs: $outputs,
                        orders: vec![],
                        metadata: "".to_string(),
                        approvals: vec![],
                        expiration: None,
                    },
                }
            };
        }

        macro_rules! transfer_input {
            ($hash:expr, $index:expr, $asset_type:expr, $quantity:expr) => {
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        tracker: $hash,
                        index: $index,
                        asset_type: $asset_type,
                        shard_id: 0,
                        quantity: $quantity,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                }
            };
        }

        macro_rules! transfer_output {
            ($asset_type:expr, $quantity:expr) => {
                AssetTransferOutput {
                    lock_script_hash: lock_script_hash_empty_sig,
                    parameters: vec![],
                    asset_type: $asset_type,
                    shard_id: 0,
                    quantity: $quantity,
                }
            };
        }

        // Helper functions
        fn sign_tx(tx: Transaction, key_pair: &KeyPair) -> SignedTransaction {
            SignedTransaction::new_with_sign(tx, key_pair.private())
        }

        fn send_tx<C, M>(tx: Transaction, client: &C, key_pair: &KeyPair, miner: &M) -> Result<H256>
        where
            C: MiningBlockChainClient,
            M: MinerService, {
            let signed = SignedTransaction::new_with_sign(tx, key_pair.private());
            let hash = signed.hash();
            miner.import_own_transaction(client, signed).map_err(errors::transaction_core)?;
            Ok(hash)
        }

        fn asset_type(tx: &Transaction) -> H160 {
            Blake::blake(tx.tracker().unwrap())
        }

        fn tps(count: u64, start_time: PreciseTime, end_time: PreciseTime) -> f64 {
            f64::from(count as u32) * 1000.0_f64 / f64::from(start_time.to(end_time).num_milliseconds() as i32)
        }

        // Main
        let count = setting.count;
        let mut rng = SmallRng::seed_from_u64(setting.seed);
        let transactions = match setting.option {
            TPSTestOption::PayOnly => {
                let mut transactions = Vec::with_capacity(count as usize);
                for i in 0..count {
                    let address = Address::random();
                    let tx = sign_tx(pay_tx!(base_seq + i, address), &genesis_keypair);
                    transactions.push(tx);
                }
                transactions
            }
            TPSTestOption::TransferSingle => {
                let mint_tx = mint_tx!(base_seq, 1);
                let asset_type = asset_type(&mint_tx);
                let mut previous_tracker = mint_tx.tracker().unwrap();
                send_tx(mint_tx, &*self.client, &genesis_keypair, &*self.miner)?;

                let mut transactions = Vec::with_capacity(count as usize);
                for i in 0..count {
                    let transfer_tx = transfer_tx!(
                        base_seq + i + 1,
                        vec![transfer_input!(previous_tracker, 0, asset_type, 1)],
                        vec![transfer_output!(asset_type, 1)]
                    );
                    previous_tracker = transfer_tx.tracker().unwrap();
                    let tx = sign_tx(transfer_tx, &genesis_keypair);
                    transactions.push(tx);
                }
                transactions
            }
            TPSTestOption::TransferMultiple => {
                let number_of_in_out: usize = 10;
                let mint_tx = mint_tx!(base_seq, number_of_in_out as u64);
                let asset_type = asset_type(&mint_tx);
                let mut previous_tracker = mint_tx.tracker().unwrap();
                send_tx(mint_tx, &*self.client, &genesis_keypair, &*self.miner)?;

                fn create_inputs(
                    transaction_hash: H256,
                    asset_type: H160,
                    total_amount: u64,
                    count: usize,
                ) -> Vec<AssetTransferInput> {
                    let mut inputs = Vec::new();
                    let amount = total_amount / (count as u64);
                    for i in 0..(count as usize) {
                        let input = transfer_input!(transaction_hash, i, asset_type, amount);
                        inputs.push(input);
                    }
                    inputs
                }

                let mut transactions = Vec::with_capacity(count as usize);
                for i in 0..count {
                    let num_input = 1 + 9 * (i > 0) as usize;
                    let inputs = create_inputs(previous_tracker, asset_type, number_of_in_out as u64, num_input);
                    let outputs = vec![transfer_output!(asset_type, 1); number_of_in_out];

                    let transfer_tx = transfer_tx!(base_seq + i + 1, inputs, outputs);
                    previous_tracker = transfer_tx.tracker().unwrap();
                    transactions.push(sign_tx(transfer_tx, &genesis_keypair));
                }
                transactions
            }
            TPSTestOption::PayOrTransfer => {
                let mint_tx = mint_tx!(base_seq, 1);
                let asset_type = asset_type(&mint_tx);
                let mut previous_tracker = mint_tx.tracker().unwrap();
                send_tx(mint_tx, &*self.client, &genesis_keypair, &*self.miner)?;

                let mut transactions = Vec::with_capacity(count as usize);
                for i in 0..count {
                    // 0. Payment
                    let tx = if rng.gen::<bool>() {
                        let address = Address::random();
                        pay_tx!(base_seq + i + 1, address)
                    }
                    // 1. Transfer
                    else {
                        let transfer_tx = transfer_tx!(
                            base_seq + i + 1,
                            vec![transfer_input!(previous_tracker, 0, asset_type, 1)],
                            vec![transfer_output!(asset_type, 1)]
                        );
                        previous_tracker = transfer_tx.tracker().unwrap();
                        transfer_tx
                    };
                    let tx = sign_tx(tx, &genesis_keypair);
                    transactions.push(tx);
                }
                transactions
            }
        };

        let last_hash = transactions.last().unwrap().hash();
        let mut start_time = None;

        for tx in transactions.into_iter().rev() {
            if tx.seq == base_seq {
                start_time = Some(PreciseTime::now());
            }
            self.miner.import_own_transaction(&*self.client, tx).map_err(errors::transaction_core)?;
        }
        while !self.client.is_pending_queue_empty() {
            thread::sleep(Duration::from_millis(50));
        }
        while self.client.parcel_invoice(&last_hash.into()).is_none() {}

        let end_time = PreciseTime::now();
        Ok(tps(count, start_time.unwrap(), end_time))
    }
}
