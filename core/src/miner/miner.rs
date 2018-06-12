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

use ctypes::{Address, Bytes, H256, U256};
use parking_lot::{Mutex, RwLock};

use super::super::account_provider::{AccountProvider, SignError};
use super::super::block::{Block, ClosedBlock, IsBlock};
use super::super::client::{AccountData, BlockChain, BlockProducer, ImportSealedBlock, MiningBlockChainClient};
use super::super::consensus::{CodeChainEngine, Seal};
use super::super::error::Error;
use super::super::header::Header;
use super::super::parcel::{ParcelError, SignedParcel, UnverifiedParcel};
use super::super::spec::Spec;
use super::super::state::State;
use super::super::types::{BlockId, BlockNumber, ParcelId};
use super::parcel_queue::{AccountDetails, ParcelOrigin, ParcelQueue, RemovalReason};
use super::sealing_queue::SealingQueue;
use super::{MinerService, MinerStatus, ParcelImportResult};

/// Configures the behaviour of the miner.
#[derive(Debug, PartialEq)]
pub struct MinerOptions {
    /// Reseal on receipt of new external parcels.
    pub reseal_on_external_parcel: bool,
    /// Reseal on receipt of new local parcels.
    pub reseal_on_own_parcel: bool,
    /// Minimum period between parcel-inspired reseals.
    pub reseal_min_period: Duration,
    /// Maximum size of the parcel queue.
    pub parcel_queue_size: usize,
    /// Maximum memory usage of parcels in the queue (current / future).
    pub parcel_queue_memory_limit: Option<usize>,
    /// How many historical work packages can we store before running out?
    pub work_queue_size: usize,
}

impl Default for MinerOptions {
    fn default() -> Self {
        MinerOptions {
            reseal_on_external_parcel: false,
            reseal_on_own_parcel: true,
            reseal_min_period: Duration::from_secs(2),
            parcel_queue_size: 8192,
            parcel_queue_memory_limit: Some(2 * 1024 * 1024),
            work_queue_size: 20,
        }
    }
}

pub struct Miner {
    parcel_queue: Arc<RwLock<ParcelQueue>>,
    parcel_listener: RwLock<Vec<Box<Fn(&[H256]) + Send + Sync>>>,
    next_allowed_reseal: Mutex<Instant>,
    author: RwLock<Address>,
    extra_data: RwLock<Bytes>,
    sealing_queue: Mutex<SealingQueue>,
    engine: Arc<CodeChainEngine>,
    options: MinerOptions,
    accounts: Option<Arc<AccountProvider>>,
}

impl Miner {
    pub fn new(options: MinerOptions, spec: &Spec, accounts: Option<Arc<AccountProvider>>) -> Arc<Self> {
        Arc::new(Self::new_raw(options, spec, accounts))
    }

    pub fn with_spec(spec: &Spec) -> Self {
        Self::new_raw(Default::default(), spec, None)
    }

    fn new_raw(options: MinerOptions, spec: &Spec, accounts: Option<Arc<AccountProvider>>) -> Self {
        let mem_limit = options.parcel_queue_memory_limit.unwrap_or_else(usize::max_value);
        let parcel_queue = Arc::new(RwLock::new(ParcelQueue::with_limits(options.parcel_queue_size, mem_limit)));
        Self {
            parcel_queue,
            parcel_listener: RwLock::new(vec![]),
            next_allowed_reseal: Mutex::new(Instant::now()),
            author: RwLock::new(Address::default()),
            extra_data: RwLock::new(Vec::new()),
            sealing_queue: Mutex::new(SealingQueue::new(options.work_queue_size)),
            engine: spec.engine.clone(),
            options,
            accounts,
        }
    }

    /// Set a callback to be notified about imported parcels' hashes.
    pub fn add_parcels_listener(&self, f: Box<Fn(&[H256]) + Send + Sync>) {
        self.parcel_listener.write().push(f);
    }

