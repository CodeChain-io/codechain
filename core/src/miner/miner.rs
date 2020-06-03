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

use super::mem_pool::{Error as MemPoolError, MemPool};
pub use super::mem_pool_types::MemPoolFees;
use super::mem_pool_types::{MemPoolInput, TxOrigin, TxTimelock};
use super::sealing_queue::SealingQueue;
use super::work_notify::{NotifyWork, WorkPoster};
use super::{fetch_account_creator, MinerService, MinerStatus, TransactionImportResult};
use crate::account_provider::{AccountProvider, Error as AccountProviderError};
use crate::block::{Block, ClosedBlock, IsBlock};
use crate::client::{
    AccountData, BlockChainTrait, BlockProducer, Client, EngineInfo, ImportBlock, MiningBlockChainClient, TermInfo,
};
use crate::codechain_machine::CodeChainMachine;
use crate::consensus::{CodeChainEngine, EngineType};
use crate::error::Error;
use crate::scheme::Scheme;
use crate::transaction::{PendingSignedTransactions, SignedTransaction, UnverifiedTransaction};
use crate::types::{BlockId, TransactionId};
use ckey::{public_to_address, Address, Password, PlatformAddress, Public};
use cstate::{FindActionHandler, TopLevelState};
use ctypes::errors::{HistoryError, RuntimeError};
use ctypes::transaction::{Action, IncompleteTransaction, Timelock};
use ctypes::{BlockHash, BlockNumber, Header, TxHash};
use cvm::ChainTimeInfo;
use kvdb::KeyValueDB;
use parking_lot::{Mutex, RwLock};
use primitives::{Bytes, H256, U256};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::iter::once;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configures the behaviour of the miner.
#[derive(Debug, PartialEq)]
pub struct MinerOptions {
    /// URLs to notify when there is new work.
    pub new_work_notify: Vec<String>,
    /// Force the miner to reseal, even when nobody has asked for work.
    pub force_sealing: bool,
    /// Reseal on receipt of new external transactions.
    pub reseal_on_external_transaction: bool,
    /// Reseal on receipt of new local transactions.
    pub reseal_on_own_transaction: bool,
    /// Minimum period between transaction-inspired reseals.
    pub reseal_min_period: Duration,
    /// Maximum period between blocks (enables force sealing after that).
    pub reseal_max_period: Duration,
    /// Disable the reseal timer
    pub no_reseal_timer: bool,
    /// Maximum size of the mem pool.
    pub mem_pool_size: usize,
    /// Maximum memory usage of transactions in the queue (current / future).
    pub mem_pool_memory_limit: Option<usize>,
    /// A value which is used to check whether a new transaciton can replace a transaction in the memory pool with the same signer and seq.
    /// If the fee of the new transaction is `new_fee` and the fee of the transaction in the memory pool is `old_fee`,
    /// then `new_fee > old_fee + old_fee >> mem_pool_fee_bump_shift` should be satisfied to replace.
    /// Local transactions ignore this option.
    pub mem_pool_fee_bump_shift: usize,
    pub allow_create_shard: bool,
    /// How many historical work packages can we store before running out?
    pub work_queue_size: usize,
    /// Minimum fees configured by the machine.
    pub mem_pool_fees: MemPoolFees,
}

