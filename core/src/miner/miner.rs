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
use std::iter::once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ckey::{public_to_address, Address, Password, PlatformAddress, Public};
use cstate::{StateError, TopLevelState};
use ctypes::parcel::{Action, Error as ParcelError, IncompleteParcel};
use ctypes::transaction::{Error as TransactionError, Timelock, Transaction};
use ctypes::BlockNumber;
use cvm::ChainTimeInfo;
use parking_lot::{Mutex, RwLock};
use primitives::{Bytes, H256};

use super::mem_pool::{AccountDetails, MemPool, ParcelOrigin, ParcelTimelock, RemovalReason};
use super::sealing_queue::SealingQueue;
use super::work_notify::{NotifyWork, WorkPoster};
use super::{MinerService, MinerStatus, ParcelImportResult};
use crate::account_provider::{AccountProvider, SignError};
use crate::block::{Block, ClosedBlock, IsBlock};
use crate::client::{
    AccountData, BlockChain, BlockProducer, ImportSealedBlock, MiningBlockChainClient, RegularKey, RegularKeyOwner,
    ResealTimer,
};
use crate::consensus::{CodeChainEngine, EngineType};
use crate::encoded;
use crate::error::Error;
use crate::header::Header;
use crate::parcel::{SignedParcel, UnverifiedParcel};
use crate::scheme::Scheme;
use crate::types::{BlockId, ParcelId};

/// Configures the behaviour of the miner.
#[derive(Debug, PartialEq)]
pub struct MinerOptions {
    /// URLs to notify when there is new work.
    pub new_work_notify: Vec<String>,
    /// Force the miner to reseal, even when nobody has asked for work.
    pub force_sealing: bool,
    /// Reseal on receipt of new external parcels.
    pub reseal_on_external_parcel: bool,
    /// Reseal on receipt of new local parcels.
    pub reseal_on_own_parcel: bool,
    /// Minimum period between parcel-inspired reseals.
    pub reseal_min_period: Duration,
    /// Maximum period between blocks (enables force sealing after that).
    pub reseal_max_period: Duration,
    /// Maximum size of the mem pool.
    pub mem_pool_size: usize,
    /// Maximum memory usage of parcels in the queue (current / future).
    pub mem_pool_memory_limit: Option<usize>,
    /// How many historical work packages can we store before running out?
    pub work_queue_size: usize,
}

