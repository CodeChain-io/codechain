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
    ActionHandler, AssetScheme, AssetSchemeAddress, FindActionHandler, OwnedAsset, OwnedAssetAddress, StateDB, Text,
    TopLevelState, TopStateView,
};
use ctimer::{TimeoutHandler, TimerApi, TimerScheduleError, TimerToken};
use ctypes::invoice::Invoice;
use ctypes::transaction::{AssetTransferInput, PartialHashing, ShardTransaction};
use ctypes::{BlockNumber, ShardId};
use cvm::{decode, execute, ChainTimeInfo, ScriptResult, VMConfig};
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
    ParcelInfo, PrepareOpenBlock, RegularKey, RegularKeyOwner, ReopenBlock, ResealTimer, Seq, Shard, StateInfo,
    StateOrBlock, TextClient, TransactionInfo,
};
use crate::block::{ClosedBlock, IsBlock, OpenBlock, SealedBlock};
use crate::blockchain::{
    BlockChain, BlockProvider, BodyProvider, HeaderProvider, InvoiceProvider, ParcelAddress, TransactionAddress,
};
use crate::consensus::CodeChainEngine;
use crate::encoded;
use crate::error::{BlockImportError, Error, ImportError, SchemeError};
use crate::miner::{Miner, MinerService};
use crate::scheme::{CommonParams, Scheme};
use crate::service::ClientIoMessage;
use crate::transaction::{LocalizedTransaction, SignedTransaction, UnverifiedTransaction};
use crate::types::{BlockId, BlockStatus, TransactionId, VerificationQueueInfo as BlockQueueInfo};

const MAX_MEM_POOL_SIZE: usize = 4096;

pub struct Client {
    engine: Arc<CodeChainEngine>,

    io_channel: Mutex<IoChannel<ClientIoMessage>>,

    chain: RwLock<BlockChain>,

    /// Client uses this to store blocks, traces, etc.
    db: Arc<KeyValueDB>,

    state_db: RwLock<StateDB>,

    /// List of actors to be notified on certain chain events
    notify: RwLock<Vec<Weak<ChainNotify>>>,

    /// Count of pending parcels in the queue
    queue_transactions: AtomicUsize,

    genesis_accounts: Vec<Address>,

    importer: Importer,

    /// Timer for reseal_min_period/reseal_max_period on miner client
    reseal_timer: TimerApi,
}

