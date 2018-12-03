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

use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};
use std::time::Instant;

use cio::IoChannel;
use ckey::{Address, PlatformAddress, Public};
use cmerkle::Result as TrieResult;
use cnetwork::NodeId;
use cstate::{
    ActionHandler, AssetScheme, AssetSchemeAddress, OwnedAsset, OwnedAssetAddress, StateDB, TopLevelState, TopStateView,
};
use ctypes::invoice::Invoice;
use ctypes::transaction::Transaction;
use ctypes::{BlockNumber, ShardId};
use cvm::ChainTimeInfo;
use hashdb::AsHashDB;
use journaldb;
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::{Mutex, RwLock, RwLockReadGuard};
use primitives::{Bytes, H256, U256};
use rlp::UntrustedRlp;

use super::importer::Importer;
use super::{
    AccountData, AssetClient, Balance, BlockChain as BlockChainTrait, BlockChainClient, BlockChainInfo, BlockInfo,
    BlockProducer, ChainInfo, ChainNotify, ClientConfig, DatabaseClient, EngineClient, EngineInfo,
    Error as ClientError, ExecuteClient, ImportBlock, ImportResult, ImportSealedBlock, MiningBlockChainClient,
    ParcelInfo, PrepareOpenBlock, RegularKey, RegularKeyOwner, ReopenBlock, Seq, Shard, StateOrBlock, TransactionInfo,
};
use crate::block::{ClosedBlock, IsBlock, OpenBlock, SealedBlock};
use crate::blockchain::{
    BlockChain, BlockProvider, BodyProvider, HeaderProvider, InvoiceProvider, ParcelAddress, TransactionAddress,
};
use crate::consensus::CodeChainEngine;
use crate::encoded;
use crate::error::{BlockImportError, Error, ImportError, SchemeError};
use crate::miner::{Miner, MinerService};
use crate::parcel::{LocalizedParcel, SignedParcel, UnverifiedParcel};
use crate::scheme::{CommonParams, Scheme};
use crate::service::ClientIoMessage;
use crate::types::{BlockId, BlockStatus, ParcelId, VerificationQueueInfo as BlockQueueInfo};

const MAX_MEM_POOL_SIZE: usize = 4096;

pub struct Client {
    engine: Arc<CodeChainEngine>,

    io_channel: Mutex<IoChannel<ClientIoMessage>>,

    chain: RwLock<BlockChain>,

    /// Client uses this to store blocks, traces, etc.
    db: RwLock<Arc<KeyValueDB>>,

    state_db: RwLock<StateDB>,

    /// List of actors to be notified on certain chain events
    notify: RwLock<Vec<Weak<ChainNotify>>>,

    /// Count of pending parcels in the queue
    queue_parcels: AtomicUsize,

    genesis_accounts: Vec<Address>,

    importer: Importer,
}