    /// Get `Some` `clone()` of the current pending block's state or `None` if we're not sealing.
    pub fn pending_state(&self, latest_block_number: BlockNumber) -> Option<State<::state_db::StateDB>> {
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

    /// Check is reseal is allowed and necessary.
    fn requires_reseal(&self) -> bool {
        let has_local_parcels = self.parcel_queue.read().has_local_pending_parcels();
        let should_disable_sealing = !has_local_parcels && self.engine.seals_internally().is_none();

        ctrace!(MINER, "requires_reseal: should_disable_sealing={}", should_disable_sealing);

        if should_disable_sealing {
            ctrace!(MINER, "Miner sleeping");
            false
        } else {
            // sealing enabled and we don't want to sleep.
            *self.next_allowed_reseal.lock() = Instant::now() + self.options.reseal_min_period;
            true
        }
    }

    fn add_parcels_to_queue<C: AccountData + BlockChain>(
        &self,
        client: &C,
        parcels: Vec<UnverifiedParcel>,
        default_origin: ParcelOrigin,
        parcel_queue: &mut ParcelQueue,
    ) -> Vec<Result<ParcelImportResult, Error>> {
        let best_block_header = client.best_block_header().decode();
        let insertion_time = client.chain_info().best_block_number;
        let mut inserted = Vec::with_capacity(parcels.len());

        let results = parcels
            .into_iter()
            .map(|parcel| {
                let hash = parcel.hash();
                if client.parcel_block(ParcelId::Hash(hash)).is_some() {
                    cdebug!(MINER, "Rejected parcel {:?}: already in the blockchain", hash);
                    return Err(Error::Parcel(ParcelError::AlreadyImported))
                }
                match self.engine
                    .verify_parcel_basic(&parcel, &best_block_header)
                    .and_then(|_| self.engine.verify_parcel_unordered(parcel, &best_block_header))
                {
                    Err(e) => {
                        cdebug!(MINER, "Rejected parcel {:?} with invalid signature: {:?}", hash, e);
                        Err(e)
                    }
                    Ok(parcel) => {
                        // This check goes here because verify_parcel takes SignedParcel parameter
                        self.engine.machine().verify_parcel(&parcel, &best_block_header, client)?;

                        let origin = self.accounts
                            .as_ref()
                            .and_then(|accounts| match accounts.has_account(parcel.sender()) {
                                Ok(true) => Some(ParcelOrigin::Local),
                                Ok(false) => None,
                                Err(_) => None,
                            })
                            .unwrap_or(default_origin);

                        let fetch_account = |a: &Address| -> AccountDetails {
                            AccountDetails {
                                nonce: client.latest_nonce(a),
                                balance: client.latest_balance(a),
                            }
                        };
                        let hash = parcel.hash();
                        let result = parcel_queue.add(parcel, origin, insertion_time, &fetch_account)?;

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

    /// Prepares new block for sealing including top parcels from queue.
    fn prepare_block<C: AccountData + BlockChain + BlockProducer>(&self, chain: &C) -> ClosedBlock {
        let (parcels, mut open_block) = {
            let parcels = self.parcel_queue.read().top_parcels();

            ctrace!(MINER, "prepare_block: No existing work - making new block");
            let open_block = chain.prepare_open_block(self.author(), self.extra_data());

            (parcels, open_block)
        };

        let mut invalid_parcels = HashSet::new();
        let mut non_allowed_parcels = HashSet::new();
        let block_number = open_block.block().header().number();

        let mut parcel_count: usize = 0;
        let parcel_total = parcels.len();
        for parcel in parcels {
            let hash = parcel.hash();
            let start = Instant::now();
            // Check whether parcel type is allowed for sender
            let result = match self.engine.machine().verify_parcel(&parcel, open_block.header(), chain) {
                Err(Error::Parcel(ParcelError::NotAllowed)) => Err(ParcelError::NotAllowed.into()),
                _ => open_block.push_parcel(parcel, None),
            };
            let took = start.elapsed();

            ctrace!(MINER, "Adding parcel {:?} took {:?}", hash, took);
            match result {
                // already have parcel - ignore
                Err(Error::Parcel(ParcelError::AlreadyImported)) => {}
                Err(Error::Parcel(ParcelError::NotAllowed)) => {
                    non_allowed_parcels.insert(hash);
                    cdebug!(MINER, "Skipping non-allowed parcel for sender {:?}", hash);
                }
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
                _ => {
                    parcel_count += 1;
                } // imported ok
            }
        }
        ctrace!(MINER, "Pushed {}/{} parcels", parcel_count, parcel_total);

        let (parcels_root, invoices_root) = {
            let parent_hash = open_block.header().parent_hash();
            let parent_header = chain.block_header(BlockId::Hash(*parent_hash)).expect("Parent header MUST exist");
            let parent_view = parent_header.view();
            (parent_view.parcels_root(), parent_view.invoices_root())
        };
        let block = open_block.close(parcels_root, invoices_root);

        let fetch_nonce = |a: &Address| chain.latest_nonce(a);

        {
            let mut queue = self.parcel_queue.write();
            for hash in invalid_parcels {
                queue.remove(&hash, &fetch_nonce, RemovalReason::Invalid);
            }
            for hash in non_allowed_parcels {
                queue.remove(&hash, &fetch_nonce, RemovalReason::NotAllowed);
            }
        }
        block
    }

    /// Attempts to perform internal sealing (one that does not require work) and handles the result depending on the type of Seal.
    fn seal_and_import_block_internally<C>(&self, chain: &C, block: ClosedBlock) -> bool
    where
        C: BlockChain + ImportSealedBlock, {
        ctrace!(MINER, "seal_block_internally: attempting internal seal.");
        if block.parcels().is_empty() {
            return false
        }

        let parent_header = match chain.block_header((*block.header().parent_hash()).into()) {
            Some(hdr) => hdr.decode(),
            None => return false,
        };

        match self.engine.generate_seal(block.block(), &parent_header) {
            // Save proposal for later seal submission and broadcast it.
            Seal::Proposal(seal) => {
                ctrace!(MINER, "Received a Proposal seal.");
                {
                    let mut sealing_queue = self.sealing_queue.lock();
                    sealing_queue.push(block.clone());
                    sealing_queue.use_last_ref();
                }
                block
                    .lock()
                    .seal(&*self.engine, seal)
                    .map(|sealed| {
                        self.engine.broadcast_proposal_block(sealed);
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
    fn parcel_reseal_allowed(&self) -> bool {
        Instant::now() > *self.next_allowed_reseal.lock()
    }

    fn map_pending_block<F, T>(&self, f: F, latest_block_number: BlockNumber) -> Option<T>
    where
        F: FnOnce(&ClosedBlock) -> T, {
        let sealing_queue = self.sealing_queue.lock();
        sealing_queue.peek_last_ref().and_then(|b| {
            if b.block().header().number() > latest_block_number {
                Some(f(b))
            } else {
                None
            }
        })
    }
}

impl MinerService for Miner {
    type State = State<::state_db::StateDB>;

    fn status(&self) -> MinerStatus {
        let status = self.parcel_queue.read().status();
        let sealing_queue = self.sealing_queue.lock();
        MinerStatus {
            parcels_in_pending_queue: status.pending,
            parcels_in_future_queue: status.future,
            parcels_in_pending_block: sealing_queue.peek_last_ref().map_or(0, |b| b.parcels().len()),
        }
    }

    fn author(&self) -> Address {
        *self.author.read()
    }

    fn set_author(&self, author: Address) {
        ctrace!(MINER, "Set author to {:?}", author);
        *self.author.write() = author;
    }

    fn extra_data(&self) -> Bytes {
        self.extra_data.read().clone()
    }

    fn set_extra_data(&self, extra_data: Bytes) {
        *self.extra_data.write() = extra_data;
    }

    fn set_engine_signer(&self, address: Address) -> Result<(), SignError> {
        if self.engine.seals_internally().is_some() {
            if let Some(ref ap) = self.accounts {
                ctrace!(MINER, "Set engine signer to {:?}", address);
                self.engine.set_signer(ap.clone(), address);
                Ok(())
            } else {
                cwarn!(MINER, "No account provider");
                Err(SignError::NotFound)
            }
        } else {
            cwarn!(MINER, "Cannot set engine signer on a PoW chain.");
            Err(SignError::InappropriateChain)
        }
    }

    fn minimal_fee(&self) -> U256 {
        *self.parcel_queue.read().minimal_fee()
    }

    fn set_minimal_fee(&self, min_fee: U256) {
        self.parcel_queue.write().set_minimal_fee(min_fee);
    }

    fn parcels_limit(&self) -> usize {
        self.parcel_queue.read().limit()
    }

    fn set_parcels_limit(&self, limit: usize) {
        self.parcel_queue.write().set_limit(limit)
    }

    fn chain_new_blocks<C>(
        &self,
        chain: &C,
        _imported: &[H256],
        _invalid: &[H256],
        _enacted: &[H256],
        retracted: &[H256],
    ) where
        C: AccountData + BlockChain + BlockProducer + ImportSealedBlock, {
        ctrace!(MINER, "chain_new_blocks");

        // Then import all parcels...
        {
            let mut parcel_queue = self.parcel_queue.write();
            for hash in retracted {
                let block = chain.block((*hash).into()).expect(
                    "Client is sending message after commit to db and inserting to chain; the block is available; qed",
                );
                let parcels = block.parcels();
                let _ = self.add_parcels_to_queue(chain, parcels, ParcelOrigin::RetractedBlock, &mut parcel_queue);
            }
        }

        // ...and at the end remove the old ones
        {
            let fetch_account = |a: &Address| AccountDetails {
                nonce: chain.latest_nonce(a),
                balance: chain.latest_balance(a),
            };
            let time = chain.chain_info().best_block_number;
            let mut parcel_queue = self.parcel_queue.write();
            parcel_queue.remove_old(&fetch_account, time);
        }
    }

    fn update_sealing<C>(&self, chain: &C)
    where
        C: AccountData + BlockChain + BlockProducer + ImportSealedBlock, {
        ctrace!(MINER, "update_sealing: preparing a block");
        if self.requires_reseal() {
            let block = self.prepare_block(chain);

            match self.engine.seals_internally() {
                Some(true) => {
                    ctrace!(MINER, "update_sealing: engine indicates internal sealing");
                    if self.seal_and_import_block_internally(chain, block) {
                        ctrace!(MINER, "update_sealing: imported internally sealed block");
                    }
                }
                Some(false) => ctrace!(MINER, "update_sealing: engine is not keen to seal internally right now"),
                None => {
                    ctrace!(MINER, "update_sealing: engine does not seal internally, preparing work");
                    unreachable!("External sealing is not supported")
                }
            }
        }
    }

    fn submit_seal<C: ImportSealedBlock>(&self, chain: &C, block_hash: H256, seal: Vec<Bytes>) -> Result<(), Error> {
        let result = if let Some(b) = self.sealing_queue.lock().take_used_if(|b| &b.hash() == &block_hash) {
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
            chain.import_sealed_block(sealed)?;
            cinfo!(MINER, "Submitted block imported OK. #{}: {}", n, h);
            Ok(())
        })
    }

    fn import_external_parcels<C: MiningBlockChainClient>(
        &self,
        client: &C,
        parcels: Vec<UnverifiedParcel>,
    ) -> Vec<Result<ParcelImportResult, Error>> {
        ctrace!(EXTERNAL_PARCEL, "Importing external parcels");
        let results = {
            let mut parcel_queue = self.parcel_queue.write();
            self.add_parcels_to_queue(client, parcels, ParcelOrigin::External, &mut parcel_queue)
        };

        if !results.is_empty() && self.options.reseal_on_external_parcel && self.parcel_reseal_allowed() {
            // ------------------------------------------------------------------
            // | NOTE Code below requires parcel_queue and sealing_queue locks. |
            // | Make sure to release the locks before calling that method.     |
            // ------------------------------------------------------------------
            self.update_sealing(client);
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
            let mut parcel_queue = self.parcel_queue.write();
            // We need to re-validate parcels
            let import = self.add_parcels_to_queue(chain, vec![parcel.into()], ParcelOrigin::Local, &mut parcel_queue)
                .pop()
                .expect("one result returned per added parcel; one added => one result; qed");

            match import {
                Ok(_) => {
                    ctrace!(OWN_PARCEL, "Status: {:?}", parcel_queue.status());
                }
                Err(ref e) => {
                    ctrace!(OWN_PARCEL, "Status: {:?}", parcel_queue.status());
                    cwarn!(OWN_PARCEL, "Error importing parcel: {:?}", e);
                }
            }
            import
        };

        // ------------------------------------------------------------------
        // | NOTE Code below requires parcel_queue and sealing_queue locks. |
        // | Make sure to release the locks before calling that method.     |
        // ------------------------------------------------------------------
        if imported.is_ok() && self.options.reseal_on_own_parcel && self.parcel_reseal_allowed() {
            // Make sure to do it after parcel is imported and lock is dropped.
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

    fn ready_parcels(&self) -> Vec<SignedParcel> {
        self.parcel_queue.read().top_parcels()
    }

    /// Get a list of all future parcels.
    fn future_parcels(&self) -> Vec<SignedParcel> {
        self.parcel_queue.read().future_parcels()
    }
}
