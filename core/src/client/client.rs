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

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};
use std::time::Instant;

use cbytes::Bytes;
use cio::IoChannel;
use ctypes::{Address, H256, U256};
use journaldb;
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::{Mutex, RwLock};
use rlp::{Encodable, UntrustedRlp};
use trie::{TrieFactory, TrieSpec};

use super::super::block::{enact, ClosedBlock, Drain, IsBlock, LockedBlock, OpenBlock, SealedBlock};
use super::super::blockchain::{
    BlockChain, BlockProvider, BodyProvider, HeaderProvider, ImportRoute, InvoiceProvider, ParcelAddress,
    ParcelInvoices, TransactionAddress,
};
use super::super::consensus::epoch::Transition as EpochTransition;
use super::super::consensus::CodeChainEngine;
use super::super::encoded;
use super::super::error::{BlockImportError, Error, ImportError};
use super::super::header::Header;
use super::super::miner::{Miner, MinerService};
use super::super::parcel::{LocalizedParcel, SignedParcel, UnverifiedParcel};
use super::super::service::ClientIoMessage;
use super::super::spec::Spec;
use super::super::state::State;
use super::super::state_db::StateDB;
use super::super::types::{
    BlockId, BlockNumber, BlockStatus, ParcelId, TransactionId, VerificationQueueInfo as BlockQueueInfo,
};
use super::super::verification::queue::{BlockQueue, HeaderQueue};
use super::super::verification::{self, PreverifiedBlock, Verifier};
use super::super::views::{BlockView, HeaderView};
use super::{
    AccountData, Balance, BlockChain as BlockChainTrait, BlockChainClient, BlockChainInfo, BlockInfo, BlockProducer,
    ChainInfo, ChainNotify, ClientConfig, EngineClient, Error as ClientError, ImportBlock, ImportResult,
    ImportSealedBlock, Invoice, MiningBlockChainClient, Nonce, ParcelInfo, PrepareOpenBlock, ReopenBlock, StateOrBlock,
};

const MAX_PARCEL_QUEUE_SIZE: usize = 4096;

pub struct Client {
    engine: Arc<CodeChainEngine>,

    io_channel: Mutex<IoChannel<ClientIoMessage>>,

    chain: RwLock<Arc<BlockChain>>,

    /// Client uses this to store blocks, traces, etc.
    db: RwLock<Arc<KeyValueDB>>,

    state_db: RwLock<StateDB>,

    /// List of actors to be notified on certain chain events
    notify: RwLock<Vec<Weak<ChainNotify>>>,

    /// Count of pending parcels in the queue
    queue_parcels: AtomicUsize,
    trie_factory: TrieFactory,

    importer: Importer,
}

impl Client {
    pub fn new(
        config: ClientConfig,
        spec: &Spec,
        db: Arc<KeyValueDB>,
        miner: Arc<Miner>,
        message_channel: IoChannel<ClientIoMessage>,
    ) -> Result<Arc<Client>, Error> {
        let trie_spec = match config.fat_db {
            true => TrieSpec::Fat,
            false => TrieSpec::Secure,
        };

        let trie_factory = TrieFactory::new(trie_spec);

        let journal_db = journaldb::new(db.clone(), journaldb::Algorithm::Archive, ::db::COL_STATE);
        let mut state_db = StateDB::new(journal_db, config.state_cache_size);
        if state_db.journal_db().is_empty() {
            // Sets the correct state root.
            state_db = spec.ensure_db_good(state_db, &trie_factory)?;
            let mut batch = DBTransaction::new();
            state_db.journal_under(&mut batch, 0, &spec.genesis_header().hash())?;
            db.write(batch).map_err(ClientError::Database)?;
        }

        let gb = spec.genesis_block();
        let chain = Arc::new(BlockChain::new(&gb, db.clone()));

        let engine = spec.engine.clone();

        let importer = Importer::new(&config, engine.clone(), message_channel.clone(), miner)?;

        let client = Arc::new(Client {
            engine,
            io_channel: Mutex::new(message_channel),
            chain: RwLock::new(chain),
            db: RwLock::new(db),
            state_db: RwLock::new(state_db),
            notify: RwLock::new(Vec::new()),
            queue_parcels: AtomicUsize::new(0),
            trie_factory,
            importer,
        });

        // ensure buffered changes are flushed.
        client.db.read().flush().map_err(ClientError::Database)?;
        Ok(client)
    }