impl Default for MinerOptions {
    fn default() -> Self {
        MinerOptions {
            new_work_notify: vec![],
            force_sealing: false,
            reseal_on_external_parcel: true,
            reseal_on_own_parcel: true,
            reseal_min_period: Duration::from_secs(2),
            reseal_max_period: Duration::from_secs(120),
            mem_pool_size: 8192,
            mem_pool_memory_limit: Some(2 * 1024 * 1024),
            work_queue_size: 20,
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

type ParcelListener = Box<Fn(&[H256]) + Send + Sync>;

pub struct Miner {
    mem_pool: Arc<RwLock<MemPool>>,
    parcel_listener: RwLock<Vec<ParcelListener>>,
    next_allowed_reseal: Mutex<Instant>,
    next_mandatory_reseal: RwLock<Instant>,
    sealing_block_last_request: Mutex<u64>,
    sealing_work: Mutex<SealingWork>,
    params: RwLock<AuthoringParams>,
    engine: Arc<CodeChainEngine>,
    options: MinerOptions,

    sealing_enabled: AtomicBool,

    accounts: Option<Arc<AccountProvider>>,
    notifiers: RwLock<Vec<Box<NotifyWork>>>,
}

impl Miner {
    /// Push listener that will handle new jobs
    pub fn add_work_listener(&self, notifier: Box<NotifyWork>) {
        self.notifiers.write().push(notifier);
    }

    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    pub fn new(options: MinerOptions, scheme: &Scheme, accounts: Option<Arc<AccountProvider>>) -> Arc<Self> {
        Arc::new(Self::new_raw(options, scheme, accounts))
    }

    pub fn with_scheme(scheme: &Scheme) -> Self {
        Self::new_raw(Default::default(), scheme, None)
    }

    fn new_raw(options: MinerOptions, scheme: &Scheme, accounts: Option<Arc<AccountProvider>>) -> Self {
        let mem_limit = options.mem_pool_memory_limit.unwrap_or_else(usize::max_value);
        let mem_pool = Arc::new(RwLock::new(MemPool::with_limits(options.mem_pool_size, mem_limit)));
        let notifiers: Vec<Box<NotifyWork>> = if options.new_work_notify.is_empty() {
            Vec::new()
        } else {
            vec![Box::new(WorkPoster::new(&options.new_work_notify))]
        };

        Self {
            mem_pool,
            parcel_listener: RwLock::new(vec![]),
            next_allowed_reseal: Mutex::new(Instant::now()),
            next_mandatory_reseal: RwLock::new(Instant::now() + options.reseal_max_period),
            params: RwLock::new(AuthoringParams::default()),
            sealing_block_last_request: Mutex::new(0),
            sealing_work: Mutex::new(SealingWork {
                queue: SealingQueue::new(options.work_queue_size),
                enabled: options.force_sealing || scheme.engine.seals_internally().is_some(),
            }),
            engine: scheme.engine.clone(),
            options,
            sealing_enabled: AtomicBool::new(true),
            accounts,
            notifiers: RwLock::new(notifiers),
        }
    }

    /// Set a callback to be notified about imported parcels' hashes.
    pub fn add_parcels_listener(&self, f: Box<Fn(&[H256]) + Send + Sync>) {
        self.parcel_listener.write().push(f);
    }

    /// Get `Some` `clone()` of the current pending block's state or `None` if we're not sealing.
    pub fn pending_state(&self, latest_block_number: BlockNumber) -> Option<TopLevelState> {
        self.map_pending_block(|b| b.state().clone(), latest_block_number)
    }

    /// Get `Some` `clone()` of the current pending block or `None` if we're not sealing.
    pub fn pending_block(&self, latest_block_number: BlockNumber) -> Option<Block> {
        self.map_pending_block(|b| b.to_base(), latest_block_number)
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
        let has_local_parcels = self.mem_pool.read().has_local_pending_parcels();
        let mut sealing_work = self.sealing_work.lock();
        if sealing_work.enabled {
            ctrace!(MINER, "requires_reseal: sealing enabled");
            let last_request = *self.sealing_block_last_request.lock();
            let should_disable_sealing = !self.options.force_sealing
                && !has_local_parcels
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
                ctrace!(MINER, "Miner sleeping");
                sealing_work.enabled = false;
                sealing_work.queue.reset();
                false
            } else {
                true
            }
        } else {
            ctrace!(MINER, "requires_reseal: sealing is disabled");
            false
        }
    }

    fn add_parcels_to_pool<C: AccountData + BlockChain + RegularKeyOwner>(
        &self,
        client: &C,
        parcels: Vec<UnverifiedParcel>,
        default_origin: ParcelOrigin,
        mem_pool: &mut MemPool,
    ) -> Vec<Result<ParcelImportResult, Error>> {
        let best_block_header = client.best_block_header().decode();
        let insertion_time = client.chain_info().best_block_number;
        let mut inserted = Vec::with_capacity(parcels.len());

        let results = parcels
            .into_iter()
            .map(|parcel| {
                let hash = parcel.hash();
                if client.parcel_block(&ParcelId::Hash(hash)).is_some() {
                    cdebug!(MINER, "Rejected parcel {:?}: already in the blockchain", hash);
                    return Err(StateError::from(ParcelError::ParcelAlreadyImported).into())
                }
                match self
                    .engine
                    .verify_parcel_basic(&parcel, &best_block_header)
                    .and_then(|_| self.engine.verify_parcel_unordered(parcel, &best_block_header))
                {
                    Err(e) => {
                        cdebug!(MINER, "Rejected parcel {:?} with invalid signature: {:?}", hash, e);
                        Err(e)
                    }
                    Ok(parcel) => {
                        // This check goes here because verify_parcel takes SignedParcel parameter
                        self.engine.machine().verify_parcel(&parcel, &best_block_header, client, false)?;

                        let origin = self
                            .accounts
                            .as_ref()
                            .and_then(|accounts| match accounts.has_public(&parcel.signer_public()) {
                                Ok(true) => Some(ParcelOrigin::Local),
                                Ok(false) => None,
                                Err(_) => None,
                            })
                            .unwrap_or(default_origin);

                        let fetch_account = |p: &Public| -> AccountDetails {
                            let address = public_to_address(p);
                            let a = client.latest_regular_key_owner(&address).unwrap_or(address);
                            AccountDetails {
                                seq: client.latest_seq(&a),
                                balance: client.latest_balance(&a),
                            }
                        };

                        let hash = parcel.hash();
                        let timestamp = client.chain_info().best_block_timestamp;
                        let timelock = self.calculate_timelock(&parcel, client)?;
                        let result = mem_pool
                            .add(parcel, origin, insertion_time, timestamp, timelock, &fetch_account)
                            .map_err(StateError::from)?;

                        inserted.push(hash);
                        Ok(result)
                    }
                }
            })
            .collect();

        for listener in &*self.parcel_listener.read() {
            listener(&inserted);
        }

        results
    }

    fn calculate_timelock<C: BlockChain>(&self, parcel: &SignedParcel, client: &C) -> Result<ParcelTimelock, Error> {
        let mut max_block = None;
        let mut max_timestamp = None;
        if let Action::AssetTransaction {
            transaction,
            ..
        } = &parcel.action
        {
            if let Transaction::AssetTransfer {
                inputs,
                ..
            } = transaction
            {
                for input in inputs {
                    if let Some(timelock) = input.timelock {
                        let (is_block_number, value) = match timelock {
                            Timelock::Block(value) => (true, value),
                            Timelock::BlockAge(value) => (
                                true,
                                client.transaction_block_number(&input.prev_out.transaction_hash).ok_or_else(|| {
                                    Error::State(StateError::Transaction(TransactionError::Timelocked {
                                        timelock,
                                        remaining_time: u64::max_value(),
                                    }))
                                })? + value,
                            ),
                            Timelock::Time(value) => (false, value),
                            Timelock::TimeAge(value) => (
                                false,
                                client.transaction_block_timestamp(&input.prev_out.transaction_hash).ok_or_else(
                                    || {
                                        Error::State(StateError::Transaction(TransactionError::Timelocked {
                                            timelock,
                                            remaining_time: u64::max_value(),
                                        }))
                                    },
                                )? + value,
                            ),
                        };
                        if is_block_number {
                            if max_block.is_none() || max_block.expect("The previous guard ensures") < value {
                                max_block = Some(value);
                            }
                        } else if max_timestamp.is_none() || max_timestamp.expect("The previous guard ensures") < value
                        {
                            max_timestamp = Some(value);
                        }
                    }
                }
            }
        };
        Ok(ParcelTimelock {
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
                let pow_hash = block.block().header().hash();
                let number = block.block().header().number();
                let score = *block.block().header().score();
                let is_new = original_work_hash.map_or(true, |h| block.block().header().hash() != h);
                sealing_work.queue.push(block);
                // If push notifications are enabled we assume all work items are used.
                if !self.notifiers.read().is_empty() && is_new {
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
                for notifier in self.notifiers.read().iter() {
                    notifier.notify(pow_hash, target)
                }
            }
        }
    }

    /// Prepares new block for sealing including top parcels from queue.
    fn prepare_block<C: AccountData + BlockChain + BlockProducer + RegularKeyOwner + ChainTimeInfo>(
        &self,
        chain: &C,
    ) -> Result<(ClosedBlock, Option<H256>), Error> {
        let (parcels, mut open_block, original_work_hash) = {
            let max_body_size = self.engine.params().max_body_size;
            let parcels = self.mem_pool.read().top_parcels(max_body_size);
            let mut sealing_work = self.sealing_work.lock();
            let last_work_hash = sealing_work.queue.peek_last_ref().map(|pb| pb.block().header().hash());

            ctrace!(MINER, "prepare_block: No existing work - making new block");
            let params = self.params.read().clone();
            let open_block = chain.prepare_open_block(params.author, params.extra_data);

            (parcels, open_block, last_work_hash)
        };

        let mut invalid_parcels = HashSet::new();
        let block_number = open_block.block().header().number();

        let mut parcel_count: usize = 0;
        let parcel_total = parcels.len();
        for parcel in parcels {
            let hash = parcel.hash();
            let start = Instant::now();
            // Check whether parcel type is allowed for sender
            let result = self
                .engine
                .machine()
                .verify_parcel(&parcel, open_block.header(), chain, true)
                .and_then(|_| open_block.push_parcel(parcel, None, chain));

            match result {
                // already have parcel - ignore
                Err(Error::State(StateError::Parcel(ParcelError::ParcelAlreadyImported))) => {}
                Err(e) => {
                    invalid_parcels.insert(hash);
                    cdebug!(
                        MINER,
                        "Error adding parcel to block: number={}. parcel_hash={:?}, Error: {:?}",
                        block_number,
                        hash,
                        e
                    );
                }
                Ok(()) => {
                    let took = start.elapsed();
                    ctrace!(MINER, "Adding parcel {:?} took {:?}", hash, took);
                    parcel_count += 1;
                } // imported ok
            }
        }
        ctrace!(MINER, "Pushed {}/{} parcels", parcel_count, parcel_total);

        let (parcels_root, invoices_root) = {
            let parent_hash = open_block.header().parent_hash();
            let parent_header = chain.block_header(&BlockId::Hash(*parent_hash)).expect("Parent header MUST exist");
            let parent_view = parent_header.view();
            (parent_view.parcels_root(), parent_view.invoices_root())
        };
        let block = open_block.close(parcels_root, invoices_root)?;

        let fetch_seq = |p: &Public| {
            let address = public_to_address(p);
            let a = chain.latest_regular_key_owner(&address).unwrap_or(address);
            chain.latest_seq(&a)
        };

        {
            let mut queue = self.mem_pool.write();
            for hash in invalid_parcels {
                queue.remove(
                    &hash,
                    &fetch_seq,
                    RemovalReason::Invalid,
                    chain.chain_info().best_block_number,
                    chain.chain_info().best_block_timestamp,
                );
            }
        }
        Ok((block, original_work_hash))
    }

    /// Attempts to perform internal sealing (one that does not require work) and handles the result depending on the type of Seal.
    fn seal_and_import_block_internally<C>(&self, chain: &C, block: ClosedBlock) -> bool
    where
        C: BlockChain + ImportSealedBlock, {
        if block.parcels().is_empty()
            && !self.options.force_sealing
            && Instant::now() <= *self.next_mandatory_reseal.read()
        {
            ctrace!(MINER, "seal_block_internally: no sealing.");
            return false
        }
        ctrace!(MINER, "seal_block_internally: attempting internal seal.");

        let parent_header = match chain.block_header(&(*block.header().parent_hash()).into()) {
            Some(hdr) => hdr.decode(),
            None => return false,
        };

        match self.engine.generate_seal(block.block(), &parent_header).seal_fields() {
            Some(seal) => {
                *self.next_mandatory_reseal.write() = Instant::now() + self.options.reseal_max_period;
                if self.engine.is_proposal(block.header()) {
                    block
                        .lock()
                        .seal(&*self.engine, seal.clone())
                        .map(|sealed| {
                            self.engine.proposal_generated(&sealed);
                            let import_result = chain.import_sealed_block(&sealed);
                            self.engine.broadcast_proposal_block(encoded::Block::new(sealed.rlp_bytes()));
                            import_result
                        })
                        .map_err(|e| {
                            cwarn!(MINER, "ERROR: seal failed when given internally generated seal: {}", e);
                        })
                        .is_ok()
                } else {
                    block
                        .lock()
                        .seal(&*self.engine, seal)
                        .map(|sealed| chain.import_sealed_block(&sealed).is_ok())
                        .unwrap_or_else(|e| {
                            cwarn!(MINER, "ERROR: seal failed when given internally generated seal: {}", e);
                            false
                        })
                }
            }
            None => {
                ctrace!(MINER, "No seal is generated.");
                false
            }
        }
    }

    /// Are we allowed to do a non-mandatory reseal?
    fn parcel_reseal_allowed(&self) -> bool {
        self.sealing_enabled.load(Ordering::Relaxed) && (Instant::now() > *self.next_allowed_reseal.lock())
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
}

const SEALING_TIMEOUT_IN_BLOCKS: u64 = 5;

impl MinerService for Miner {
    type State = TopLevelState;

    fn status(&self) -> MinerStatus {
        let status = self.mem_pool.read().status();
        let sealing_work = self.sealing_work.lock();
        MinerStatus {
            parcels_in_pending_queue: status.pending,
            parcels_in_future_queue: status.future,
            parcels_in_pending_block: sealing_work.queue.peek_last_ref().map_or(0, |b| b.parcels().len()),
        }
    }

    fn authoring_params(&self) -> AuthoringParams {
        self.params.read().clone()
    }

    fn set_author(&self, address: Address, password: Option<Password>) -> Result<(), SignError> {
        self.params.write().author = address;

        if self.engine_type() == EngineType::InternalSealing && self.engine.seals_internally().is_some() {
            if let Some(ref ap) = self.accounts {
                ctrace!(MINER, "Set author to {:?}", address);
                // Sign test message
                ap.sign(address, password.clone(), Default::default())?;
                // Limit the scope of the locks.
                {
                    let mut sealing_work = self.sealing_work.lock();
                    sealing_work.enabled = true;
                }
                self.engine.set_signer(ap.clone(), address, password);
                Ok(())
            } else {
                cwarn!(MINER, "No account provider");
                Err(SignError::NotFound)
            }
        } else {
            Ok(())
        }
    }

    fn set_extra_data(&self, extra_data: Bytes) {
        self.params.write().extra_data = extra_data;
    }

    fn minimal_fee(&self) -> u64 {
        self.mem_pool.read().minimal_fee()
    }

    fn set_minimal_fee(&self, min_fee: u64) {
        self.mem_pool.write().set_minimal_fee(min_fee);
    }

    fn parcels_limit(&self) -> usize {
        self.mem_pool.read().limit()
    }

    fn set_parcels_limit(&self, limit: usize) {
        self.mem_pool.write().set_limit(limit)
    }

    fn chain_new_blocks<C>(
        &self,
        chain: &C,
        _imported: &[H256],
        _invalid: &[H256],
        _enacted: &[H256],
        retracted: &[H256],
    ) where
        C: AccountData + BlockChain + BlockProducer + ImportSealedBlock + RegularKeyOwner, {
        ctrace!(MINER, "chain_new_blocks");

        // Then import all parcels...
        {
            let mut mem_pool = self.mem_pool.write();
            for hash in retracted {
                let block = chain.block(&(*hash).into()).expect(
                    "Client is sending message after commit to db and inserting to chain; the block is available; qed",
                );
                let parcels = block.parcels();
                let _ = self.add_parcels_to_pool(chain, parcels, ParcelOrigin::RetractedBlock, &mut mem_pool);
            }
        }

        // ...and at the end remove the old ones
        {
            let fetch_account = |p: &Public| {
                let address = public_to_address(p);
                let a = chain.latest_regular_key_owner(&address).unwrap_or(address);

                AccountDetails {
                    seq: chain.latest_seq(&a),
                    balance: chain.latest_balance(&a),
                }
            };
            let time = chain.chain_info().best_block_number;
            let timestamp = chain.chain_info().best_block_timestamp;
            let mut mem_pool = self.mem_pool.write();
            mem_pool.remove_old(&fetch_account, time, timestamp);
        }
    }

    fn can_produce_work_package(&self) -> bool {
        self.engine.seals_internally().is_none()
    }

    fn engine_type(&self) -> EngineType {
        self.engine.engine_type()
    }

    fn prepare_work_sealing<C: AccountData + BlockChain + BlockProducer + RegularKeyOwner + ChainTimeInfo>(
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
            match self.prepare_block(client) {
                Ok((block, original_work_hash)) => {
                    self.prepare_work(block, original_work_hash);
                }
                Err(err) => {
                    ctrace!(MINER, "prepare_work_sealing: cannot prepare block: {:?}", err);
                }
            }
        }
        let mut sealing_block_last_request = self.sealing_block_last_request.lock();
        let best_number = client.chain_info().best_block_number;
        if *sealing_block_last_request != best_number {
            ctrace!(
                MINER,
                "prepare_work_sealing: Miner received request (was {}, now {}) - waking up.",
                *sealing_block_last_request,
                best_number
            );
            *sealing_block_last_request = best_number;
        }

        // Return if we restarted
        prepare_new
    }

    fn update_sealing<C>(&self, chain: &C, allow_empty_block: bool)
    where
        C: AccountData + BlockChain + BlockProducer + ImportSealedBlock + RegularKeyOwner + ResealTimer + ChainTimeInfo,
    {
        ctrace!(MINER, "update_sealing: preparing a block");

        if self.requires_reseal(chain.chain_info().best_block_number) {
            let (block, original_work_hash) = match self.prepare_block(chain) {
                Ok((block, original_work_hash)) => {
                    if !allow_empty_block && block.block().parcels().is_empty() {
                        ctrace!(MINER, "update_sealing: block is empty, and allow_empty_block is false");
                        return
                    }
                    (block, original_work_hash)
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
                    chain.set_max_timer();
                }
            }

            // Sealing successful
            *self.next_allowed_reseal.lock() = Instant::now() + self.options.reseal_min_period;
            chain.set_min_timer();
        }
    }

    fn submit_seal<C: ImportSealedBlock>(&self, chain: &C, block_hash: H256, seal: Vec<Bytes>) -> Result<(), Error> {
        let result = if let Some(b) = self.sealing_work.lock().queue.take_used_if(|b| b.hash() == block_hash) {
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
        C: AccountData + BlockChain + BlockProducer + RegularKeyOwner + ChainTimeInfo,
        F: FnOnce(&ClosedBlock) -> T, {
        ctrace!(MINER, "map_sealing_work: entering");
        self.prepare_work_sealing(client);
        ctrace!(MINER, "map_sealing_work: sealing prepared");
        let mut sealing_work = self.sealing_work.lock();
        let ret = sealing_work.queue.use_last_ref();
        ctrace!(MINER, "map_sealing_work: leaving use_last_ref={:?}", ret.as_ref().map(|b| b.block().header().hash()));
        ret.map(f)
    }

    fn import_external_parcels<C: MiningBlockChainClient>(
        &self,
        client: &C,
        parcels: Vec<UnverifiedParcel>,
    ) -> Vec<Result<ParcelImportResult, Error>> {
        ctrace!(EXTERNAL_PARCEL, "Importing external parcels");
        let results = {
            let mut mem_pool = self.mem_pool.write();
            self.add_parcels_to_pool(client, parcels, ParcelOrigin::External, &mut mem_pool)
        };

        if !results.is_empty() && self.options.reseal_on_external_parcel && self.parcel_reseal_allowed() {
            // ------------------------------------------------------------------
            // | NOTE Code below requires mem_pool and sealing_queue locks.     |
            // | Make sure to release the locks before calling that method.     |
            // ------------------------------------------------------------------
            self.update_sealing(client, false);
        }
        results
    }

    fn import_own_parcel<C: MiningBlockChainClient>(
        &self,
        chain: &C,
        parcel: SignedParcel,
    ) -> Result<ParcelImportResult, Error> {
        ctrace!(OWN_PARCEL, "Importing parcel: {:?}", parcel);

        let imported = {
            // Be sure to release the lock before we call prepare_work_sealing
            let mut mem_pool = self.mem_pool.write();
            // We need to re-validate parcels
            let import = self
                .add_parcels_to_pool(chain, vec![parcel.into()], ParcelOrigin::Local, &mut mem_pool)
                .pop()
                .expect("one result returned per added parcel; one added => one result; qed");

            match import {
                Ok(_) => {
                    ctrace!(OWN_PARCEL, "Status: {:?}", mem_pool.status());
                }
                Err(ref e) => {
                    ctrace!(OWN_PARCEL, "Status: {:?}", mem_pool.status());
                    cwarn!(OWN_PARCEL, "Error importing parcel: {:?}", e);
                }
            }
            import
        };

        // ------------------------------------------------------------------
        // | NOTE Code below requires mem_pool and sealing_queue locks.     |
        // | Make sure to release the locks before calling that method.     |
        // ------------------------------------------------------------------
        if imported.is_ok() && self.options.reseal_on_own_parcel && self.parcel_reseal_allowed()
            // Make sure to do it after parcel is imported and lock is dropped.
            // We need to create pending block and enable sealing.
            && (self.engine.seals_internally().unwrap_or(false) || !self.prepare_work_sealing(chain))
        {
            // If new block has not been prepared (means we already had one)
            // or Engine might be able to seal internally,
            // we need to update sealing.
            self.update_sealing(chain, false);
        }
        imported
    }

    fn import_incomplete_parcel<C: MiningBlockChainClient + RegularKey + RegularKeyOwner>(
        &self,
        client: &C,
        account_provider: &AccountProvider,
        parcel: IncompleteParcel,
        platform_address: PlatformAddress,
        passphrase: Option<Password>,
        seq: Option<u64>,
    ) -> Result<(H256, u64), Error> {
        let address = platform_address.try_into_address()?;
        let seq = match seq {
            Some(seq) => seq,
            None => {
                let addresses: Vec<_> = {
                    let owner_address = client.latest_regular_key_owner(&address);
                    let regular_key_address = client.latest_regular_key(&address).map(|key| public_to_address(&key));
                    once(address).chain(owner_address.into_iter()).chain(regular_key_address.into_iter()).collect()
                };
                get_next_seq(self.future_parcels().into_iter(), &addresses)
                    .map(|seq| {
                        cerror!(RPC, "There are future parcels for {}", platform_address);
                        seq
                    })
                    .unwrap_or_else(|| {
                        get_next_seq(self.ready_parcels().into_iter(), &addresses)
                            .map(|seq| {
                                cdebug!(RPC, "There are ready parcels for {}", platform_address);
                                seq
                            })
                            .unwrap_or_else(|| client.latest_seq(&address))
                    })
            }
        };
        let parcel = parcel.complete(seq);
        let parcel_hash = parcel.hash();
        let sig = account_provider.sign(address, passphrase, parcel_hash)?;
        let unverified = UnverifiedParcel::new(parcel, sig);
        let signed = SignedParcel::try_new(unverified)?;
        let hash = signed.hash();
        self.import_own_parcel(client, signed)?;

        Ok((hash, seq))
    }

    fn ready_parcels(&self) -> Vec<SignedParcel> {
        let max_body_size = self.engine.params().max_body_size;
        self.mem_pool.read().top_parcels(max_body_size)
    }

    /// Get a list of all future parcels.
    fn future_parcels(&self) -> Vec<SignedParcel> {
        self.mem_pool.read().future_parcels()
    }

    fn start_sealing<C: MiningBlockChainClient>(&self, client: &C) {
        cdebug!(MINER, "Start sealing");
        self.sealing_enabled.store(true, Ordering::Relaxed);
        // ------------------------------------------------------------------
        // | NOTE Code below requires mem_pool and sealing_queue locks.     |
        // | Make sure to release the locks before calling that method.     |
        // ------------------------------------------------------------------
        if self.parcel_reseal_allowed() {
            cdebug!(MINER, "Update sealing");
            self.update_sealing(client, true);
        }
    }

    fn stop_sealing(&self) {
        cdebug!(MINER, "Stop sealing");
        self.sealing_enabled.store(false, Ordering::Relaxed);
    }
}

fn get_next_seq(parcels: impl Iterator<Item = SignedParcel>, addresses: &[Address]) -> Option<u64> {
    let mut seqs: Vec<_> = parcels
        .filter(|parcel| addresses.contains(&public_to_address(&parcel.signer_public())))
        .map(|parcel| parcel.seq)
        .collect();
    seqs.sort();
    seqs.last().map(|seq| seq + 1)
}