impl Client {
    pub fn try_new(
        config: &ClientConfig,
        scheme: &Scheme,
        db: Arc<KeyValueDB>,
        miner: Arc<Miner>,
        message_channel: IoChannel<ClientIoMessage>,
        reseal_timer: TimerApi,
    ) -> Result<Arc<Client>, Error> {
        let journal_db = journaldb::new(Arc::clone(&db), journaldb::Algorithm::Archive, ::db::COL_STATE);
        let mut state_db = StateDB::new(journal_db);
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
            db,
            state_db: RwLock::new(state_db),
            notify: RwLock::new(Vec::new()),
            queue_transactions: AtomicUsize::new(0),
            genesis_accounts,
            importer,
            reseal_timer,
        });

        // ensure buffered changes are flushed.
        client.db.flush().map_err(ClientError::Database)?;
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

    /// This is triggered by a message coming from a engine when a new block should be created
    pub fn update_sealing(&self, parent_block: BlockId, allow_empty_block: bool) {
        self.importer.miner.update_sealing(self, parent_block, allow_empty_block);
    }

    fn block_hash(chain: &BlockChain, id: &BlockId) -> Option<H256> {
        match id {
            BlockId::Hash(hash) => Some(*hash),
            BlockId::Number(number) => chain.block_hash(*number),
            BlockId::Earliest => chain.block_hash(0),
            BlockId::Latest => Some(chain.best_block_hash()),
        }
    }

    fn parcel_address(&self, id: &TransactionId) -> Option<ParcelAddress> {
        match id {
            TransactionId::Hash(hash) => self.block_chain().parcel_address(hash),
            TransactionId::Location(id, index) => Self::block_hash(&self.block_chain(), id).map(|hash| ParcelAddress {
                block_hash: hash,
                index: *index,
            }),
        }
    }

    fn transaction_address(&self, tracker: &H256) -> Option<TransactionAddress> {
        self.block_chain().transaction_address(tracker)
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
        self.queue_transactions.fetch_sub(parcels.len(), AtomicOrdering::SeqCst);
        let parcels: Vec<UnverifiedTransaction> =
            parcels.iter().filter_map(|bytes| UntrustedRlp::new(bytes).as_val().ok()).collect();
        let hashes: Vec<_> = parcels.iter().map(|parcel| parcel.hash()).collect();
        self.notify(|notify| {
            notify.transactions_received(hashes.clone(), peer_id);
        });
        let results = self.importer.miner.import_external_tranasctions(self, parcels);
        results.len()
    }

    /// This is triggered by a message coming from the Tendermint engine when a block is committed.
    /// See EngineClient::update_best_as_committed() for details.
    pub fn update_best_as_committed(&self, block_hash: H256) {
        ctrace!(CLIENT, "Update the best block to the hash({}), as requested", block_hash);
        let start = Instant::now();
        let route = {
            let _import_lock = self.importer.import_lock.lock();

            let chain = self.block_chain();
            let mut batch = DBTransaction::new();

            let route = chain.update_best_as_committed(&mut batch, block_hash);
            self.db().write(batch).expect("DB flush failed.");
            chain.commit();

            // Clear the state DB cache
            let mut state_db = self.state_db().write();
            state_db.clear_cache();

            route
        };

        if route.is_none() {
            return
        }

        let (enacted, retracted) = self.importer.calculate_enacted_retracted(&[route]);
        self.importer.miner.chain_new_blocks(self, &[], &[], &enacted, &retracted);
        self.notify(|notify| {
            notify.new_blocks(vec![], vec![], enacted.clone(), retracted.clone(), vec![], {
                let elapsed = start.elapsed();
                elapsed.as_secs() * 1_000_000_000 + u64::from(elapsed.subsec_nanos())
            });
        });
    }

    fn block_number_ref(&self, id: &BlockId) -> Option<BlockNumber> {
        match id {
            BlockId::Number(number) => Some(*number),
            BlockId::Hash(hash) => self.block_chain().block_number(hash),
            BlockId::Earliest => Some(0),
            BlockId::Latest => Some(self.block_chain().best_block_detail().number),
        }
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

    pub fn db(&self) -> &Arc<KeyValueDB> {
        &self.db
    }
}

/// When RESEAL_MAX_TIMER invoked, a block is created although the block is empty.
const RESEAL_MAX_TIMER_TOKEN: TimerToken = 0;
/// The minimum time between blocks, the miner creates a block when RESEAL_MIN_TIMER is invoked.
/// Do not create a block before RESEAL_MIN_TIMER event.
const RESEAL_MIN_TIMER_TOKEN: TimerToken = 1;

impl TimeoutHandler for Client {
    fn on_timeout(&self, token: TimerToken) {
        match token {
            RESEAL_MAX_TIMER_TOKEN => {
                // Working in PoW only
                if self.engine().seals_internally().is_none() && !self.importer.miner.prepare_work_sealing(self) {
                    self.update_sealing(BlockId::Latest, true);
                }
            }
            RESEAL_MIN_TIMER_TOKEN => {
                // Checking self.ready_transactions() for efficiency
                if !self.engine().engine_type().ignore_reseal_min_period() && !self.ready_transactions().is_empty() {
                    self.update_sealing(BlockId::Latest, false);
                }
            }
            _ => unreachable!(),
        }
    }
}

impl ResealTimer for Client {
    fn set_max_timer(&self) {
        self.reseal_timer.cancel(RESEAL_MAX_TIMER_TOKEN).expect("Reseal max timer clear succeeds");
        match self
            .reseal_timer
            .schedule_once(self.importer.miner.get_options().reseal_max_period, RESEAL_MAX_TIMER_TOKEN)
        {
            Ok(_) => {}
            Err(TimerScheduleError::TokenAlreadyScheduled) => {
                // Since set_max_timer could be called in multi thread, ignore the TokenAlreadyScheduled error
            }
            Err(err) => unreachable!("Reseal max timer should not fail but failed with {:?}", err),
        }
    }