    /// Returns engine reference.
    pub fn engine(&self) -> &CodeChainEngine {
        &*self.engine
    }

    /// Adds an actor to be notified on certain events
    pub fn add_notify(&self, target: Arc<ChainNotify>) {
        self.notify.write().push(Arc::downgrade(&target));
    }

    fn notify<F>(&self, f: F)
    where
        F: Fn(&ChainNotify), {
        for np in self.notify.read().iter() {
            if let Some(n) = np.upgrade() {
                f(&*n);
            }
        }
    }

    /// This is triggered by a message coming from a header queue when the header is ready for insertion
    pub fn import_verified_headers(&self) -> usize {
        self.importer.import_verified_headers(self)
    }

    /// This is triggered by a message coming from a block queue when the block is ready for insertion
    pub fn import_verified_blocks(&self) -> usize {
        self.importer.import_verified_blocks(self)
    }

    fn block_hash(chain: &BlockChain, id: BlockId) -> Option<H256> {
        match id {
            BlockId::Hash(hash) => Some(hash),
            BlockId::Number(number) => chain.block_hash(number),
            BlockId::Earliest => chain.block_hash(0),
            BlockId::Latest => Some(chain.best_block_hash()),
        }
    }

    fn parcel_address(&self, id: ParcelId) -> Option<ParcelAddress> {
        match id {
            ParcelId::Hash(ref hash) => self.chain.read().parcel_address(hash),
            ParcelId::Location(id, index) => Self::block_hash(&self.chain.read(), id).map(|hash| ParcelAddress {
                block_hash: hash,
                index,
            }),
        }
    }

    fn transaction_address(&self, id: TransactionId) -> Option<TransactionAddress> {
        match id {
            TransactionId::Hash(ref hash) => self.chain.read().transaction_address(hash),
            TransactionId::Location(id, index) => self.parcel_address(id).map(|parcel_address| TransactionAddress {
                parcel_address,
                index,
            }),
        }
    }

    /// Import parcels from the IO queue
    pub fn import_queued_parcels(&self, parcels: &[Bytes], peer_id: usize) -> usize {
        trace!(target: "external_parcel", "Importing queued");
        self.queue_parcels.fetch_sub(parcels.len(), AtomicOrdering::SeqCst);
        let parcels: Vec<UnverifiedParcel> =
            parcels.iter().filter_map(|bytes| UntrustedRlp::new(bytes).as_val().ok()).collect();
        let hashes: Vec<_> = parcels.iter().map(|parcel| parcel.hash()).collect();
        self.notify(|notify| {
            notify.parcels_received(hashes.clone(), peer_id);
        });
        let results = self.importer.miner.import_external_parcels(self, parcels);
        results.len()
    }

    fn block_number_ref(&self, id: &BlockId) -> Option<BlockNumber> {
        match id {
            BlockId::Number(number) => Some(number.clone()),
            BlockId::Hash(hash) => self.chain.read().block_number(hash),
            BlockId::Earliest => Some(0),
            BlockId::Latest => Some(self.chain.read().best_block_detail().number),
        }
    }

    /// Get a copy of the best block's state.
    pub fn latest_state(&self) -> State<StateDB> {
        let header = self.best_block_header();
        State::from_existing(
            self.state_db.read().boxed_clone_canon(&header.hash()),
            header.state_root(),
            self.engine.machine().account_start_nonce(),
            self.trie_factory.clone(),
        ).expect("State root of best block header always valid.")
    }

    /// Attempt to get a copy of a specific block's final state.
    ///
    /// This will not fail if given BlockId::Latest.
    /// Otherwise, this can fail (but may not) if the DB prunes state or the block
    /// is unknown.
    pub fn state_at(&self, id: BlockId) -> Option<State<StateDB>> {
        // fast path for latest state.
        match id {
            BlockId::Latest => return Some(self.latest_state()),
            _ => {}
        }

        self.block_header(id).and_then(|header| {
            let db = self.state_db.read().boxed_clone();

            let root = header.state_root();
            State::from_existing(db, root, self.engine.machine().account_start_nonce(), self.trie_factory.clone()).ok()
        })
    }

