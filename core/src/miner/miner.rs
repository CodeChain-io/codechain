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

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    AccountDetails, RemovalReason, TransactionDetailsProvider as TransactionQueueDetailsProvider, TransactionOrigin,
    TransactionQueue,
};
use super::{MinerService, MinerStatus, TransactionImportResult};

/// Configures the behaviour of the miner.
#[derive(Debug, PartialEq)]
pub struct MinerOptions {
    /// Reseal on receipt of new local transactions.
    pub reseal_on_own_tx: bool,
    /// Minimum period between transaction-inspired reseals.
    pub reseal_min_period: Duration,
    /// Maximum size of the transaction queue.
    pub tx_queue_size: usize,
    /// Maximum memory usage of transactions in the queue (current / future).
    pub tx_queue_memory_limit: Option<usize>,
}

impl Default for MinerOptions {
    fn default() -> Self {
        MinerOptions {
            reseal_on_own_tx: true,
            reseal_min_period: Duration::from_secs(2),
            tx_queue_size: 8192,
            tx_queue_memory_limit: Some(2 * 1024 * 1024),
        }
    }
}

struct SealingQueue {
    backing: Vec<ClosedBlock>,
}

impl SealingQueue {
    fn new() -> Self {
        Self {
            backing: Vec::new(),
        }
    }

    fn push(&mut self, b: ClosedBlock) {
        self.backing.push(b)
    }

    fn take_if<P>(&mut self, predicate: P) -> Option<ClosedBlock>
    where
        P: Fn(&ClosedBlock) -> bool, {
        self.backing.iter().position(|r| predicate(r)).map(|i| self.backing.remove(i))
    }
}

pub struct Miner {
    transaction_queue: Arc<RwLock<TransactionQueue>>,
    next_allowed_reseal: Mutex<Instant>,
    author: RwLock<Address>,
    extra_data: RwLock<Bytes>,
    sealing_queue: Mutex<SealingQueue>,
    engine: Arc<CodeChainEngine>,
    options: MinerOptions,
}

impl Miner {
    pub fn new(options: MinerOptions, spec: &Spec) -> Arc<Self> {
        Arc::new(Self::new_raw(options, spec))
    }

    pub fn with_spec(spec: &Spec) -> Self {
        Self::new_raw(Default::default(), spec)
    }

    fn new_raw(options: MinerOptions, spec: &Spec) -> Self {
        let mem_limit = options.tx_queue_memory_limit.unwrap_or_else(usize::max_value);
        let txq = TransactionQueue::with_limits(options.tx_queue_size, mem_limit);
        Self {
            transaction_queue: Arc::new(RwLock::new(txq)),
            next_allowed_reseal: Mutex::new(Instant::now()),
            author: RwLock::new(Address::default()),
            extra_data: RwLock::new(Vec::new()),
            sealing_queue: Mutex::new(SealingQueue::new()),
            engine: spec.engine.clone(),
            options,
        }
    }