impl Client {
    pub fn try_new(
        config: &ClientConfig,
        scheme: &Scheme,
        db: Arc<KeyValueDB>,
        miner: Arc<Miner>,
        message_channel: IoChannel<ClientIoMessage>,
    ) -> Result<Arc<Client>, Error> {
        let journal_db = journaldb::new(Arc::clone(&db), journaldb::Algorithm::Archive, ::db::COL_STATE);
        let mut state_db = StateDB::new(journal_db, scheme.custom_handlers.clone());
        if !scheme.check_genesis_root(state_db.as_hashdb()) {
            return Err(SchemeError::InvalidState.into())
        }
        if state_db.is_empty() {
            // Sets the correct state root.
            state_db = scheme.ensure_genesis_state(state_db)?;
            let mut batch = DBTransaction::new();
            state_db.journal_under(&mut batch, 0, scheme.genesis_header().hash())?;
            db.write(batch).map_err(ClientError::Database)?;
        }

        let gb = scheme.genesis_block();
        let chain = BlockChain::new(&gb, db.clone());
        scheme.check_genesis_common_params(&chain)?;

        let engine = scheme.engine.clone();

        let importer = Importer::try_new(config, engine.clone(), message_channel.clone(), miner)?;
        let genesis_accounts = scheme.genesis_accounts();

        let client = Arc::new(Client {
            engine,
            io_channel: Mutex::new(message_channel),
            chain: RwLock::new(chain),
            db: RwLock::new(db),
            state_db: RwLock::new(state_db),
            notify: RwLock::new(Vec::new()),
            queue_parcels: AtomicUsize::new(0),
            genesis_accounts,
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
    pub fn add_notify(&self, target: Weak<ChainNotify>) {
        self.notify.write().push(target);
    }

    pub fn notify<F>(&self, f: F)
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

    fn block_hash(chain: &BlockChain, id: &BlockId) -> Option<H256> {
        match id {
            BlockId::Hash(hash) => Some(*hash),
            BlockId::Number(number) => chain.block_hash(*number),
            BlockId::Earliest => chain.block_hash(0),
            BlockId::Latest => Some(chain.best_block_hash()),
        }
    }

    fn parcel_address(&self, id: &ParcelId) -> Option<ParcelAddress> {
        match id {
            ParcelId::Hash(hash) => self.block_chain().parcel_address(hash),
            ParcelId::Location(id, index) => Self::block_hash(&self.block_chain(), id).map(|hash| ParcelAddress {
                block_hash: hash,
                index: *index,
            }),
        }
    }

    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        self.block_chain().transaction_address(hash)
    }

    fn parcel_address_of_successful_transaction(&self, hash: &H256) -> Option<ParcelAddress> {
        self.transaction_address(hash).and_then(|transaction_address| {
            transaction_address
                .into_iter()
                .filter(|parcel_address| {
                    self.parcel_invoice(&(*parcel_address).into()).map_or(false, |invoice| invoice == Invoice::Success)
                })
                .take(1)
                .next()
        })
    }

    /// Import parcels from the IO queue
    pub fn import_queued_parcels(&self, parcels: &[Bytes], peer_id: NodeId) -> usize {
        ctrace!(EXTERNAL_PARCEL, "Importing queued");
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
            BlockId::Number(number) => Some(*number),
            BlockId::Hash(hash) => self.block_chain().block_number(hash),
            BlockId::Earliest => Some(0),
            BlockId::Latest => Some(self.block_chain().best_block_detail().number),
        }
    }

    /// Get a copy of the best block's state.
    fn latest_state(&self) -> TopLevelState {
        let header = self.best_block_header();
        TopLevelState::from_existing(self.state_db.read().clone(&header.state_root()), header.state_root())
            .expect("State root of best block header always valid.")
    }

    /// Attempt to get a copy of a specific block's final state.
    ///
    /// This will not fail if given BlockId::Latest.
    /// Otherwise, this can fail (but may not) if the DB prunes state or the block
    /// is unknown.
    fn state_at(&self, id: BlockId) -> Option<TopLevelState> {
        // fast path for latest state.
        if BlockId::Latest == id {
            return Some(self.latest_state())
        }

        self.block_header(&id).and_then(|header| {
            let root = header.state_root();
            TopLevelState::from_existing(self.state_db.read().clone(&root), root).ok()
        })
    }

    fn state_info(&self, state: StateOrBlock) -> Option<Box<TopStateView>> {
        Some(match state {
            StateOrBlock::State(state) => state,
            StateOrBlock::Block(id) => Box::new(self.state_at(id)?),
        })
    }

    pub fn state_db(&self) -> &RwLock<StateDB> {
        &self.state_db
    }

    pub fn block_chain(&self) -> RwLockReadGuard<BlockChain> {
        self.chain.read()
    }

    pub fn db(&self) -> RwLockReadGuard<Arc<KeyValueDB>> {
        self.db.read()
    }
}

impl DatabaseClient for Client {
    fn database(&self) -> Arc<KeyValueDB> {
        Arc::clone(&self.db())
    }
}

impl AssetClient for Client {
    fn get_asset_scheme(&self, asset_type: AssetSchemeAddress, id: BlockId) -> TrieResult<Option<AssetScheme>> {
        if let Some(state) = Client::state_at(&self, id) {
            let shard_id = asset_type.shard_id();
            Ok(state.asset_scheme(shard_id, &asset_type)?)
        } else {
            Ok(None)
        }
    }