    fn set_min_timer(&self) {
        self.reseal_timer.cancel(RESEAL_MIN_TIMER_TOKEN).expect("Reseal min timer clear succeeds");
        match self
            .reseal_timer
            .schedule_once(self.importer.miner.get_options().reseal_min_period, RESEAL_MIN_TIMER_TOKEN)
        {
            Ok(_) => {}
            Err(TimerScheduleError::TokenAlreadyScheduled) => {
                // Since set_min_timer could be called in multi thread, ignore the TokenAlreadyScheduled error
            }
            Err(err) => unreachable!("Reseal min timer should not fail but failed with {:?}", err),
        }
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
                if block_number
                    < self
                        .block_number(&parcel_address.block_hash.into())
                        .expect("There is a successful transaction") =>
            {
                return Ok(None)
            }
            Some(_) => {}
        }

        let parcel = self.transaction(&transaction_hash).expect("There is a successful transaction");
        let transaction = if let Some(tx) = Option::<ShardTransaction>::from(parcel.action.clone()) {
            tx
        } else {
            return Ok(None)
        };
        if !transaction.is_valid_shard_id_index(index, shard_id) {
            return Ok(None)
        }

        let state = Client::state_at(&self, block_id).unwrap();
        let address = OwnedAssetAddress::new(transaction_hash, index, shard_id);
        Ok(Some(state.asset(shard_id, &address)?.is_none()))
    }
}

impl TextClient for Client {
    fn get_text(&self, parcel_hash: H256, id: BlockId) -> TrieResult<Option<Text>> {
        if let Some(state) = Client::state_at(&self, id) {
            Ok(state.text(&parcel_hash)?)
        } else {
            Ok(None)
        }
    }
}

impl ExecuteClient for Client {
    fn execute_transaction(&self, transaction: &ShardTransaction, sender: &Address) -> Result<Invoice, Error> {
        let mut state = Client::state_at(&self, BlockId::Latest).expect("Latest state MUST exist");
        Ok(state.apply_shard_transaction(transaction, sender, &[], self)?)
    }

    fn execute_vm(
        &self,
        tx: &PartialHashing,
        inputs: &[AssetTransferInput],
        params: &[Vec<Bytes>],
        indices: &[usize],
    ) -> Result<Vec<String>, Error> {
        let mut results = vec![];
        for (i, index) in indices.iter().enumerate() {
            let input = &inputs[*index];
            let param = &params[i];
            let result = match (decode(&input.lock_script), decode(&input.unlock_script)) {
                (Ok(lock_script), Ok(unlock_script)) => {
                    let script_result =
                        execute(&unlock_script, &param, &lock_script, tx, VMConfig::default(), &input, false, self);
                    match script_result {
                        Ok(ScriptResult::Burnt) => "burnt",
                        Ok(ScriptResult::Unlocked) => "unlocked",
                        _ => "failed",
                    }
                }
                _ => "invalid",
            }
            .to_string();

            results.push(result);
        }
        Ok(results)
    }
}

impl StateInfo for Client {
    fn state_at(&self, id: BlockId) -> Option<TopLevelState> {
        self.block_header(&id).and_then(|header| {
            let root = header.state_root();
            TopLevelState::from_existing(self.state_db.read().clone(&root), root).ok()
        })
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
        let block_fee = self.engine().block_fee(Box::new(block.transactions().into_iter()));
        Some(self.engine().block_reward(block_number) + block_fee)
    }

    fn recommended_confirmation(&self) -> u32 {
        self.engine().recommended_confirmation()
    }
}

