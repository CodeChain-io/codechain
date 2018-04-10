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

use std::collections::VecDeque;
use std::sync::Arc;

use cbytes::Bytes;
use ckeys::Private;
use ctypes::{Address, H256, U256};
use parking_lot::{Mutex, RwLock};

use super::super::block::{ClosedBlock, IsBlock};
use super::super::client::{AccountData, BlockChain, BlockProducer, MiningBlockChainClient, SealedBlockImporter};
use super::super::consensus::{CodeChainEngine, Seal};
use super::super::error::Error;
use super::super::spec::Spec;
use super::super::state::State;
use super::super::transaction::{SignedTransaction, TransactionError, UnverifiedTransaction};
use super::super::types::{BlockId, TransactionId};
use super::transaction_queue::{
    AccountDetails, TransactionDetailsProvider as TransactionQueueDetailsProvider, TransactionOrigin, TransactionQueue,
};
use super::{MinerService, MinerStatus, TransactionImportResult};

/// Configures the behaviour of the miner.
#[derive(Debug, PartialEq)]
pub struct MinerOptions {
    /// Maximum size of the transaction queue.
    pub tx_queue_size: usize,
    /// Maximum memory usage of transactions in the queue (current / future).
    pub tx_queue_memory_limit: Option<usize>,
}

impl Default for MinerOptions {
    fn default() -> Self {
        MinerOptions {
            tx_queue_size: 8192,
            tx_queue_memory_limit: Some(2 * 1024 * 1024),
        }
    }
}

pub struct Miner {
    transaction_queue: Arc<RwLock<TransactionQueue>>,
    author: RwLock<Address>,
    extra_data: RwLock<Bytes>,
    sealing_queue: Mutex<VecDeque<ClosedBlock>>,
    engine: Arc<CodeChainEngine>,
}

impl Miner {
    pub fn new(options: MinerOptions, spec: &Spec) -> Arc<Self> {
        let mem_limit = options.tx_queue_memory_limit.unwrap_or_else(usize::max_value);
        let txq = TransactionQueue::with_limits(options.tx_queue_size, mem_limit);
        Arc::new(Self {
            transaction_queue: Arc::new(RwLock::new(txq)),
            author: RwLock::new(Address::default()),
            extra_data: RwLock::new(Vec::new()),
            sealing_queue: Mutex::new(VecDeque::new()),
            engine: spec.engine.clone(),
        })
    }

    fn add_transactions_to_queue<C: AccountData + BlockChain>(
        &self,
        client: &C,
        transactions: Vec<UnverifiedTransaction>,
        default_origin: TransactionOrigin,
        transaction_queue: &mut TransactionQueue,
    ) -> Vec<Result<TransactionImportResult, Error>> {
        let best_block_header = client.best_block_header().decode();
        let insertion_time = client.chain_info().best_block_number;
        let mut inserted = Vec::with_capacity(transactions.len());

        let results = transactions
            .into_iter()
            .map(|tx| {
                let hash = tx.hash();
                if client.transaction_block(TransactionId::Hash(hash)).is_some() {
                    debug!(target: "miner", "Rejected tx {:?}: already in the blockchain", hash);
                    return Err(Error::Transaction(TransactionError::AlreadyImported))
                }
                match self.engine
                    .verify_transaction_basic(&tx, &best_block_header)
                    .and_then(|_| self.engine.verify_transaction_unordered(tx, &best_block_header))
                {
                    Err(e) => {
                        debug!(target: "miner", "Rejected tx {:?} with invalid signature: {:?}", hash, e);
                        Err(e)
                    }
                    Ok(transaction) => {
                        // This check goes here because verify_transaction takes SignedTransaction parameter
                        self.engine.machine().verify_transaction(&transaction, &best_block_header, client)?;

                        // FIXME: Determine the origin from transaction.sender().
                        let origin = default_origin;
                        let details_provider = TransactionDetailsProvider::new(client);
                        let hash = transaction.hash();
                        let result = transaction_queue.add(transaction, origin, insertion_time, &details_provider)?;

                        inserted.push(hash);
                        Ok(result)
                    }
                }
            })
            .collect();

        results
    }

    /// Attempts to perform internal sealing (one that does not require work) and handles the result depending on the type of Seal.
    fn seal_and_import_block_internally<C>(&self, chain: &C, block: ClosedBlock) -> bool
    where
        C: BlockChain + SealedBlockImporter, {
        trace!(target: "miner", "seal_block_internally: attempting internal seal.");
        if block.transactions().is_empty() {
            return false
        }

        let parent_header = match chain.block_header(BlockId::Hash(*block.header().parent_hash())) {
            Some(hdr) => hdr.decode(),
            None => return false,
        };

        match self.engine.generate_seal(block.block(), &parent_header) {
            // Save proposal for later seal submission and broadcast it.
            Seal::Proposal(seal) => {
                trace!(target: "miner", "Received a Proposal seal.");
                {
                    let mut sealing_queue = self.sealing_queue.lock();
                    sealing_queue.push_back(block.clone());
                }
                block
                    .lock()
                    .seal(&*self.engine, seal)
                    .map(|sealed| {
                        chain.broadcast_proposal_block(sealed);
                        true
                    })
                    .unwrap_or_else(|e| {
                        warn!("ERROR: seal failed when given internally generated seal: {}", e);
                        false
                    })
            }
            // Directly import a regular sealed block.
            Seal::Regular(seal) => block
                .lock()
                .seal(&*self.engine, seal)
                .map(|sealed| chain.import_sealed_block(sealed).is_ok())
                .unwrap_or_else(|e| {
                    warn!("ERROR: seal failed when given internally generated seal: {}", e);
                    false
                }),
            Seal::None => false,
        }
    }
}