    fn get_asset(&self, transaction_hash: H256, index: usize, id: BlockId) -> TrieResult<Option<OwnedAsset>> {
        if let Some(state) = Client::state_at(&self, id) {
            let shard_id = 0; // FIXME
            let address = OwnedAssetAddress::new(transaction_hash, index, shard_id);
            Ok(state.asset(shard_id, &address)?)
        } else {
            Ok(None)
        }
    }

    /// Checks whether an asset is spent or not.
    ///
    /// It returns None if such an asset never existed in the shard at the given block.
    fn is_asset_spent(
        &self,
        transaction_hash: H256,
        index: usize,
        shard_id: ShardId,
        block_id: BlockId,
    ) -> TrieResult<Option<bool>> {
        let parcel_address = match self.parcel_address_of_successful_transaction(&transaction_hash) {
            Some(parcel_address) => parcel_address,
            None => return Ok(None),
        };

        match self.block_number(&block_id) {
            None => return Ok(None),
            Some(block_number)
                if block_number < self
                    .block_number(&parcel_address.block_hash.into())
                    .expect("There is a successful transaction") =>
            {
                return Ok(None)
            }
            Some(_) => {}
        }

        let transaction = self.transaction(&transaction_hash).expect("There is a successful transaction");
        if !transaction.is_valid_shard_id_index(index, shard_id) {
            return Ok(None)
        }

        let state = Client::state_at(&self, block_id).unwrap();
        let address = OwnedAssetAddress::new(transaction_hash, index, shard_id);
        Ok(Some(state.asset(shard_id, &address)?.is_none()))
    }
}

impl ExecuteClient for Client {
    fn execute_transaction(&self, transaction: &Transaction, sender: &Address) -> Result<Invoice, Error> {
        let mut state = Client::state_at(&self, BlockId::Latest).expect("Latest state MUST exist");
        Ok(state.apply_transaction(transaction, sender, self)?)
    }
}

impl ChainInfo for Client {
    fn chain_info(&self) -> BlockChainInfo {
        let mut chain_info = self.block_chain().chain_info();
        chain_info.pending_total_score = chain_info.best_score + self.importer.block_queue.total_score();
        chain_info
    }

    fn genesis_accounts(&self) -> Vec<PlatformAddress> {
        let network_id = self.common_params().network_id;
        self.genesis_accounts.iter().map(|addr| PlatformAddress::new_v1(network_id, *addr)).collect()
    }
}

impl EngineInfo for Client {
    fn common_params(&self) -> &CommonParams {
        self.engine().params()
    }

    fn block_reward(&self, block_number: u64) -> u64 {
        self.engine().block_reward(block_number)
    }

    fn mining_reward(&self, block_number: u64) -> Option<u64> {
        let block = self.block(&block_number.into())?;
        let block_fee = self.engine().block_fee(Box::new(block.parcels().into_iter()));
        Some(self.engine().block_reward(block_number) + block_fee)
    }

    fn recommended_confirmation(&self) -> u32 {
        self.engine().recommended_confirmation()
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
            cwarn!(CLIENT, "Wrong internal seal submission!")
        }
    }

    /// Convert PoW difficulty to target.
    fn score_to_target(&self, score: &U256) -> U256 {
        self.engine.score_to_target(score)
    }
}

impl BlockInfo for Client {
    fn block_header(&self, id: &BlockId) -> Option<::encoded::Header> {
        let chain = self.block_chain();

        Self::block_hash(&chain, id).and_then(|hash| chain.block_header_data(&hash))
    }

    fn best_block_header(&self) -> encoded::Header {
        self.block_chain().best_block_header()
    }

    fn best_header(&self) -> encoded::Header {
        self.block_chain().best_header()
    }