    /// Check is reseal is allowed and necessary.
    fn requires_reseal(&self) -> bool {
        let has_local_transactions = self.transaction_queue.read().has_local_pending_transactions();
        let should_disable_sealing = !has_local_transactions && self.engine.seals_internally().is_none();

        trace!(target: "miner", "requires_reseal: should_disable_sealing={}", should_disable_sealing);

        if should_disable_sealing {
            trace!(target: "miner", "Miner sleeping");
            false
        } else {
            // sealing enabled and we don't want to sleep.
            *self.next_allowed_reseal.lock() = Instant::now() + self.options.reseal_min_period;
            true
        }
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

    /// Prepares new block for sealing including top transactions from queue.
    fn prepare_block<C: AccountData + BlockChain + BlockProducer>(&self, chain: &C) -> ClosedBlock {
        let (transactions, mut open_block) = {
            let transactions = self.transaction_queue.read().top_transactions();

            trace!(target: "miner", "prepare_block: No existing work - making new block");
            let open_block = chain.prepare_open_block(self.author(), self.extra_data());

            (transactions, open_block)
        };

        let mut invalid_transactions = HashSet::new();
        let mut non_allowed_transactions = HashSet::new();
        let block_number = open_block.block().header().number();

        let mut tx_count: usize = 0;
        let tx_total = transactions.len();
        for tx in transactions {
            let hash = tx.hash();
            let start = Instant::now();
            // Check whether transaction type is allowed for sender
            let result = match self.engine.machine().verify_transaction(&tx, open_block.header(), chain) {
                Err(Error::Transaction(TransactionError::NotAllowed)) => Err(TransactionError::NotAllowed.into()),
                _ => open_block.push_transaction(tx, None),
            };
            let took = start.elapsed();

            trace!(target: "miner", "Adding tx {:?} took {:?}", hash, took);
            match result {
                // already have transaction - ignore
                Err(Error::Transaction(TransactionError::AlreadyImported)) => {}
                Err(Error::Transaction(TransactionError::NotAllowed)) => {
                    non_allowed_transactions.insert(hash);
                    debug!(target: "miner",
                           "Skipping non-allowed transaction for sender {:?}",
                           hash);
                }
                Err(e) => {
                    invalid_transactions.insert(hash);
                    debug!(target: "miner",
                           "Error adding transaction to block: number={}. transaction_hash={:?}, Error: {:?}",
                           block_number, hash, e);
                }
                _ => {
                    tx_count += 1;
                } // imported ok
            }
        }
        trace!(target: "miner", "Pushed {}/{} transactions", tx_count, tx_total);

        let block = open_block.close();

        let fetch_nonce = |a: &Address| chain.latest_nonce(a);

        {
            let mut queue = self.transaction_queue.write();
            for hash in invalid_transactions {
                queue.remove(&hash, &fetch_nonce, RemovalReason::Invalid);
            }
            for hash in non_allowed_transactions {
                queue.remove(&hash, &fetch_nonce, RemovalReason::NotAllowed);
            }
        }
        block
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
                    sealing_queue.push(block.clone());
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

    /// Are we allowed to do a non-mandatory reseal?
    fn tx_reseal_allowed(&self) -> bool {
        Instant::now() > *self.next_allowed_reseal.lock()
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

    fn update_sealing<C>(&self, chain: &C)
    where
        C: AccountData + BlockChain + BlockProducer + SealedBlockImporter, {
        trace!(target: "miner", "update_sealing: preparing a block");
        if self.requires_reseal() {
            let block = self.prepare_block(chain);

            match self.engine.seals_internally() {
                Some(true) => {
                    trace!(target: "miner", "update_sealing: engine indicates internal sealing");
                    if self.seal_and_import_block_internally(chain, block) {
                        trace!(target: "miner", "update_sealing: imported internally sealed block");
                    }
                }
                Some(false) => {
                    trace!(target: "miner", "update_sealing: engine is not keen to seal internally right now")
                }
                None => {
                    trace!(target: "miner", "update_sealing: engine does not seal internally, preparing work");
                    unreachable!("External sealing is not supported")
                }
            }
        }
    }

    fn submit_seal<C: SealedBlockImporter>(&self, chain: &C, block_hash: H256, seal: Vec<Bytes>) -> Result<(), Error> {
        let result = if let Some(b) = self.sealing_queue.lock().take_if(|b| &b.hash() == &block_hash) {
            trace!(target: "miner", "Submitted block {}={}={} with seal {:?}", block_hash, b.hash(), b.header().bare_hash(), seal);
            b.lock().try_seal(&*self.engine, seal).or_else(|(e, _)| {
                warn!(target: "miner", "Mined solution rejected: {}", e);
                Err(Error::PowInvalid)
            })
        } else {
            warn!(target: "miner", "Submitted solution rejected: Block unknown or out of date.");
            Err(Error::PowHashInvalid)
        };
        result.and_then(|sealed| {
            let n = sealed.header().number();
            let h = sealed.header().hash();
            chain.import_sealed_block(sealed)?;
            info!(target: "miner", "Submitted block imported OK. #{}: {}", n, h);
            Ok(())
        })
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

        let imported = {
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
        };

        // --------------------------------------------------------------------------
        // | NOTE Code below requires transaction_queue and sealing_work locks.     |
        // | Make sure to release the locks before calling that method.             |
        // --------------------------------------------------------------------------
        if imported.is_ok() && self.options.reseal_on_own_tx && self.tx_reseal_allowed() {
            // Make sure to do it after transaction is imported and lock is dropped.
            // We need to create pending block and enable sealing.
            if self.engine.seals_internally().unwrap_or(false) {
                // If new block has not been prepared (means we already had one)
                // or Engine might be able to seal internally,
                // we need to update sealing.
                self.update_sealing(chain);
            }
        }
        imported
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