    pub fn database(&self) -> Arc<KeyValueDB> {
        Arc::clone(&self.db.read())
    }
}

impl ChainInfo for Client {
    fn chain_info(&self) -> BlockChainInfo {
        let mut chain_info = self.chain.read().chain_info();
        chain_info.pending_total_score = chain_info.total_score + self.importer.block_queue.total_score();
        chain_info
    }
}

impl EngineClient for Client {
    /// Make a new block and seal it.
    fn update_sealing(&self) {
        self.importer.miner.update_sealing(self)
    }

    /// Submit a seal for a block in the mining queue.
    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>) {
        if self.importer.miner.submit_seal(self, block_hash, seal).is_err() {
            warn!(target: "poa", "Wrong internal seal submission!")
        }
    }
}

impl BlockInfo for Client {
    fn block_header(&self, id: BlockId) -> Option<::encoded::Header> {
        let chain = self.chain.read();

        Self::block_hash(&chain, id).and_then(|hash| chain.block_header_data(&hash))
    }

    fn best_block_header(&self) -> encoded::Header {
        self.chain.read().best_block_header()
    }

    fn block(&self, id: BlockId) -> Option<encoded::Block> {
        let chain = self.chain.read();

        Self::block_hash(&chain, id).and_then(|hash| chain.block(&hash))
    }
}

impl ParcelInfo for Client {
    fn parcel_block(&self, id: ParcelId) -> Option<H256> {
        self.parcel_address(id).map(|addr| addr.block_hash)
    }
}

impl ImportBlock for Client {
    fn import_block(&self, bytes: Bytes) -> Result<H256, BlockImportError> {
        use super::super::verification::queue::kind::blocks::Unverified;
        use super::super::verification::queue::kind::BlockLike;

        let unverified = Unverified::new(bytes);
        {
            if self.chain.read().is_known(&unverified.hash()) {
                return Err(BlockImportError::Import(ImportError::AlreadyInChain))
            }
        }
        Ok(self.importer.block_queue.import(unverified)?)
    }
}

impl BlockChainTrait for Client {}

impl BlockChainClient for Client {
    fn queue_info(&self) -> BlockQueueInfo {
        self.importer.block_queue.queue_info()
    }

    fn queue_parcels(&self, parcels: Vec<Bytes>, peer_id: usize) {
        let queue_size = self.queue_parcels.load(AtomicOrdering::Relaxed);
        trace!(target: "external_parcel", "Queue size: {}", queue_size);
        if queue_size > MAX_PARCEL_QUEUE_SIZE {
            debug!("Ignoring {} parcels: queue is full", parcels.len());
        } else {
            let len = parcels.len();
            match self.io_channel.lock().send(ClientIoMessage::NewParcels(parcels, peer_id)) {
                Ok(_) => {
                    self.queue_parcels.fetch_add(len, AtomicOrdering::SeqCst);
                }
                Err(e) => {
                    debug!("Ignoring {} parcels: error queueing: {}", len, e);
                }
            }
        }
    }

    fn ready_parcels(&self) -> Vec<SignedParcel> {
        self.importer.miner.ready_parcels()
    }

    fn block_number(&self, id: BlockId) -> Option<BlockNumber> {
        self.block_number_ref(&id)
    }

    fn block_body(&self, id: BlockId) -> Option<encoded::Body> {
        let chain = self.chain.read();

        Self::block_hash(&chain, id).and_then(|hash| chain.block_body(&hash))
    }

    fn block_status(&self, id: BlockId) -> BlockStatus {
        let chain = self.chain.read();
        match Self::block_hash(&chain, id) {
            Some(ref hash) if chain.is_known(hash) => BlockStatus::InChain,
            Some(hash) => self.importer.block_queue.status(&hash).into(),
            None => BlockStatus::Unknown,
        }
    }

    fn block_total_score(&self, id: BlockId) -> Option<U256> {
        let chain = self.chain.read();

        Self::block_hash(&chain, id).and_then(|hash| chain.block_details(&hash)).map(|d| d.total_score)
    }