impl EngineClient for Client {
    /// Make a new block and seal it.
    fn update_sealing(&self, parent_block: BlockId, allow_empty_block: bool) {
        match self.io_channel.lock().send(ClientIoMessage::NewBlockRequired {
            parent_block,
            allow_empty_block,
        }) {
            Ok(_) => {}
            Err(e) => {
                cdebug!(CLIENT, "Error while triggering block creation: {}", e);
            }
        }
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

    /// Update the best block as the given block hash.
    ///
    /// Used in Tendermint, when going to the commit step.
    fn update_best_as_committed(&self, block_hash: H256) {
        ctrace!(ENGINE, "Requesting a best block update (block hash: {})", block_hash);
        match self.io_channel.lock().send(ClientIoMessage::UpdateBestAsCommitted(block_hash)) {
            Ok(_) => {}
            Err(e) => {
                cdebug!(CLIENT, "Error while triggering the best block update: {}", e);
            }
        }
    }

    fn get_kvdb(&self) -> Arc<KeyValueDB> {
        self.db.clone()
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

    fn highest_header(&self) -> encoded::Header {
        self.block_chain().highest_header()
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
    fn transaction_block(&self, id: &TransactionId) -> Option<H256> {
        self.parcel_address(id).map(|addr| addr.block_hash)
    }
}

impl TransactionInfo for Client {
    fn transaction_header(&self, hash: &H256) -> Option<::encoded::Header> {
        self.transaction_address(hash)
            .and_then(|addr| {
                addr.into_iter()
                    .find(|addr| {
                        let invoice = self.parcel_invoice(&TransactionId::from(*addr)).expect("Parcel must exist");
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

    fn queue_transactions(&self, transactions: Vec<Bytes>, peer_id: NodeId) {
        let queue_size = self.queue_transactions.load(AtomicOrdering::Relaxed);
        ctrace!(EXTERNAL_PARCEL, "Queue size: {}", queue_size);
        if queue_size > MAX_MEM_POOL_SIZE {
            debug!("Ignoring {} transactions: queue is full", transactions.len());
        } else {
            let len = transactions.len();
            match self.io_channel.lock().send(ClientIoMessage::NewParcels(transactions, peer_id)) {
                Ok(_) => {
                    self.queue_transactions.fetch_add(len, AtomicOrdering::SeqCst);
                }
                Err(e) => {
                    debug!("Ignoring {} transactions: error queueing: {}", len, e);
                }
            }
        }
    }

    fn ready_transactions(&self) -> Vec<SignedTransaction> {
        self.importer.miner.ready_transactions()
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

    fn parcel(&self, id: &TransactionId) -> Option<LocalizedTransaction> {
        let chain = self.block_chain();
        self.parcel_address(id).and_then(|address| chain.parcel(&address))
    }

    fn parcel_invoice(&self, id: &TransactionId) -> Option<Invoice> {
        let chain = self.block_chain();
        self.parcel_address(id).and_then(|address| chain.parcel_invoice(&address))
    }

    fn transaction(&self, tracker: &H256) -> Option<LocalizedTransaction> {
        let chain = self.block_chain();
        let address = self.transaction_address(tracker)?;
        address.into_iter().map(Into::into).map(|address| chain.parcel(&address)).next()?
    }

    fn transaction_invoices(&self, tracker: &H256) -> Vec<Invoice> {
        self.transaction_address(tracker)
            .map(|address| {
                address
                    .into_iter()
                    .map(Into::into)
                    .map(|address| self.parcel_invoice(&address).expect("The invoice must exist"))
                    .collect()
            })
            .unwrap_or_default()
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
    fn prepare_open_block(&self, parent_block_id: BlockId, author: Address, extra_data: Bytes) -> OpenBlock {
        let engine = &*self.engine;
        let chain = self.block_chain();
        let parent_hash = self.block_hash(&parent_block_id).expect("parent exist always");
        let parent_header = chain.block_header(&parent_hash).expect("parent exist always");

        let is_epoch_begin = chain.epoch_transition(parent_header.number(), parent_hash).is_some();
        OpenBlock::try_new(
            engine,
            self.state_db.read().clone(&parent_header.state_root()),
            &parent_header,
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

impl FindActionHandler for Client {
    fn find_action_handler_for(&self, id: u64) -> Option<&Arc<ActionHandler>> {
        self.engine.action_handlers().iter().find(|handler| handler.handler_id() == id)
    }
}