    fn block(&self, id: &BlockId) -> Option<encoded::Block> {
        let chain = self.block_chain();

        Self::block_hash(&chain, id).and_then(|hash| chain.block(&hash))
    }
}

impl ParcelInfo for Client {
    fn parcel_block(&self, id: &ParcelId) -> Option<H256> {
        self.parcel_address(id).map(|addr| addr.block_hash)
    }
}

impl TransactionInfo for Client {
    fn transaction_header(&self, hash: &H256) -> Option<::encoded::Header> {
        self.transaction_address(hash)
            .and_then(|addr| {
                addr.into_iter()
                    .find(|addr| {
                        let invoice = self.parcel_invoice(&ParcelId::from(*addr)).expect("Parcel must exist");
                        invoice == Invoice::Success
                    })
                    .map(|hash| hash.block_hash)
            })
            .and_then(|hash| self.block_header(&hash.into()))
    }
}

impl ImportBlock for Client {
    fn import_block(&self, bytes: Bytes) -> Result<H256, BlockImportError> {
        use crate::verification::queue::kind::blocks::Unverified;
        use crate::verification::queue::kind::BlockLike;

        let unverified = Unverified::new(bytes);
        {
            if self.block_chain().is_known(&unverified.hash()) {
                return Err(BlockImportError::Import(ImportError::AlreadyInChain))
            }
        }
        Ok(self.importer.block_queue.import(unverified)?)
    }

    fn import_header(&self, bytes: Bytes) -> Result<H256, BlockImportError> {
        let unverified = ::encoded::Header::new(bytes).decode();
        {
            if self.block_chain().is_known_header(&unverified.hash()) {
                return Err(BlockImportError::Import(ImportError::AlreadyInChain))
            }
        }
        Ok(self.importer.header_queue.import(unverified)?)
    }
}

impl BlockChainTrait for Client {}

impl BlockChainClient for Client {
    fn queue_info(&self) -> BlockQueueInfo {
        self.importer.block_queue.queue_info()
    }