    fn block_hash(&self, id: BlockId) -> Option<H256> {
        let chain = self.chain.read();
        Self::block_hash(&chain, id)
    }

    fn parcel(&self, id: ParcelId) -> Option<LocalizedParcel> {
        let chain = self.chain.read();
        self.parcel_address(id).and_then(|address| chain.parcel(&address))
    }

    fn parcel_invoices(&self, id: ParcelId) -> Option<ParcelInvoices> {
        let chain = self.chain.read();
        self.parcel_address(id).and_then(|address| chain.parcel_invoices(&address))
    }

    fn transaction_invoice(&self, id: TransactionId) -> Option<Invoice> {
        self.transaction_address(id).and_then(|transaction_address| {
            let parcel_address = transaction_address.parcel_address.clone();
            let parcel_id = parcel_address.into();

            self.parcel_invoices(parcel_id)
                .and_then(|invoices| invoices.invoices.get(transaction_address.index).map(|i| i.clone()))
        })
    }
}

pub struct Importer {
    /// Lock used during block import
    pub import_lock: Mutex<()>, // FIXME Maybe wrap the whole `Importer` instead?

    /// Used to verify blocks
    pub verifier: Box<Verifier<Client>>,

    /// Queue containing pending blocks
    pub block_queue: BlockQueue,

    /// Queue containing pending headers
    pub header_queue: HeaderQueue,

    /// Handles block sealing
    pub miner: Arc<Miner>,

    /// CodeChain engine to be used during import
    pub engine: Arc<CodeChainEngine>,
}

impl Importer {
    pub fn new(
        config: &ClientConfig,
        engine: Arc<CodeChainEngine>,
        message_channel: IoChannel<ClientIoMessage>,
        miner: Arc<Miner>,
    ) -> Result<Importer, Error> {
        let block_queue = BlockQueue::new(
            config.queue.clone(),
            engine.clone(),
            message_channel.clone(),
            config.verifier_type.verifying_seal(),
        );

        let header_queue = HeaderQueue::new(
            config.queue.clone(),
            engine.clone(),
            message_channel.clone(),
            config.verifier_type.verifying_seal(),
        );

        Ok(Importer {
            import_lock: Mutex::new(()),
            verifier: verification::new(config.verifier_type.clone()),
            block_queue,
            header_queue,
            miner,
            engine,
        })
    }

    /// This is triggered by a message coming from a block queue when the block is ready for insertion
    pub fn import_verified_blocks(&self, client: &Client) -> usize {
        let max_blocks_to_import = 4;
        let (imported_blocks, import_results, invalid_blocks, imported, duration, is_empty) = {
            let mut imported_blocks = Vec::with_capacity(max_blocks_to_import);
            let mut invalid_blocks = HashSet::new();
            let mut import_results = Vec::with_capacity(max_blocks_to_import);

            let _import_lock = self.import_lock.lock();
            let blocks = self.block_queue.drain(max_blocks_to_import);
            if blocks.is_empty() {
                return 0
            }
            let start = Instant::now();

            for block in blocks {
                let header = &block.header;
                let is_invalid = invalid_blocks.contains(header.parent_hash());
                if is_invalid {
                    invalid_blocks.insert(header.hash());
                    continue
                }
                if let Ok(closed_block) = self.check_and_close_block(&block, client) {
                    if self.engine.is_proposal(&block.header) {
                        self.block_queue.mark_as_good(&[header.hash()]);
                    } else {
                        imported_blocks.push(header.hash());

                        let route = self.commit_block(closed_block, &header, &block.bytes, client);
                        import_results.push(route);
                    }
                } else {
                    invalid_blocks.insert(header.hash());
                }
            }

            let imported = imported_blocks.len();
            let invalid_blocks = invalid_blocks.into_iter().collect::<Vec<H256>>();

            if !invalid_blocks.is_empty() {
                self.block_queue.mark_as_bad(&invalid_blocks);
            }
            let is_empty = self.block_queue.mark_as_good(&imported_blocks);
            let duration_ns = {
                let elapsed = start.elapsed();
                elapsed.as_secs() * 1_000_000_000 + elapsed.subsec_nanos() as u64
            };
            (imported_blocks, import_results, invalid_blocks, imported, duration_ns, is_empty)
        };

        {
            if !imported_blocks.is_empty() && is_empty {
                let (enacted, retracted) = self.calculate_enacted_retracted(&import_results);

                if is_empty {
                    self.miner.chain_new_blocks(client, &imported_blocks, &invalid_blocks, &enacted, &retracted);
                }

                client.notify(|notify| {
                    notify.new_blocks(
                        imported_blocks.clone(),
                        invalid_blocks.clone(),
                        enacted.clone(),
                        retracted.clone(),
                        Vec::new(),
                        duration,
                    );
                });
            }
        }

        client.db.read().flush().expect("DB flush failed.");
        imported
    }