impl MinerService for Miner {
    type State = State<::state_db::StateDB>;

    fn status(&self) -> MinerStatus {
        let status = self.transaction_queue.read().status();
        MinerStatus {
            transactions_in_pending_queue: status.pending,
            transactions_in_future_queue: status.future,
            // FIXME: Fill in transactions_in_pending_block.
            transactions_in_pending_block: 0,
        }
    }

    fn author(&self) -> Address {
        *self.author.read()
    }

    fn set_author(&self, author: Address) {
        *self.author.write() = author;
    }

    fn extra_data(&self) -> Bytes {
        self.extra_data.read().clone()
    }

    fn set_extra_data(&self, extra_data: Bytes) {
        *self.extra_data.write() = extra_data;
    }

    fn set_engine_signer(&self, address: Address, private: Private) {
        if self.engine.seals_internally().is_some() {
            self.engine.set_signer(address, private)
        }
    }

    fn minimal_fee(&self) -> U256 {
        *self.transaction_queue.read().minimal_fee()
    }

    fn set_minimal_fee(&self, min_fee: U256) {
        self.transaction_queue.write().set_minimal_fee(min_fee);
    }

    fn transactions_limit(&self) -> usize {
        self.transaction_queue.read().limit()
    }

    fn set_transactions_limit(&self, limit: usize) {
        self.transaction_queue.write().set_limit(limit)
    }

    fn chain_new_blocks<C>(
        &self,
        chain: &C,
        _imported: &[H256],
        _invalid: &[H256],
        _enacted: &[H256],
        retracted: &[H256],
    ) where
        C: AccountData + BlockChain + BlockProducer + SealedBlockImporter, {
        trace!(target: "miner", "chain_new_blocks");

        // Then import all transactions...
        {
            let mut transaction_queue = self.transaction_queue.write();
            for hash in retracted {
                let block = chain.block(BlockId::Hash(*hash)).expect(
                    "Client is sending message after commit to db and inserting to chain; the block is available; qed",
                );
                let txs = block.transactions();
                let _ = self.add_transactions_to_queue(
                    chain,
                    txs,
                    TransactionOrigin::RetractedBlock,
                    &mut transaction_queue,
                );
            }
        }

        // ...and at the end remove the old ones
        {
            let fetch_account = |a: &Address| AccountDetails {
                nonce: chain.latest_nonce(a),
                balance: chain.latest_balance(a),
            };
            let time = chain.chain_info().best_block_number;
            let mut transaction_queue = self.transaction_queue.write();
            transaction_queue.remove_old(&fetch_account, time);
        }
    }

    fn update_sealing<C>(&self, _chain: &C)
    where
        C: AccountData + BlockChain + BlockProducer + SealedBlockImporter, {
        unimplemented!();
    }

    fn submit_seal<C: SealedBlockImporter>(
        &self,
        _chain: &C,
        _block_hash: H256,
        _seal: Vec<Bytes>,
    ) -> Result<(), Error> {
        unimplemented!();
    }

    fn import_external_transactions<C: MiningBlockChainClient>(
        &self,
        client: &C,
        transactions: Vec<UnverifiedTransaction>,
    ) -> Vec<Result<TransactionImportResult, Error>> {
        trace!(target: "external_tx", "Importing external transactions");
        let mut transaction_queue = self.transaction_queue.write();
        self.add_transactions_to_queue(client, transactions, TransactionOrigin::External, &mut transaction_queue)
    }

    fn import_own_transaction<C: MiningBlockChainClient>(
        &self,
        chain: &C,
        transaction: SignedTransaction,
    ) -> Result<TransactionImportResult, Error> {
        trace!(target: "own_tx", "Importing transaction: {:?}", transaction);

        // Be sure to release the lock before we call prepare_work_sealing
        let mut transaction_queue = self.transaction_queue.write();
        // We need to re-validate transactions
        let import = self.add_transactions_to_queue(
            chain,
            vec![transaction.into()],
            TransactionOrigin::Local,
            &mut transaction_queue,
        ).pop()
            .expect("one result returned per added transaction; one added => one result; qed");

        match import {
            Ok(_) => {
                trace!(target: "own_tx", "Status: {:?}", transaction_queue.status());
            }
            Err(ref e) => {
                trace!(target: "own_tx", "Status: {:?}", transaction_queue.status());
                warn!(target: "own_tx", "Error importing transaction: {:?}", e);
            }
        }
        import
    }

    fn ready_transactions(&self) -> Vec<SignedTransaction> {
        self.transaction_queue.read().top_transactions()
    }

    /// Get a list of all future transactions.
    fn future_transactions(&self) -> Vec<SignedTransaction> {
        self.transaction_queue.read().future_transactions()
    }
}

struct TransactionDetailsProvider<'a, C: 'a> {
    client: &'a C,
}

impl<'a, C> TransactionDetailsProvider<'a, C> {
    pub fn new(client: &'a C) -> Self {
        TransactionDetailsProvider {
            client,
        }
    }
}

impl<'a, C> TransactionQueueDetailsProvider for TransactionDetailsProvider<'a, C>
where
    C: AccountData,
{
    fn fetch_account(&self, address: &Address) -> AccountDetails {
        AccountDetails {
            nonce: self.client.latest_nonce(address),
            balance: self.client.latest_balance(address),
        }
    }
}