    fn queue_parcels(&self, parcels: Vec<Bytes>, peer_id: NodeId) {
        let queue_size = self.queue_parcels.load(AtomicOrdering::Relaxed);
        ctrace!(EXTERNAL_PARCEL, "Queue size: {}", queue_size);
        if queue_size > MAX_MEM_POOL_SIZE {
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

    fn block_number(&self, id: &BlockId) -> Option<BlockNumber> {
        self.block_number_ref(&id)
    }

    fn block_body(&self, id: &BlockId) -> Option<encoded::Body> {
        let chain = self.block_chain();

        Self::block_hash(&chain, id).and_then(|hash| chain.block_body(&hash))
    }

    fn block_status(&self, id: &BlockId) -> BlockStatus {
        let chain = self.block_chain();
        match Self::block_hash(&chain, id) {
            Some(ref hash) if chain.is_known(hash) => BlockStatus::InChain,
            Some(hash) => self.importer.block_queue.status(&hash),
            None => BlockStatus::Unknown,
        }
    }

    fn block_total_score(&self, id: &BlockId) -> Option<U256> {
        let chain = self.block_chain();

        Self::block_hash(&chain, id).and_then(|hash| chain.block_details(&hash)).map(|d| d.total_score)
    }

    fn block_hash(&self, id: &BlockId) -> Option<H256> {
        let chain = self.block_chain();
        Self::block_hash(&chain, id)
    }

    fn parcel(&self, id: &ParcelId) -> Option<LocalizedParcel> {
        let chain = self.block_chain();
        self.parcel_address(id).and_then(|address| chain.parcel(&address))
    }

    fn parcel_invoice(&self, id: &ParcelId) -> Option<Invoice> {
        let chain = self.block_chain();
        self.parcel_address(id).and_then(|address| chain.parcel_invoice(&address))
    }

    fn transaction(&self, hash: &H256) -> Option<Transaction> {
        let chain = self.block_chain();
        self.transaction_address(hash).and_then(|address| chain.transaction(&address))
    }

    fn transaction_invoices(&self, hash: &H256) -> Vec<Invoice> {
        self.transaction_address(hash)
            .map(|address| {
                address
                    .into_iter()
                    .map(Into::into)
                    .map(|address| self.parcel_invoice(&address).expect("The invoice must exist"))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn custom_handlers(&self) -> Vec<Arc<ActionHandler>> {
        self.state_db.read().custom_handlers().to_vec()
    }
}

impl AccountData for Client {}

impl Seq for Client {
    fn seq(&self, address: &Address, id: BlockId) -> Option<u64> {
        self.state_at(id).and_then(|s| s.seq(address).ok())
    }
}

impl Balance for Client {
    fn balance(&self, address: &Address, state: StateOrBlock) -> Option<u64> {
        let state = self.state_info(state)?;
        state.balance(address).ok()
    }
}

impl RegularKey for Client {
    fn regular_key(&self, address: &Address, state: StateOrBlock) -> Option<Public> {
        let state = self.state_info(state)?;
        state.regular_key(address).ok()?
    }
}

impl RegularKeyOwner for Client {
    fn regular_key_owner(&self, address: &Address, state: StateOrBlock) -> Option<Address> {
        let state = self.state_info(state)?;
        state.regular_key_owner(address).ok()?
    }
}

impl Shard for Client {
    fn number_of_shards(&self, state: StateOrBlock) -> Option<ShardId> {
        let state = self.state_info(state)?;
        state.number_of_shards().ok()
    }

    fn shard_root(&self, shard_id: ShardId, state: StateOrBlock) -> Option<H256> {
        let state = self.state_info(state)?;
        state.shard_root(shard_id).ok()?
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
        let chain = self.block_chain();
        let h = engine.get_latest_block_hash(chain.best_block_hash());
        let latest_header = &chain.block_header(&h).expect("h is best block hash: so its header must exist: qed");

        let is_epoch_begin = chain.epoch_transition(latest_header.number(), h).is_some();
        OpenBlock::try_new(
            engine,
            self.state_db.read().clone(&latest_header.state_root()),
            latest_header,
            author,
            extra_data,
            is_epoch_begin,
        ).expect("OpenBlock::new only fails if parent state root invalid; state root of best block's header is never invalid; qed")
    }
}

impl BlockProducer for Client {}

impl ImportSealedBlock for Client {
    fn import_sealed_block(&self, block: &SealedBlock) -> ImportResult {
        let h = block.header().hash();
        let start = Instant::now();
        let route = {
            // scope for self.import_lock
            let _import_lock = self.importer.import_lock.lock();

            let number = block.header().number();
            let block_data = block.rlp_bytes();
            let header = block.header().clone();

            let route = self.importer.commit_block(block, &header, &block_data, self);
            ctrace!(CLIENT, "Imported sealed block #{} ({})", number, h);
            route
        };
        let (enacted, retracted) = self.importer.calculate_enacted_retracted(&[route]);
        self.importer.miner.chain_new_blocks(self, &[h], &[], &enacted, &retracted);
        self.notify(|notify| {
            notify.new_blocks(vec![h], vec![], enacted.clone(), retracted.clone(), vec![h], {
                let elapsed = start.elapsed();
                elapsed.as_secs() * 1_000_000_000 + u64::from(elapsed.subsec_nanos())
            });
        });
        self.db().flush().expect("DB flush failed.");
        Ok(h)
    }
}

impl MiningBlockChainClient for Client {}

impl ChainTimeInfo for Client {
    fn best_block_number(&self) -> BlockNumber {
        self.chain_info().best_block_number
    }

    fn best_block_timestamp(&self) -> u64 {
        self.chain_info().best_block_timestamp
    }

    fn transaction_block_age(&self, hash: &H256) -> Option<u64> {
        self.transaction_block_number(hash).map(|block_number| self.chain_info().best_block_number - block_number)
    }

    fn transaction_time_age(&self, hash: &H256) -> Option<u64> {
        self.transaction_block_timestamp(hash)
            .map(|block_timestamp| self.chain_info().best_block_timestamp - block_timestamp)
    }
}