    fn calculate_enacted_retracted(&self, import_results: &[ImportRoute]) -> (Vec<H256>, Vec<H256>) {
        fn map_to_vec(map: Vec<(H256, bool)>) -> Vec<H256> {
            map.into_iter().map(|(k, _v)| k).collect()
        }

        // In ImportRoute we get all the blocks that have been enacted and retracted by single insert.
        // Because we are doing multiple inserts some of the blocks that were enacted in import `k`
        // could be retracted in import `k+1`. This is why to understand if after all inserts
        // the block is enacted or retracted we iterate over all routes and at the end final state
        // will be in the hashmap
        let map = import_results.iter().fold(HashMap::new(), |mut map, route| {
            for hash in &route.enacted {
                map.insert(hash.clone(), true);
            }
            for hash in &route.retracted {
                map.insert(hash.clone(), false);
            }
            map
        });

        // Split to enacted retracted (using hashmap value)
        let (enacted, retracted) = map.into_iter().partition(|&(_k, v)| v);
        // And convert tuples to keys
        (map_to_vec(enacted), map_to_vec(retracted))
    }

    // NOTE: the header of the block passed here is not necessarily sealed, as
    // it is for reconstructing the state transition.
    //
    // The header passed is from the original block data and is sealed.
    fn commit_block<B>(&self, block: B, header: &Header, block_data: &[u8], client: &Client) -> ImportRoute
    where
        B: IsBlock + Drain, {
        let hash = &header.hash();
        let number = header.number();
        let chain = client.chain.read();

        // Commit results
        let invoices = block.invoices().to_owned();

        assert_eq!(header.hash(), BlockView::new(block_data).header_view().hash());

        let mut batch = DBTransaction::new();

        // CHECK! I *think* this is fine, even if the state_root is equal to another
        // already-imported block of the same number.
        // TODO: Prove it with a test.
        let mut state = block.drain();

        // check epoch end signal
        self.check_epoch_end_signal(&header, &chain, &mut batch);

        state.journal_under(&mut batch, number, hash).expect("DB commit failed");
        let route = chain.insert_block(&mut batch, block_data, invoices.clone());

        let is_canon = route.enacted.last().map_or(false, |h| h == hash);
        state.sync_cache(&route.enacted, &route.retracted, is_canon);
        // Final commit to the DB
        client.db.read().write_buffered(batch);
        chain.commit();

        self.check_epoch_end(&header, &chain, client);

        route
    }