impl Default for MinerOptions {
    fn default() -> Self {
        MinerOptions {
            new_work_notify: vec![],
            force_sealing: false,
            reseal_on_external_transaction: true,
            reseal_on_own_transaction: true,
            reseal_min_period: Duration::from_secs(2),
            reseal_max_period: Duration::from_secs(120),
            no_reseal_timer: false,
            mem_pool_size: 8192,
            mem_pool_memory_limit: Some(2 * 1024 * 1024),
            mem_pool_fee_bump_shift: 3,
            allow_create_shard: false,
            work_queue_size: 20,
            mem_pool_fees: Default::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct AuthoringParams {
    pub author: Address,
    pub extra_data: Bytes,
}

struct SealingWork {
    queue: SealingQueue,
    enabled: bool,
}

pub struct Miner {
    mem_pool: Arc<RwLock<MemPool>>,
    next_allowed_reseal: NextAllowedReseal,
    next_mandatory_reseal: NextMandatoryReseal,
    sealing_block_last_request: SealingBlockLastRequest,
    sealing_work: Mutex<SealingWork>,
    params: Params,
    engine: Arc<dyn CodeChainEngine>,
    options: MinerOptions,

    sealing_enabled: AtomicBool,

    accounts: Option<Arc<AccountProvider>>,
    notifiers: Notifiers,
    malicious_users: Users,
    immune_users: Users,
}

struct Users {
    users: RwLock<HashSet<Address>>,
}

impl Users {
    pub fn new() -> Self {
        Self {
            users: RwLock::new(HashSet::new()),
        }
    }

    pub fn cloned(&self) -> Vec<Address> {
        self.users.read().iter().map(Clone::clone).collect()
    }

    pub fn contains(&self, address: &Address) -> bool {
        self.users.read().contains(address)
    }

    pub fn insert(&self, address: Address) -> bool {
        self.users.write().insert(address)
    }

    pub fn remove_users<'a>(&self, addresses: impl Iterator<Item = &'a Address>) {
        let mut users = self.users.write();
        for address in addresses {
            users.remove(address);
        }
    }
}

struct Notifiers {
    notifiers: RwLock<Vec<Box<dyn NotifyWork>>>,
}

impl Notifiers {
    pub fn new(notifiers: Vec<Box<dyn NotifyWork>>) -> Self {
        Self {
            notifiers: RwLock::new(notifiers),
        }
    }

    pub fn push(&self, notifier: Box<dyn NotifyWork>) {
        self.notifiers.write().push(notifier);
    }

    pub fn is_empty(&self) -> bool {
        self.notifiers.read().is_empty()
    }

    pub fn notify(&self, pow_hash: H256, target: U256) {
        // FIXME: Calling callbacks inside of lock lifetime may cause a deadlock.
        for notifier in self.notifiers.read().iter() {
            notifier.notify(pow_hash, target)
        }
    }
}

struct SealingBlockLastRequest {
    block_number: Mutex<u64>,
}

impl SealingBlockLastRequest {
    pub fn new() -> Self {
        Self {
            block_number: Mutex::new(0),
        }
    }

    pub fn get(&self) -> u64 {
        *self.block_number.lock()
    }

    /// Returns previous value
    pub fn set(&self, block_number: u64) -> u64 {
        let mut guard = self.block_number.lock();
        let prev = *guard;
        *guard = block_number;
        prev
    }
}

type NextAllowedReseal = NextMandatoryReseal;

struct NextMandatoryReseal {
    instant: RwLock<Instant>,
}

impl NextMandatoryReseal {
    pub fn new(instant: Instant) -> Self {
        Self {
            instant: RwLock::new(instant),
        }
    }

    pub fn get(&self) -> Instant {
        *self.instant.read()
    }

    pub fn set(&self, instant: Instant) {
        *self.instant.write() = instant;
    }
}

struct Params {
    params: RwLock<AuthoringParams>,
}

impl Params {
    pub fn new(params: AuthoringParams) -> Self {
        Self {
            params: RwLock::new(params),
        }
    }

    pub fn get(&self) -> AuthoringParams {
        self.params.read().clone()
    }

    pub fn apply<F>(&self, f: F)
    where
        F: FnOnce(&mut AuthoringParams) -> (), {
        let mut params = self.params.write();
        f(&mut params);
    }
}

impl Miner {
    /// Push listener that will handle new jobs
    pub fn add_work_listener(&self, notifier: Box<dyn NotifyWork>) {
        self.notifiers.push(notifier);
    }

    pub fn new(
        options: MinerOptions,
        scheme: &Scheme,
        accounts: Option<Arc<AccountProvider>>,
        db: Arc<dyn KeyValueDB>,
    ) -> Arc<Self> {
        Arc::new(Self::new_raw(options, scheme, accounts, db))
    }

    pub fn with_scheme(scheme: &Scheme, db: Arc<dyn KeyValueDB>) -> Self {
        Self::new_raw(Default::default(), scheme, None, db)
    }

    fn new_raw(
        options: MinerOptions,
        scheme: &Scheme,
        accounts: Option<Arc<AccountProvider>>,
        db: Arc<dyn KeyValueDB>,
    ) -> Self {
        let mem_limit = options.mem_pool_memory_limit.unwrap_or_else(usize::max_value);
        let mem_pool = Arc::new(RwLock::new(MemPool::with_limits(
            options.mem_pool_size,
            mem_limit,
            options.mem_pool_fee_bump_shift,
            db,
            options.mem_pool_fees,
        )));

        let notifiers: Vec<Box<dyn NotifyWork>> = if options.new_work_notify.is_empty() {
            Vec::new()
        } else {
            vec![Box::new(WorkPoster::new(&options.new_work_notify))]
        };

        Self {
            mem_pool,
            next_allowed_reseal: NextAllowedReseal::new(Instant::now()),
            next_mandatory_reseal: NextMandatoryReseal::new(Instant::now() + options.reseal_max_period),
            params: Params::new(AuthoringParams::default()),
            sealing_block_last_request: SealingBlockLastRequest::new(),
            sealing_work: Mutex::new(SealingWork {
                queue: SealingQueue::new(options.work_queue_size),
                enabled: options.force_sealing || scheme.engine.seals_internally().is_some(),
            }),
            engine: scheme.engine.clone(),
            options,
            sealing_enabled: AtomicBool::new(true),
            accounts,
            notifiers: Notifiers::new(notifiers),
            malicious_users: Users::new(),
            immune_users: Users::new(),
        }
    }

    pub fn recover_from_db(&self, client: &Client) {
        self.mem_pool.write().recover_from_db(client);
    }

    /// Get `Some` `clone()` of the current pending block's state or `None` if we're not sealing.
    pub fn pending_state(&self, latest_block_number: BlockNumber) -> Option<TopLevelState> {
        self.map_pending_block(|b| b.state().clone(), latest_block_number)
    }

    /// Get `Some` `clone()` of the current pending block or `None` if we're not sealing.
    pub fn pending_block(&self, latest_block_number: BlockNumber) -> Option<Block> {
        self.map_pending_block(IsBlock::to_base, latest_block_number)
    }

    /// Get `Some` `clone()` of the current pending block header or `None` if we're not sealing.
    pub fn pending_block_header(&self, latest_block_number: BlockNumber) -> Option<Header> {
        self.map_pending_block(|b| b.header().clone(), latest_block_number)
    }

    pub fn get_options(&self) -> &MinerOptions {
        &self.options
    }

    /// Check is reseal is allowed and necessary.
    fn requires_reseal(&self, best_block: BlockNumber) -> bool {
        let has_local_transactions = self.mem_pool.read().has_local_pending_transactions();
        let mut sealing_work = self.sealing_work.lock();
        if sealing_work.enabled {
            ctrace!(MINER, "requires_reseal: sealing enabled");
            let last_request = self.sealing_block_last_request.get();
            let should_disable_sealing = !self.options.force_sealing
                && !has_local_transactions
                && self.engine.seals_internally().is_none()
                && best_block > last_request
                && best_block - last_request > SEALING_TIMEOUT_IN_BLOCKS;

            ctrace!(
                MINER,
                "requires_reseal: should_disable_sealing={}; best_block={}, last_request={}",
                should_disable_sealing,
                best_block,
                last_request
            );

            if should_disable_sealing {
                cdebug!(MINER, "Miner sleeping");
                sealing_work.enabled = false;
                sealing_work.queue.reset();
                false
            } else {
                true
            }
        } else {
            cdebug!(MINER, "requires_reseal: sealing is disabled");
            false
        }
    }

    fn add_transactions_to_pool<C: AccountData + BlockChainTrait + EngineInfo>(
        &self,
        client: &C,
        transactions: Vec<UnverifiedTransaction>,
        default_origin: TxOrigin,
        mem_pool: &mut MemPool,
    ) -> Vec<Result<TransactionImportResult, Error>> {
        let best_header = client.best_block_header().decode();
        let fake_header = best_header.generate_child();
        let current_block_number = client.chain_info().best_block_number;
        let current_timestamp = client.chain_info().best_block_timestamp;
        let mut inserted = Vec::with_capacity(transactions.len());
        let mut to_insert = Vec::new();
        let mut tx_hashes = Vec::new();

        let intermediate_results: Vec<Result<(), Error>> = transactions
            .into_iter()
            .map(|tx| {
                let hash = tx.hash();
                // FIXME: Refactoring is needed. recover_public is calling in verify_transaction_unordered.
                let signer_public = tx.recover_public()?;
                let signer_address = public_to_address(&signer_public);
                if default_origin.is_local() {
                    self.immune_users.insert(signer_address);
                }

                let origin = self
                    .accounts
                    .as_ref()
                    .and_then(|accounts| match accounts.has_public(&signer_public) {
                        Ok(true) => Some(TxOrigin::Local),
                        Ok(false) => None,
                        Err(_) => None,
                    })
                    .unwrap_or(default_origin);

                if self.malicious_users.contains(&signer_address) {
                    // FIXME: just to skip, think about another way.
                    return Ok(())
                }
                if client.transaction_block(&TransactionId::Hash(hash)).is_some() {
                    cdebug!(MINER, "Rejected transaction {:?}: already in the blockchain", hash);
                    return Err(HistoryError::TransactionAlreadyImported.into())
                }
                if !self.is_allowed_transaction(&tx.action) {
                    cdebug!(MINER, "Rejected transaction {:?}: {:?} is not allowed transaction", hash, tx.action);
                }
                let tx = tx
                    .verify_basic()
                    .map_err(From::from)
                    .and_then(|_| {
                        let common_params = client.common_params(best_header.hash().into()).unwrap();
                        self.engine.verify_transaction_with_params(&tx, &common_params)
                    })
                    .and_then(|_| CodeChainMachine::verify_transaction_seal(tx, &fake_header))
                    .map_err(|e| {
                        match e {
                            Error::Syntax(_) if !origin.is_local() && !self.immune_users.contains(&signer_address) => {
                                self.malicious_users.insert(signer_address);
                            }
                            _ => {}
                        }
                        cdebug!(MINER, "Rejected transaction {:?} with invalid signature: {:?}", hash, e);
                        e
                    })?;

                // This check goes here because verify_transaction takes SignedTransaction parameter
                self.engine.machine().verify_transaction(&tx, &fake_header, client, false).map_err(|e| {
                    match e {
                        Error::Syntax(_) if !origin.is_local() && !self.immune_users.contains(&signer_address) => {
                            self.malicious_users.insert(signer_address);
                        }
                        _ => {}
                    }
                    e
                })?;

                let timelock = self.calculate_timelock(&tx, client)?;
                let tx_hash = tx.hash();

                to_insert.push(MemPoolInput::new(tx, origin, timelock));
                tx_hashes.push(tx_hash);
                Ok(())
            })
            .collect();

        let fetch_account = fetch_account_creator(client);

        let insertion_results = mem_pool.add(to_insert, current_block_number, current_timestamp, &fetch_account);

        debug_assert_eq!(insertion_results.len(), intermediate_results.iter().filter(|r| r.is_ok()).count());
        let mut insertion_results_index = 0;
        intermediate_results
            .into_iter()
            .map(|res| match res {
                Err(e) => Err(e),
                Ok(()) => {
                    let idx = insertion_results_index;
                    let result = insertion_results[idx].clone().map_err(MemPoolError::into_core_error)?;
                    inserted.push(tx_hashes[idx]);
                    insertion_results_index += 1;
                    Ok(result)
                }
            })
            .collect()
    }

    pub fn delete_all_pending_transactions(&self) {
        let mut mem_pool = self.mem_pool.write();
        mem_pool.remove_all();
    }

    fn calculate_timelock<C: BlockChainTrait>(&self, tx: &SignedTransaction, client: &C) -> Result<TxTimelock, Error> {
        let mut max_block = None;
        let mut max_timestamp = None;
        if let Action::TransferAsset {
            inputs,
            ..
        } = &tx.action
        {
            for input in inputs {
                if let Some(timelock) = input.timelock {
                    let (is_block_number, value) = match timelock {
                        Timelock::Block(value) => (true, value),
                        Timelock::BlockAge(value) => (
                            true,
                            client.transaction_block_number(&input.prev_out.tracker).ok_or_else(|| {
                                Error::History(HistoryError::Timelocked {
                                    timelock,
                                    remaining_time: u64::max_value(),
                                })
                            })? + value,
                        ),
                        Timelock::Time(value) => (false, value),
                        Timelock::TimeAge(value) => (
                            false,
                            client.transaction_block_timestamp(&input.prev_out.tracker).ok_or_else(|| {
                                Error::History(HistoryError::Timelocked {
                                    timelock,
                                    remaining_time: u64::max_value(),
                                })
                            })? + value,
                        ),
                    };
                    if is_block_number {
                        if max_block.is_none() || max_block.expect("The previous guard ensures") < value {
                            max_block = Some(value);
                        }
                    } else if max_timestamp.is_none() || max_timestamp.expect("The previous guard ensures") < value {
                        max_timestamp = Some(value);
                    }
                }
            }
        };
        Ok(TxTimelock {
            block: max_block,
            timestamp: max_timestamp,
        })
    }

    /// Prepares work which has to be done to seal.
    fn prepare_work(&self, block: ClosedBlock, original_work_hash: Option<H256>) {
        let (work, is_new) = {
            let mut sealing_work = self.sealing_work.lock();
            let last_work_hash = sealing_work.queue.peek_last_ref().map(|pb| pb.block().header().hash());
            ctrace!(
                MINER,
                "prepare_work: Checking whether we need to reseal: orig={:?} last={:?}, this={:?}",
                original_work_hash,
                last_work_hash,
                block.block().header().hash()
            );
            let (work, is_new) = if last_work_hash.map_or(true, |h| h != block.block().header().hash()) {
                ctrace!(
                    MINER,
                    "prepare_work: Pushing a new, refreshed or borrowed pending {}...",
                    block.block().header().hash()
                );
                let pow_hash = *block.block().header().hash();
                let number = block.block().header().number();
                let score = *block.block().header().score();
                let is_new = original_work_hash.map_or(true, |h| *block.block().header().hash() != h);
                sealing_work.queue.push(block);
                // If push notifications are enabled we assume all work items are used.
                if !self.notifiers.is_empty() && is_new {
                    sealing_work.queue.use_last_ref();
                }
                (Some((pow_hash, score, number)), is_new)
            } else {
                (None, false)
            };
            ctrace!(
                MINER,
                "prepare_work: leaving (last={:?})",
                sealing_work.queue.peek_last_ref().map(|b| b.block().header().hash())
            );
            (work, is_new)
        };
        if is_new {
            if let Some((pow_hash, score, _number)) = work {
                let target = self.engine.score_to_target(&score);
                self.notifiers.notify(pow_hash, target);
            }
        }
    }

    /// Prepares new block for sealing including top transactions from queue.
    fn prepare_block<
        C: AccountData + BlockChainTrait + BlockProducer + ChainTimeInfo + EngineInfo + FindActionHandler + TermInfo,
    >(
        &self,
        parent_block_id: BlockId,
        chain: &C,
    ) -> Result<Option<(ClosedBlock, Option<H256>)>, Error> {
        let (transactions, mut open_block, original_work_hash, block_number) = {
            let sealing_work = self.sealing_work.lock();

            let last_work_hash = sealing_work.queue.peek_last_ref().map(|pb| *pb.block().header().hash());
            ctrace!(MINER, "prepare_block: No existing work - making new block");
            let params = self.params.get();
            let open_block = chain.prepare_open_block(parent_block_id, params.author, params.extra_data);
            let (block_number, parent_hash) = {
                let header = open_block.block().header();
                let block_number = header.number();
                let parent_hash = *header.parent_hash();
                (block_number, parent_hash)
            };
            let max_body_size = chain.common_params(parent_hash.into()).unwrap().max_body_size();
            const DEFAULT_RANGE: Range<u64> = 0..::std::u64::MAX;

            // NOTE: This lock should be acquired after `prepare_open_block` to prevent deadlock
            let mem_pool = self.mem_pool.read();
            let transactions = mem_pool
                .top_transactions(max_body_size, Some(open_block.header().timestamp()), DEFAULT_RANGE)
                .transactions;

            (transactions, open_block, last_work_hash, block_number)
        };

        let parent_header = {
            let parent_hash = open_block.header().parent_hash();
            chain.block_header(&BlockId::Hash(*parent_hash)).expect("Parent header MUST exist")
        };
        if self.engine_type().is_seal_first() {
            match self.engine.seals_internally() {
                Some(false) => panic!("If a signer is not prepared, prepare_block should not be called"),
                None => panic!("Exteranl sealing is not seals_first"),
                Some(true) => {}
            };
            let seal = self.engine.generate_seal(None, &parent_header.decode());
            if let Some(seal_bytes) = seal.seal_fields() {
                open_block.seal(self.engine.borrow(), seal_bytes).expect("Sealing always success");
            } else {
                return Ok(None)
            }
        }

        let mut invalid_transactions = Vec::new();

        let mut tx_count: usize = 0;
        let tx_total = transactions.len();
        let mut invalid_tx_users = HashSet::new();

        for tx in transactions {
            let signer_public = tx.signer_public();
            let signer_address = public_to_address(&signer_public);
            if self.malicious_users.contains(&signer_address) {
                invalid_transactions.push(tx.hash());
                continue
            }
            if invalid_tx_users.contains(&signer_public) {
                // The previous transaction has failed
                continue
            }
            if !self.is_allowed_transaction(&tx.action) {
                invalid_tx_users.insert(signer_public);
                invalid_transactions.push(tx.hash());
                continue
            }

            let hash = tx.hash();
            let start = Instant::now();
            // Check whether transaction type is allowed for sender
            let result =
                self.engine.machine().verify_transaction(&tx, open_block.header(), chain, true).and_then(|_| {
                    open_block.push_transaction(tx, None, chain, parent_header.number(), parent_header.timestamp())
                });

            match result {
                // already have transaction - ignore
                Err(Error::History(HistoryError::TransactionAlreadyImported)) => {}
                Err(e) => {
                    match e {
                        Error::Runtime(RuntimeError::AssetSupplyOverflow)
                        | Error::Runtime(RuntimeError::InvalidScript) => {
                            if !self
                                .mem_pool
                                .read()
                                .is_local_transaction(hash)
                                .expect("The tx is clearly fetched from the mempool")
                                && !self.immune_users.contains(&signer_address)
                            {
                                self.malicious_users.insert(signer_address);
                            }
                        }
                        _ => {}
                    }
                    invalid_tx_users.insert(signer_public);
                    invalid_transactions.push(hash);
                    cinfo!(
                        MINER,
                        "Error adding transaction to block: number={}. tx_hash={:?}, Error: {:?}",
                        block_number,
                        hash,
                        e
                    );
                }
                Ok(()) => {
                    let took = start.elapsed();
                    ctrace!(MINER, "Adding transaction {:?} took {:?}", hash, took);
                    tx_count += 1;
                } // imported ok
            }
        }
        cdebug!(MINER, "Pushed {}/{} transactions", tx_count, tx_total);

        let (parent_header, parent_hash) = {
            let parent_hash = *open_block.header().parent_hash();
            let parent_header = chain.block_header(&parent_hash.into()).expect("Parent header MUST exist");
            (parent_header.decode(), parent_hash)
        };
        let term_common_params = chain.term_common_params(parent_hash.into());
        let block = open_block.close(&parent_header, term_common_params.as_ref())?;

        let fetch_seq = |p: &Public| {
            let address = public_to_address(p);
            let a = chain.latest_regular_key_owner(&address).unwrap_or(address);
            chain.latest_seq(&a)
        };

        {
            let mut mem_pool = self.mem_pool.write();
            mem_pool.remove(
                &invalid_transactions,
                &fetch_seq,
                chain.chain_info().best_block_number,
                chain.chain_info().best_block_timestamp,
            );
        }
        Ok(Some((block, original_work_hash)))
    }

    /// Attempts to perform internal sealing (one that does not require work) and handles the result depending on the type of Seal.
    fn seal_and_import_block_internally<C>(&self, chain: &C, block: ClosedBlock) -> bool
    where
        C: BlockChainTrait + ImportBlock, {
        if block.transactions().is_empty()
            && !self.options.force_sealing
            && Instant::now() <= self.next_mandatory_reseal.get()
        {
            cdebug!(MINER, "seal_block_internally: no sealing.");
            return false
        }
        ctrace!(MINER, "seal_block_internally: attempting internal seal.");

        let parent_header = match chain.block_header(&(*block.header().parent_hash()).into()) {
            Some(hdr) => hdr.decode(),
            None => return false,
        };

        if self.engine.seals_internally() != Some(true) {
            ctrace!(MINER, "No seal is generated.");
            return false
        }

        self.next_mandatory_reseal.set(Instant::now() + self.options.reseal_max_period);
        let sealed = if self.engine_type().is_seal_first() {
            block.lock().already_sealed()
        } else {
            let seal = self.engine.generate_seal(Some(block.block()), &parent_header).seal_fields();
            if seal.is_none() {
                ctrace!(MINER, "No seal is generated.");
                return false
            }
            match block.lock().seal(&*self.engine, seal.unwrap()) {
                Ok(sealed) => sealed,
                Err(e) => {
                    cwarn!(MINER, "ERROR: seal failed when given internally generated seal: {}", e);
                    return false
                }
            }
        };

        if self.engine.is_proposal(sealed.header()) {
            self.engine.proposal_generated(&sealed);
        }

        chain.import_sealed_block(&sealed).is_ok()
    }

    /// Are we allowed to do a non-mandatory reseal?
    fn transaction_reseal_allowed(&self) -> bool {
        self.sealing_enabled.load(Ordering::Relaxed) && (Instant::now() > self.next_allowed_reseal.get())
    }

    fn map_pending_block<F, T>(&self, f: F, latest_block_number: BlockNumber) -> Option<T>
    where
        F: FnOnce(&ClosedBlock) -> T, {
        let sealing_work = self.sealing_work.lock();
        sealing_work.queue.peek_last_ref().and_then(|b| {
            if b.block().header().number() > latest_block_number {
                Some(f(b))
            } else {
                None
            }
        })
    }

    fn is_allowed_transaction(&self, action: &Action) -> bool {
        if let Action::CreateShard {
            ..
        } = action
        {
            if !self.options.allow_create_shard {
                return false
            }
        }
        true
    }
}

const SEALING_TIMEOUT_IN_BLOCKS: u64 = 5;

impl MinerService for Miner {
    type State = TopLevelState;

    fn status(&self) -> MinerStatus {
        let status = self.mem_pool.read().status();
        let sealing_work = self.sealing_work.lock();
        MinerStatus {
            transactions_in_pending_queue: status.pending,
            transactions_in_future_queue: status.future,
            tranasction_in_pending_block: sealing_work.queue.peek_last_ref().map_or(0, |b| b.transactions().len()),
        }
    }

    fn authoring_params(&self) -> AuthoringParams {
        self.params.get()
    }

    fn set_author(&self, address: Address) -> Result<(), AccountProviderError> {
        self.params.apply(|params| params.author = address);

        if self.engine_type().need_signer_key() && self.engine.seals_internally().is_some() {
            if let Some(ref ap) = self.accounts {
                ctrace!(MINER, "Set author to {:?}", address);
                // Sign test message
                ap.get_unlocked_account(&address)?.sign(&Default::default())?;
                // Limit the scope of the locks.
                {
                    let mut sealing_work = self.sealing_work.lock();
                    sealing_work.enabled = true;
                }
                self.engine.set_signer(ap.clone(), address);
                Ok(())
            } else {
                cwarn!(MINER, "No account provider");
                Err(AccountProviderError::NotFound)
            }
        } else {
            Ok(())
        }
    }

    fn set_extra_data(&self, extra_data: Bytes) {
        self.params.apply(|params| params.extra_data = extra_data);
    }

    fn transactions_limit(&self) -> usize {
        self.mem_pool.read().limit()
    }

    fn set_transactions_limit(&self, limit: usize) {
        self.mem_pool.write().set_limit(limit)
    }

    fn chain_new_blocks<C>(
        &self,
        chain: &C,
        _imported: &[BlockHash],
        _invalid: &[BlockHash],
        _enacted: &[BlockHash],
        retracted: &[BlockHash],
    ) where
        C: AccountData + BlockChainTrait + BlockProducer + EngineInfo + ImportBlock, {
        ctrace!(MINER, "chain_new_blocks");

        // Then import all transactions...
        {
            let mut mem_pool = self.mem_pool.write();
            for hash in retracted {
                let block = chain.block(&(*hash).into()).expect(
                    "Client is sending message after commit to db and inserting to chain; the block is available; qed",
                );
                let transactions = block.transactions();
                let _ = self.add_transactions_to_pool(chain, transactions, TxOrigin::RetractedBlock, &mut mem_pool);
            }
        }

        // ...and at the end remove the old ones
        {
            let fetch_account = fetch_account_creator(chain);
            let current_block_number = chain.chain_info().best_block_number;
            let current_timestamp = chain.chain_info().best_block_timestamp;
            let mut mem_pool = self.mem_pool.write();
            mem_pool.remove_old(&fetch_account, current_block_number, current_timestamp);
        }

        if !self.options.no_reseal_timer {
            chain.set_min_timer();
        }
    }

    fn can_produce_work_package(&self) -> bool {
        self.engine.seals_internally().is_none()
    }

    fn engine_type(&self) -> EngineType {
        self.engine.engine_type()
    }

    fn prepare_work_sealing<
        C: AccountData + BlockChainTrait + BlockProducer + ChainTimeInfo + EngineInfo + FindActionHandler + TermInfo,
    >(
        &self,
        client: &C,
    ) -> bool {
        ctrace!(MINER, "prepare_work_sealing: entering");
        let prepare_new = {
            let mut sealing_work = self.sealing_work.lock();
            let have_work = sealing_work.queue.peek_last_ref().is_some();
            ctrace!(MINER, "prepare_work_sealing: have_work={}", have_work);
            if !have_work {
                sealing_work.enabled = true;
                true
            } else {
                false
            }
        };
        if prepare_new {
            // --------------------------------------------------------------------------
            // | NOTE Code below requires transaction_queue and sealing_work locks.     |
            // | Make sure to release the locks before calling that method.             |
            // --------------------------------------------------------------------------
            match self.prepare_block(BlockId::Latest, client) {
                Ok(Some((block, original_work_hash))) => {
                    self.prepare_work(block, original_work_hash);
                }
                Ok(None) => {
                    ctrace!(MINER, "prepare_work_sealing: cannot prepare block");
                }
                Err(err) => {
                    ctrace!(MINER, "prepare_work_sealing: cannot prepare block: {:?}", err);
                }
            }
        }

        let best_number = client.chain_info().best_block_number;
        let prev_request = self.sealing_block_last_request.set(best_number);
        if prev_request != best_number {
            ctrace!(
                MINER,
                "prepare_work_sealing: Miner received request (was {}, now {}) - waking up.",
                prev_request,
                best_number
            );
        }

        // Return if we restarted
        prepare_new
    }

    fn update_sealing<C>(&self, chain: &C, parent_block: BlockId, allow_empty_block: bool)
    where
        C: AccountData
            + BlockChainTrait
            + BlockProducer
            + EngineInfo
            + ImportBlock
            + ChainTimeInfo
            + FindActionHandler
            + TermInfo, {
        ctrace!(MINER, "update_sealing: preparing a block");

        let parent_block_number = chain.block_header(&parent_block).expect("Parent is always exist").number();
        if self.requires_reseal(parent_block_number) {
            let (block, original_work_hash) = match self.prepare_block(parent_block, chain) {
                Ok(Some((block, original_work_hash))) => {
                    if !allow_empty_block && block.block().transactions().is_empty() {
                        ctrace!(MINER, "update_sealing: block is empty, and allow_empty_block is false");
                        return
                    }
                    (block, original_work_hash)
                }
                Ok(None) => {
                    ctrace!(MINER, "update_sealing: cannot prepare block");
                    return
                }
                Err(err) => {
                    ctrace!(MINER, "update_sealing: cannot prepare block: {:?}", err);
                    return
                }
            };

            match self.engine.seals_internally() {
                Some(true) => {
                    ctrace!(MINER, "update_sealing: engine indicates internal sealing");
                    if self.seal_and_import_block_internally(chain, block) {
                        ctrace!(MINER, "update_sealing: imported internally sealed block");
                    }
                }
                Some(false) => {
                    ctrace!(MINER, "update_sealing: engine is not keen to seal internally right now");
                    return
                }
                None => {
                    ctrace!(MINER, "update_sealing: engine does not seal internally, preparing work");
                    self.prepare_work(block, original_work_hash);
                    // Set the reseal max timer, for creating empty blocks every reseal_max_period
                    // Not related to next_mandatory_reseal, which is used in seal_and_import_block_internally
                    if !self.options.no_reseal_timer {
                        chain.set_max_timer();
                    }
                }
            }

            // Sealing successful
            self.next_allowed_reseal.set(Instant::now() + self.options.reseal_min_period);
            if !self.options.no_reseal_timer {
                chain.set_min_timer();
            }
        }
    }

    fn submit_seal<C: ImportBlock>(&self, chain: &C, block_hash: BlockHash, seal: Vec<Bytes>) -> Result<(), Error> {
        let result = if let Some(b) = self.sealing_work.lock().queue.take_used_if(|b| b.hash() == *block_hash) {
            ctrace!(
                MINER,
                "Submitted block {}={}={} with seal {:?}",
                block_hash,
                b.hash(),
                b.header().bare_hash(),
                seal
            );
            b.lock().try_seal(&*self.engine, seal).or_else(|(e, _)| {
                cwarn!(MINER, "Mined solution rejected: {}", e);
                Err(Error::PowInvalid)
            })
        } else {
            cwarn!(MINER, "Submitted solution rejected: Block unknown or out of date.");
            Err(Error::PowHashInvalid)
        };
        result.and_then(|sealed| {
            let n = sealed.header().number();
            let h = sealed.header().hash();
            chain.import_sealed_block(&sealed)?;
            cinfo!(MINER, "Submitted block imported OK. #{}: {}", n, h);
            Ok(())
        })
    }

    fn map_sealing_work<C, F, T>(&self, client: &C, f: F) -> Option<T>
    where
        C: AccountData + BlockChainTrait + BlockProducer + ChainTimeInfo + EngineInfo + FindActionHandler + TermInfo,
        F: FnOnce(&ClosedBlock) -> T, {
        ctrace!(MINER, "map_sealing_work: entering");
        self.prepare_work_sealing(client);
        ctrace!(MINER, "map_sealing_work: sealing prepared");
        let mut sealing_work = self.sealing_work.lock();
        let ret = sealing_work.queue.use_last_ref();
        ctrace!(MINER, "map_sealing_work: leaving use_last_ref={:?}", ret.as_ref().map(|b| b.block().header().hash()));
        ret.map(f)
    }

    fn import_external_transactions<C: MiningBlockChainClient + EngineInfo + TermInfo>(
        &self,
        client: &C,
        transactions: Vec<UnverifiedTransaction>,
    ) -> Vec<Result<TransactionImportResult, Error>> {
        ctrace!(EXTERNAL_TX, "Importing external transactions");
        let results = {
            let mut mem_pool = self.mem_pool.write();
            self.add_transactions_to_pool(client, transactions, TxOrigin::External, &mut mem_pool)
        };

        if !results.is_empty()
            && self.options.reseal_on_external_transaction
            && self.transaction_reseal_allowed()
            && !self.engine_type().ignore_reseal_on_transaction()
        {
            // ------------------------------------------------------------------
            // | NOTE Code below requires mem_pool and sealing_queue locks.     |
            // | Make sure to release the locks before calling that method.     |
            // ------------------------------------------------------------------
            self.update_sealing(client, BlockId::Latest, false);
        }
        results
    }

    fn import_own_transaction<C: MiningBlockChainClient + EngineInfo + TermInfo>(
        &self,
        chain: &C,
        tx: SignedTransaction,
    ) -> Result<TransactionImportResult, Error> {
        ctrace!(OWN_TX, "Importing transaction: {:?}", tx);

        let imported = {
            // Be sure to release the lock before we call prepare_work_sealing
            let mut mem_pool = self.mem_pool.write();
            // We need to re-validate transactions
            let import = self
                .add_transactions_to_pool(chain, vec![tx.into()], TxOrigin::Local, &mut mem_pool)
                .pop()
                .expect("one result returned per added transaction; one added => one result; qed");

            match import {
                Ok(_) => {
                    ctrace!(OWN_TX, "Status: {:?}", mem_pool.status());
                }
                Err(ref e) => {
                    ctrace!(OWN_TX, "Status: {:?}", mem_pool.status());
                    cwarn!(OWN_TX, "Error importing transaction: {:?}", e);
                }
            }
            import
        };

        // ------------------------------------------------------------------
        // | NOTE Code below requires mem_pool and sealing_queue locks.     |
        // | Make sure to release the locks before calling that method.     |
        // ------------------------------------------------------------------
        if imported.is_ok() && self.options.reseal_on_own_transaction && self.transaction_reseal_allowed() && !self.engine_type().ignore_reseal_on_transaction()
            // Make sure to do it after transaction is imported and lock is dropped.
            // We need to create pending block and enable sealing.
            && (self.engine.seals_internally().unwrap_or(false) || !self.prepare_work_sealing(chain))
        {
            // If new block has not been prepared (means we already had one)
            // or Engine might be able to seal internally,
            // we need to update sealing.
            self.update_sealing(chain, BlockId::Latest, false);
        }
        imported
    }

    fn import_incomplete_transaction<C: MiningBlockChainClient + AccountData + EngineInfo + TermInfo>(
        &self,
        client: &C,
        account_provider: &AccountProvider,
        tx: IncompleteTransaction,
        platform_address: PlatformAddress,
        passphrase: Option<Password>,
        seq: Option<u64>,
    ) -> Result<(TxHash, u64), Error> {
        let address = platform_address.try_into_address()?;
        let seq = match seq {
            Some(seq) => seq,
            None => {
                let addresses: Vec<_> = {
                    let owner_address = client.latest_regular_key_owner(&address);
                    let regular_key_address = client.latest_regular_key(&address).map(|key| public_to_address(&key));
                    once(address).chain(owner_address.into_iter()).chain(regular_key_address.into_iter()).collect()
                };
                get_next_seq(self.future_transactions(), &addresses)
                    .map(|seq| {
                        cwarn!(RPC, "There are future transactions for {}", platform_address);
                        seq
                    })
                    .unwrap_or_else(|| {
                        const DEFAULT_RANGE: Range<u64> = 0..::std::u64::MAX;
                        get_next_seq(self.ready_transactions(DEFAULT_RANGE).transactions, &addresses)
                            .map(|seq| {
                                cdebug!(RPC, "There are ready transactions for {}", platform_address);
                                seq
                            })
                            .unwrap_or_else(|| client.latest_seq(&address))
                    })
            }
        };
        let tx = tx.complete(seq);
        let tx_hash = tx.hash();
        let sig = account_provider.get_account(&address, passphrase.as_ref())?.sign(&tx_hash)?;
        let unverified = UnverifiedTransaction::new(tx, sig);
        let signed = SignedTransaction::try_new(unverified)?;
        let hash = signed.hash();
        self.import_own_transaction(client, signed)?;

        Ok((hash, seq))
    }

    fn ready_transactions(&self, range: Range<u64>) -> PendingSignedTransactions {
        // FIXME: Update the body size when the common params are updated
        let max_body_size = self.engine.machine().genesis_common_params().max_body_size();
        self.mem_pool.read().top_transactions(max_body_size, None, range)
    }

    fn count_pending_transactions(&self, range: Range<u64>) -> usize {
        self.mem_pool.read().count_pending_transactions(range)
    }

    fn future_included_count_pending_transactions(&self, range: Range<u64>) -> usize {
        self.mem_pool.read().future_included_count_pending_transactions(range)
    }

    fn future_ready_transactions(&self, range: Range<u64>) -> PendingSignedTransactions {
        let max_body_size = self.engine.machine().genesis_common_params().max_body_size();
        self.mem_pool.read().get_future_pending_transactions(max_body_size, None, range)
    }
    /// Get a list of all future transactions.
    fn future_transactions(&self) -> Vec<SignedTransaction> {
        self.mem_pool.read().future_transactions()
    }

    fn start_sealing<C: MiningBlockChainClient + EngineInfo + TermInfo>(&self, client: &C) {
        cdebug!(MINER, "Start sealing");
        self.sealing_enabled.store(true, Ordering::Relaxed);
        // ------------------------------------------------------------------
        // | NOTE Code below requires mem_pool and sealing_queue locks.     |
        // | Make sure to release the locks before calling that method.     |
        // ------------------------------------------------------------------
        if self.transaction_reseal_allowed() {
            cdebug!(MINER, "Update sealing");
            self.update_sealing(client, BlockId::Latest, true);
        }
    }

    fn stop_sealing(&self) {
        cdebug!(MINER, "Stop sealing");
        self.sealing_enabled.store(false, Ordering::Relaxed);
    }

    fn get_malicious_users(&self) -> Vec<Address> {
        self.malicious_users.cloned()
    }

    fn release_malicious_users(&self, prisoner_vec: Vec<Address>) {
        self.malicious_users.remove_users(prisoner_vec.iter());
    }

    fn imprison_malicious_users(&self, prisoner_vec: Vec<Address>) {
        self.malicious_users.remove_users(prisoner_vec.iter());
    }

    fn get_immune_users(&self) -> Vec<Address> {
        self.immune_users.cloned()
    }

    fn register_immune_users(&self, immune_user_vec: Vec<Address>) {
        self.immune_users.remove_users(immune_user_vec.iter())
    }
}

fn get_next_seq(transactions: impl IntoIterator<Item = SignedTransaction>, addresses: &[Address]) -> Option<u64> {
    let mut txes = transactions
        .into_iter()
        .filter(|tx| addresses.contains(&public_to_address(&tx.signer_public())))
        .map(|tx| tx.seq);
    if let Some(first) = txes.next() {
        Some(txes.fold(first, std::cmp::max) + 1)
    } else {
        None
    }
}

#[cfg(test)]
pub mod test {
    use cio::IoService;
    use ckey::{Private, Signature};
    use ctimer::TimerLoop;
    use ctypes::transaction::Transaction;
    use primitives::H512;

    use super::super::super::client::ClientConfig;
    use super::super::super::service::ClientIoMessage;
    use super::super::super::transaction::{SignedTransaction, UnverifiedTransaction};
    use super::*;
    use crate::client::Client;
    use crate::db::NUM_COLUMNS;

    #[test]
    fn check_add_transactions_result_idx() {
        let db = Arc::new(kvdb_memorydb::create(NUM_COLUMNS.unwrap()));
        let scheme = Scheme::new_test();
        let miner = Arc::new(Miner::with_scheme(&scheme, db.clone()));

        let mut mem_pool = MemPool::with_limits(8192, usize::max_value(), 3, db.clone(), Default::default());
        let client = generate_test_client(db, Arc::clone(&miner), &scheme).unwrap();

        let private: Private = H256::random().into();
        let transaction1: UnverifiedTransaction = SignedTransaction::new_with_sign(
            Transaction {
                seq: 30,
                fee: 40,
                network_id: "tc".into(),
                action: Action::SetRegularKey {
                    key: H512::random(),
                },
            },
            &private,
        )
        .into();

        // Invalid signature transaction which will be rejected before mem_pool.add
        let transaction2 = UnverifiedTransaction::new(
            Transaction {
                seq: 32,
                fee: 40,
                network_id: "tc".into(),
                action: Action::SetRegularKey {
                    key: H512::random(),
                },
            },
            Signature::random(),
        );

        let transactions = vec![transaction1.clone(), transaction2, transaction1];
        miner.add_transactions_to_pool(client.as_ref(), transactions, TxOrigin::Local, &mut mem_pool);
    }

    fn generate_test_client(db: Arc<dyn KeyValueDB>, miner: Arc<Miner>, scheme: &Scheme) -> Result<Arc<Client>, Error> {
        let timer_loop = TimerLoop::new(2);

        let client_config: ClientConfig = Default::default();
        let reseal_timer = timer_loop.new_timer_with_name("Client reseal timer");
        let io_service = IoService::<ClientIoMessage>::start("Client")?;

        Client::try_new(&client_config, scheme, db, miner, io_service.channel(), reseal_timer)
    }
}