    // check for ending of epoch and write transition if it occurs.
    fn check_epoch_end<'a>(&self, header: &'a Header, chain: &BlockChain, client: &Client) {
        let is_epoch_end = self.engine.is_epoch_end(
            header,
            &(|hash| chain.block_header(&hash)),
            &(|hash| chain.get_pending_transition(hash)), // TODO: limit to current epoch.
        );

        if let Some(proof) = is_epoch_end {
            debug!(target: "client", "Epoch transition at block {}", header.hash());

            let mut batch = DBTransaction::new();
            chain.insert_epoch_transition(
                &mut batch,
                header.number(),
                EpochTransition {
                    block_hash: header.hash(),
                    block_number: header.number(),
                    proof,
                },
            );

            // always write the batch directly since epoch transition proofs are
            // fetched from a DB iterator and DB iterators are only available on
            // flushed data.
            client.db.read().write(batch).expect("DB flush failed");
        }
    }

    // check for epoch end signal and write pending transition if it occurs.
    // state for the given block must be available.
    fn check_epoch_end_signal(&self, header: &Header, chain: &BlockChain, batch: &mut DBTransaction) {
        use super::super::consensus::EpochChange;
        let hash = header.hash();

        match self.engine.signals_epoch_end(header) {
            EpochChange::Yes(proof) => {
                use super::super::consensus::epoch::PendingTransition;
                use super::super::consensus::Proof;

                let Proof::Known(proof) = proof;
                debug!(target: "client", "Block {} signals epoch end.", hash);

                let pending = PendingTransition {
                    proof,
                };
                chain.insert_pending_transition(batch, hash, pending);
            }
            EpochChange::No => {}
            EpochChange::Unsure => {
                warn!(target: "client", "Detected invalid engine implementation.");
                warn!(target: "client", "Engine claims to require more block data, but everything provided.");
            }
        }
    }

    fn check_and_close_block(&self, block: &PreverifiedBlock, client: &Client) -> Result<LockedBlock, ()> {
        let engine = &*self.engine;
        let header = &block.header;

        let chain = client.chain.read();

        // Check if parent is in chain
        let parent = match chain.block_header(header.parent_hash()) {
            Some(h) => h,
            None => {
                warn!(target: "client", "Block import failed for #{} ({}): Parent not found ({}) ", header.number(), header.hash(), header.parent_hash());
                return Err(())
            }
        };

        // Verify Block Family
        let verify_family_result = self.verifier.verify_block_family(
            header,
            &parent,
            engine,
            Some(verification::FullFamilyParams {
                block_bytes: &block.bytes,
                parcels: &block.parcels,
                block_provider: &**chain,
                client,
            }),
        );

        if let Err(e) = verify_family_result {
            warn!(target: "client", "Stage 3 block verification failed for #{} ({})\nError: {:?}", header.number(), header.hash(), e);
            return Err(())
        };

        let verify_external_result = self.verifier.verify_block_external(header, engine);
        if let Err(e) = verify_external_result {
            warn!(target: "client", "Stage 4 block verification failed for #{} ({})\nError: {:?}", header.number(), header.hash(), e);
            return Err(())
        };

        // Enact Verified Block
        let db = client.state_db.read().boxed_clone_canon(header.parent_hash());

        let is_epoch_begin = chain.epoch_transition(parent.number(), *header.parent_hash()).is_some();
        let enact_result =
            enact(&block.header, &block.parcels, engine, db, &parent, client.trie_factory.clone(), is_epoch_begin);
        let locked_block = enact_result.map_err(|e| {
            warn!(target: "client", "Block import failed for #{} ({})\nError: {:?}", header.number(), header.hash(), e);
        })?;

        // Final Verification
        if let Err(e) = self.verifier.verify_block_final(header, locked_block.block().header()) {
            warn!(target: "client", "Stage 5 block verification failed for #{} ({})\nError: {:?}", header.number(), header.hash(), e);
            return Err(())
        }

        Ok(locked_block)
    }
}

impl Importer {
    /// This is triggered by a message coming from a header queue when the header is ready for insertion
    pub fn import_verified_headers(&self, client: &Client) -> usize {
        let max_headers_to_import = 256;

        let _lock = self.import_lock.lock();

        let mut bad = HashSet::new();
        let mut imported = Vec::new();
        for header in self.header_queue.drain(max_headers_to_import) {
            let hash = header.hash();
            trace!(target: "client", "importing header {}", header.number());

            if bad.contains(&hash) || bad.contains(header.parent_hash()) {
                trace!(target: "client", "Bad header detected : {}", hash);
                bad.insert(hash);
                continue
            }

            let parent_header = client
                .block_header(BlockId::Hash(*header.parent_hash()))
                .expect("Parent of importing header must exist")
                .decode();
            if self.check_header(&header, &parent_header) {
                if self.engine.is_proposal(&header) {
                    self.header_queue.mark_as_good(&[hash]);
                } else {
                    imported.push(hash);
                    self.commit_header(&header, client);
                }
            } else {
                bad.insert(hash);
            }
        }

        self.header_queue.mark_as_bad(&bad.drain().collect::<Vec<_>>());

        // FIXME: notify new headers
        client.db.read().flush().expect("DB flush failed.");

        imported.len()
    }

    fn check_header(&self, header: &Header, parent: &Header) -> bool {
        // FIXME: self.verifier.verify_block_family
        if let Err(e) = self.engine.verify_block_family(&header, &parent) {
            warn!(target: "client", "Stage 3 block verification failed for #{} ({})\nError: {:?}",
            header.number(), header.hash(), e);
            return false
        };

        // "external" verification.
        if let Err(e) = self.engine.verify_block_external(&header) {
            warn!(target: "client", "Stage 4 block verification failed for #{} ({})\nError: {:?}",
            header.number(), header.hash(), e);
            return false
        };

        true
    }

    fn commit_header(&self, header: &Header, client: &Client) {
        let chain = client.chain.read();

        let mut batch = DBTransaction::new();
        // FIXME: Check if this line is still necessary.
        // self.check_epoch_end_signal(header, &chain, &mut batch);
        chain.insert_header(&mut batch, &HeaderView::new(&header.rlp_bytes()));
        client.db.read().write_buffered(batch);
        chain.commit();

        // FIXME: Check if this line is still necessary.
        // self.check_epoch_end(&header, &chain, client);
    }
}

impl AccountData for Client {}

impl Nonce for Client {
    fn nonce(&self, address: &Address, id: BlockId) -> Option<U256> {
        self.state_at(id).and_then(|s| s.nonce(address).ok())
    }
}

impl Balance for Client {
    fn balance(&self, address: &Address, state: StateOrBlock) -> Option<U256> {
        match state {
            StateOrBlock::State(s) => s.balance(address).ok(),
            StateOrBlock::Block(id) => self.state_at(id).and_then(|s| s.balance(address).ok()),
        }
    }
}

impl ReopenBlock for Client {
    fn reopen_block(&self, block: ClosedBlock) -> OpenBlock {
        let engine = &*self.engine;
        block.reopen(engine)
    }
}

impl PrepareOpenBlock for Client {
    fn prepare_open_block(&self, author: Address, extra_data: Bytes) -> OpenBlock {
        let engine = &*self.engine;
        let chain = self.chain.read();
        let h = chain.best_block_hash();
        let best_header = &chain.block_header(&h).expect("h is best block hash: so its header must exist: qed");

        let is_epoch_begin = chain.epoch_transition(best_header.number(), h).is_some();
        OpenBlock::new(
            engine,
            self.trie_factory.clone(),
            self.state_db.read().boxed_clone_canon(&h),
            best_header,
            author,
            extra_data,
            is_epoch_begin,
        ).expect("OpenBlock::new only fails if parent state root invalid; state root of best block's header is never invalid; qed")
    }
}

impl BlockProducer for Client {}

impl ImportSealedBlock for Client {
    fn import_sealed_block(&self, block: SealedBlock) -> ImportResult {
        let h = block.header().hash();
        let start = Instant::now();
        let route = {
            // scope for self.import_lock
            let _import_lock = self.importer.import_lock.lock();

            let number = block.header().number();
            let block_data = block.rlp_bytes();
            let header = block.header().clone();

            let route = self.importer.commit_block(block, &header, &block_data, self);
            trace!(target: "client", "Imported sealed block #{} ({})", number, h);
            self.state_db.write().sync_cache(&route.enacted, &route.retracted, false);
            route
        };
        let (enacted, retracted) = self.importer.calculate_enacted_retracted(&[route]);
        self.importer.miner.chain_new_blocks(self, &[h.clone()], &[], &enacted, &retracted);
        self.notify(|notify| {
            notify.new_blocks(vec![h.clone()], vec![], enacted.clone(), retracted.clone(), vec![h.clone()], {
                let elapsed = start.elapsed();
                elapsed.as_secs() * 1_000_000_000 + elapsed.subsec_nanos() as u64
            });
        });
        self.db.read().flush().expect("DB flush failed.");
        Ok(h)
    }
}

impl MiningBlockChainClient for Client {}
