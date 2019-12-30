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

use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Weak};

use cdb::{new_journaldb, Algorithm, AsHashDB, DatabaseError};
use cio::IoChannel;
use ckey::{Address, NetworkId, PlatformAddress, Public};
use cmerkle::Result as TrieResult;
use cnetwork::NodeId;
use cstate::{
    ActionHandler, AssetScheme, FindActionHandler, OwnedAsset, StateDB, StateResult, Text, TopLevelState, TopStateView,
};
use ctimer::{TimeoutHandler, TimerApi, TimerScheduleError, TimerToken};
use ctypes::transaction::{AssetTransferInput, PartialHashing, ShardTransaction};
use ctypes::{BlockHash, BlockNumber, CommonParams, ShardId, Tracker, TxHash};
use cvm::{decode, execute, ChainTimeInfo, ScriptResult, VMConfig};
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::{Mutex, RwLock, RwLockReadGuard};
use primitives::{Bytes, H160, H256, U256};
use rlp::Rlp;

use super::importer::Importer;
use super::{
    AccountData, AssetClient, BlockChainClient, BlockChainInfo, BlockChainTrait, BlockProducer, ChainNotify,
    ClientConfig, DatabaseClient, EngineClient, EngineInfo, ExecuteClient, ImportBlock, ImportResult,
    MiningBlockChainClient, Shard, StateInfo, StateOrBlock, TextClient,
};
use crate::block::{ClosedBlock, IsBlock, OpenBlock, SealedBlock};
use crate::blockchain::{BlockChain, BlockProvider, BodyProvider, HeaderProvider, InvoiceProvider, TransactionAddress};
use crate::client::{ConsensusClient, TermInfo};
use crate::consensus::{CodeChainEngine, EngineError};
use crate::encoded;
use crate::error::{BlockImportError, Error, ImportError, SchemeError};
use crate::miner::{Miner, MinerService};
use crate::scheme::Scheme;
use crate::service::ClientIoMessage;
use crate::transaction::{LocalizedTransaction, PendingSignedTransactions, SignedTransaction, UnverifiedTransaction};
use crate::types::{BlockId, BlockStatus, TransactionId, VerificationQueueInfo as BlockQueueInfo};

const MAX_MEM_POOL_SIZE: usize = 4096;

pub struct Client {
    engine: Arc<dyn CodeChainEngine>,

    io_channel: Mutex<IoChannel<ClientIoMessage>>,

    chain: RwLock<BlockChain>,

    /// Client uses this to store blocks, traces, etc.
    db: Arc<dyn KeyValueDB>,

    state_db: RwLock<StateDB>,

    /// List of actors to be notified on certain chain events
    notify: RwLock<Vec<Weak<dyn ChainNotify>>>,

    /// Count of pending transactions in the queue
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
        db: Arc<dyn KeyValueDB>,
        miner: Arc<Miner>,
        message_channel: IoChannel<ClientIoMessage>,
        reseal_timer: TimerApi,
    ) -> Result<Arc<Client>, Error> {
        let journal_db = new_journaldb(Arc::clone(&db), Algorithm::Archive, crate::db::COL_STATE);
        let mut state_db = StateDB::new(journal_db);
        if !scheme.check_genesis_root(state_db.as_hashdb()) {
            return Err(SchemeError::InvalidState.into())
        }
        if state_db.is_empty() {
            // Sets the correct state root.
            state_db = scheme.ensure_genesis_state(state_db)?;
            let mut batch = DBTransaction::new();
            state_db.journal_under(&mut batch, 0, *scheme.genesis_header().hash())?;
            db.write(batch)?;
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
        client.db.flush()?;
        Ok(client)
    }

    /// Returns engine reference.
    pub fn engine(&self) -> &dyn CodeChainEngine {
        &*self.engine
    }

    /// Adds an actor to be notified on certain events
    pub fn add_notify(&self, target: Weak<dyn ChainNotify>) {
        self.notify.write().push(target);
    }

    pub fn transactions_received(&self, hashes: &[TxHash], peer_id: NodeId) {
        self.notify(|notify| {
            notify.transactions_received(hashes.to_vec(), peer_id);
        });
    }

    pub fn new_blocks(
        &self,
        imported: &[BlockHash],
        invalid: &[BlockHash],
        enacted: &[BlockHash],
        retracted: &[BlockHash],
        sealed: &[BlockHash],
    ) {
        self.notify(|notify| {
            notify.new_blocks(
                imported.to_vec(),
                invalid.to_vec(),
                enacted.to_vec(),
                retracted.to_vec(),
                sealed.to_vec(),
            )
        });
    }

    pub fn new_headers(
        &self,
        imported: &[BlockHash],
        invalid: &[BlockHash],
        enacted: &[BlockHash],
        retracted: &[BlockHash],
        sealed: &[BlockHash],
        new_best_proposal: Option<BlockHash>,
    ) {
        self.notify(|notify| {
            notify.new_headers(
                imported.to_vec(),
                invalid.to_vec(),
                enacted.to_vec(),
                retracted.to_vec(),
                sealed.to_vec(),
                new_best_proposal,
            );
        });
    }

    fn notify<F>(&self, f: F)
    where
        F: Fn(&dyn ChainNotify), {
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

    fn block_hash(chain: &BlockChain, id: &BlockId) -> Option<BlockHash> {
        match id {
            BlockId::Hash(hash) => Some(*hash),
            BlockId::Number(number) => chain.block_hash(*number),
            BlockId::Earliest => chain.block_hash(0),
            BlockId::Latest => Some(chain.best_block_hash()),
            BlockId::ParentOfLatest => Some(chain.best_block_header().parent_hash()),
        }
    }

    fn transaction_address(&self, id: &TransactionId) -> Option<TransactionAddress> {
        match id {
            TransactionId::Hash(hash) => self.block_chain().transaction_address(hash),
            TransactionId::Location(id, index) => {
                Self::block_hash(&self.block_chain(), id).map(|hash| TransactionAddress {
                    block_hash: hash,
                    index: *index,
                })
            }
        }
    }

    fn transaction_addresses(&self, tracker: &Tracker) -> Option<TransactionAddress> {
        self.block_chain().transaction_address_by_tracker(tracker)
    }

    /// Import transactions from the IO queue
    pub fn import_queued_transactions(&self, transactions: &[Bytes], peer_id: NodeId) -> usize {
        ctrace!(EXTERNAL_TX, "Importing queued");
        self.queue_transactions.fetch_sub(transactions.len(), AtomicOrdering::SeqCst);
        let transactions: Vec<UnverifiedTransaction> =
            transactions.iter().filter_map(|bytes| Rlp::new(bytes).as_val().ok()).collect();
        let hashes: Vec<_> = transactions.iter().map(UnverifiedTransaction::hash).collect();
        self.transactions_received(&hashes, peer_id);
        let results = self.importer.miner.import_external_transactions(self, transactions);
        results.len()
    }

    /// This is triggered by a message coming from the Tendermint engine when a block is committed.
    /// See EngineClient::update_best_as_committed() for details.
    pub fn update_best_as_committed(&self, block_hash: BlockHash) {
        ctrace!(CLIENT, "Update the best block to the hash({}), as requested", block_hash);
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
        self.new_blocks(&[], &[], &enacted, &retracted, &[]);
    }

    fn block_number_ref(&self, id: &BlockId) -> Option<BlockNumber> {
        match id {
            BlockId::Number(number) => Some(*number),
            BlockId::Hash(hash) => self.block_chain().block_number(hash),
            BlockId::Earliest => Some(0),
            BlockId::Latest => Some(self.block_chain().best_block_detail().number),
            BlockId::ParentOfLatest => {
                if self.block_chain().best_block_detail().number == 0 {
                    None
                } else {
                    Some(self.block_chain().best_block_detail().number - 1)
                }
            }
        }
    }

    fn state_info(&self, state: StateOrBlock) -> Option<Box<dyn TopStateView>> {
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

    pub fn db(&self) -> &Arc<dyn KeyValueDB> {
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
                if !self.engine().engine_type().ignore_reseal_min_period() && !self.is_pending_queue_empty() {
                    self.update_sealing(BlockId::Latest, false);
                }
            }
            _ => unreachable!(),
        }
    }
}

impl DatabaseClient for Client {
    fn database(&self) -> Arc<dyn KeyValueDB> {
        Arc::clone(&self.db())
    }
}

impl AssetClient for Client {
    fn get_asset_scheme(&self, asset_type: H160, shard_id: ShardId, id: BlockId) -> TrieResult<Option<AssetScheme>> {
        if let Some(state) = Client::state_at(&self, id) {
            Ok(state.asset_scheme(shard_id, asset_type)?)
        } else {
            Ok(None)
        }
    }

    fn get_asset(
        &self,
        tracker: Tracker,
        index: usize,
        shard_id: ShardId,
        id: BlockId,
    ) -> TrieResult<Option<OwnedAsset>> {
        if let Some(state) = Client::state_at(&self, id) {
            Ok(state.asset(shard_id, tracker, index)?)
        } else {
            Ok(None)
        }
    }

    /// Checks whether an asset is spent or not.
    ///
    /// It returns None if such an asset never existed in the shard at the given block.
    fn is_asset_spent(
        &self,
        tracker: Tracker,
        index: usize,
        shard_id: ShardId,
        block_id: BlockId,
    ) -> TrieResult<Option<bool>> {
        let tx_address = match self.transaction_addresses(&tracker) {
            Some(itx_address) => itx_address,
            None => return Ok(None),
        };

        match self.block_number(&block_id) {
            None => return Ok(None),
            Some(block_number)
                if block_number
                    < self.block_number(&tx_address.block_hash.into()).expect("There is a successful transaction") =>
            {
                return Ok(None)
            }
            Some(_) => {}
        }

        let localized = self.transaction_by_tracker(&tracker).expect("There is a successful transaction");
        let transaction = if let Some(tx) = Option::<ShardTransaction>::from(localized.action.clone()) {
            tx
        } else {
            return Ok(None)
        };
        if !transaction.is_valid_shard_id_index(index, shard_id) {
            return Ok(None)
        }

        let state = Client::state_at(&self, block_id).unwrap();
        Ok(Some(state.asset(shard_id, tracker, index)?.is_none()))
    }
}

impl TextClient for Client {
    fn get_text(&self, tx_hash: TxHash, id: BlockId) -> TrieResult<Option<Text>> {
        if let Some(state) = Client::state_at(&self, id) {
            Ok(state.text(&tx_hash)?)
        } else {
            Ok(None)
        }
    }
}

impl ExecuteClient for Client {
    fn execute_transaction(&self, transaction: &ShardTransaction, sender: &Address) -> StateResult<()> {
        let mut state = Client::state_at(&self, BlockId::Latest).expect("Latest state MUST exist");
        state.apply_shard_transaction(
            transaction,
            sender,
            &[],
            self,
            self.best_block_header().number(),
            self.best_block_header().timestamp(),
        )
    }

    fn execute_vm(
        &self,
        tx: &dyn PartialHashing,
        inputs: &[AssetTransferInput],
        params: &[Vec<Bytes>],
        indices: &[usize],
    ) -> Result<Vec<String>, DatabaseError> {
        let mut results = Vec::with_capacity(indices.len());
        for (i, index) in indices.iter().enumerate() {
            let input = inputs.get(*index);
            let param = params.get(i);
            let result = match (input, param) {
                (Some(input), Some(param)) => {
                    let lock_script = decode(&input.lock_script);
                    let unlock_script = decode(&input.unlock_script);
                    match (lock_script, unlock_script) {
                        (Ok(lock_script), Ok(unlock_script)) => {
                            match execute(
                                &unlock_script,
                                &param,
                                &lock_script,
                                tx,
                                VMConfig::default(),
                                &input,
                                false,
                                self,
                                self.best_block_header().number(),
                                self.best_block_header().timestamp(),
                            ) {
                                Ok(ScriptResult::Burnt) => "burnt".to_string(),
                                Ok(ScriptResult::Unlocked) => "unlocked".to_string(),
                                _ => "failed".to_string(),
                            }
                        }
                        _ => "invalid".to_string(),
                    }
                }
                _ => "invalid".to_string(),
            };
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

impl EngineInfo for Client {
    fn network_id(&self) -> NetworkId {
        self.common_params(BlockId::Earliest).expect("Genesis state must exist").network_id()
    }

    fn common_params(&self, block_id: BlockId) -> Option<CommonParams> {
        self.state_info(block_id.into()).map(|state| {
            state
                .metadata()
                .unwrap_or_else(|err| unreachable!("Unexpected failure. Maybe DB was corrupted: {:?}", err))
                .unwrap()
                .params()
                .map(Clone::clone)
                .unwrap_or_else(|| *self.engine().machine().genesis_common_params())
        })
    }

    fn metadata_seq(&self, block_id: BlockId) -> Option<u64> {
        self.state_info(block_id.into()).map(|state| {
            state
                .metadata()
                .unwrap_or_else(|err| unreachable!("Unexpected failure. Maybe DB was corrupted: {:?}", err))
                .unwrap()
                .seq()
        })
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

    fn possible_authors(&self, block_number: Option<u64>) -> Result<Option<Vec<PlatformAddress>>, EngineError> {
        let network_id = self.network_id();
        if block_number == Some(0) {
            let genesis_author = self.block_header(&0.into()).expect("genesis block").author();
            return Ok(Some(vec![PlatformAddress::new_v1(network_id, genesis_author)]))
        }
        let addresses = self.engine().possible_authors(block_number)?;
        Ok(addresses.map(|addresses| {
            addresses.into_iter().map(|address| PlatformAddress::new_v1(network_id, address)).collect()
        }))
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
    fn submit_seal(&self, block_hash: BlockHash, seal: Vec<Bytes>) {
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
    fn update_best_as_committed(&self, block_hash: BlockHash) {
        ctrace!(ENGINE, "Requesting a best block update (block hash: {})", block_hash);
        match self.io_channel.lock().send(ClientIoMessage::UpdateBestAsCommitted(block_hash)) {
            Ok(_) => {}
            Err(e) => {
                cerror!(CLIENT, "Error while triggering the best block update: {}", e);
            }
        }
    }

    fn get_kvdb(&self) -> Arc<dyn KeyValueDB> {
        self.db.clone()
    }
}

impl ConsensusClient for Client {}

impl BlockChainTrait for Client {
    fn chain_info(&self) -> BlockChainInfo {
        let mut chain_info = self.block_chain().chain_info();
        chain_info.pending_total_score = chain_info.best_score + self.importer.block_queue.total_score();
        chain_info
    }

    fn genesis_accounts(&self) -> Vec<PlatformAddress> {
        let network_id = self.network_id();
        self.genesis_accounts.iter().map(|addr| PlatformAddress::new_v1(network_id, *addr)).collect()
    }

    fn block_header(&self, id: &BlockId) -> Option<encoded::Header> {
        let chain = self.block_chain();

        Self::block_hash(&chain, id).and_then(|hash| chain.block_header_data(&hash))
    }

    fn best_block_header(&self) -> encoded::Header {
        self.block_chain().best_block_header()
    }

    fn best_header(&self) -> encoded::Header {
        self.block_chain().best_header()
    }

    fn best_proposal_header(&self) -> encoded::Header {
        self.block_chain().best_proposal_header()
    }

    fn block(&self, id: &BlockId) -> Option<encoded::Block> {
        let chain = self.block_chain();

        Self::block_hash(&chain, id).and_then(|hash| chain.block(&hash))
    }

    fn transaction_block(&self, id: &TransactionId) -> Option<BlockHash> {
        self.transaction_address(id).map(|addr| addr.block_hash)
    }

    fn transaction_header(&self, tracker: &Tracker) -> Option<encoded::Header> {
        self.transaction_addresses(tracker).map(|addr| addr.block_hash).and_then(|hash| self.block_header(&hash.into()))
    }
}

impl ImportBlock for Client {
    fn import_block(&self, bytes: Bytes) -> Result<BlockHash, BlockImportError> {
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

    fn import_header(&self, bytes: Bytes) -> Result<BlockHash, BlockImportError> {
        let unverified = encoded::Header::new(bytes).decode();
        {
            if self.block_chain().is_known_header(&unverified.hash()) {
                return Err(BlockImportError::Import(ImportError::AlreadyInChain))
            }
        }
        Ok(self.importer.header_queue.import(unverified)?)
    }

    fn import_sealed_block(&self, block: &SealedBlock) -> ImportResult {
        let h = block.header().hash();
        let route = {
            // scope for self.import_lock
            let import_lock = self.importer.import_lock.lock();

            let number = block.header().number();
            let block_data = block.rlp_bytes();
            let header = block.header();

            self.importer.import_headers(vec![header], self, &import_lock);

            let route = self.importer.commit_block(block, header, &block_data, self);
            cinfo!(CLIENT, "Imported sealed block #{} ({})", number, h);
            route
        };
        let (enacted, retracted) = self.importer.calculate_enacted_retracted(&[route]);
        self.importer.miner.chain_new_blocks(self, &[h], &[], &enacted, &retracted);
        self.new_blocks(&[h], &[], &enacted, &retracted, &[h]);
        self.db().flush().expect("DB flush failed.");
        Ok(h)
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
}


impl BlockChainClient for Client {
    fn queue_info(&self) -> BlockQueueInfo {
        self.importer.block_queue.queue_info()
    }

    /// Import own transaction
    fn queue_own_transaction(&self, transaction: SignedTransaction) -> Result<(), Error> {
        self.importer.miner.import_own_transaction(self, transaction)?;
        Ok(())
    }

    fn queue_transactions(&self, transactions: Vec<Bytes>, peer_id: NodeId) {
        let queue_size = self.queue_transactions.load(AtomicOrdering::Relaxed);
        ctrace!(EXTERNAL_TX, "Queue size: {}", queue_size);
        if queue_size > MAX_MEM_POOL_SIZE {
            cwarn!(EXTERNAL_TX, "Ignoring {} transactions: queue is full", transactions.len());
        } else {
            let len = transactions.len();
            match self.io_channel.lock().send(ClientIoMessage::NewTransactions(transactions, peer_id)) {
                Ok(_) => {
                    self.queue_transactions.fetch_add(len, AtomicOrdering::SeqCst);
                }
                Err(e) => {
                    cwarn!(EXTERNAL_TX, "Ignoring {} transactions: error queueing: {}", len, e);
                }
            }
        }
    }

    fn delete_all_pending_transactions(&self) {
        self.importer.miner.delete_all_pending_transactions();
    }

    fn ready_transactions(&self, range: Range<u64>) -> PendingSignedTransactions {
        self.importer.miner.ready_transactions(range)
    }

    fn count_pending_transactions(&self, range: Range<u64>) -> usize {
        self.importer.miner.count_pending_transactions(range)
    }

    fn future_included_count_pending_transactions(&self, range: Range<u64>) -> usize {
        self.importer.miner.future_included_count_pending_transactions(range)
    }

    fn future_ready_transactions(&self, range: Range<u64>) -> PendingSignedTransactions {
        self.importer.miner.future_ready_transactions(range)
    }
    fn is_pending_queue_empty(&self) -> bool {
        self.importer.miner.status().transactions_in_pending_queue == 0
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

    fn block_hash(&self, id: &BlockId) -> Option<BlockHash> {
        let chain = self.block_chain();
        Self::block_hash(&chain, id)
    }

    fn transaction(&self, id: &TransactionId) -> Option<LocalizedTransaction> {
        let chain = self.block_chain();
        self.transaction_address(id).and_then(|address| chain.transaction(&address))
    }

    fn error_hint(&self, hash: &TxHash) -> Option<String> {
        let chain = self.block_chain();
        chain.error_hint(hash)
    }

    fn transaction_by_tracker(&self, tracker: &Tracker) -> Option<LocalizedTransaction> {
        let chain = self.block_chain();
        let address = self.transaction_addresses(tracker);
        address.and_then(|address| chain.transaction(&address))
    }

    fn error_hints_by_tracker(&self, tracker: &Tracker) -> Vec<(TxHash, Option<String>)> {
        let chain = self.block_chain();
        chain.error_hints_by_tracker(tracker)
    }
}

impl TermInfo for Client {
    fn last_term_finished_block_num(&self, id: BlockId) -> Option<BlockNumber> {
        self.state_at(id)
            .map(|state| state.metadata().unwrap().expect("Metadata always exist"))
            .map(|metadata| metadata.last_term_finished_block_num())
    }

    fn current_term_id(&self, id: BlockId) -> Option<u64> {
        self.state_at(id)
            .map(|state| state.metadata().unwrap().expect("Metadata always exist"))
            .map(|metadata| metadata.current_term_id())
    }

    fn term_common_params(&self, id: BlockId) -> Option<CommonParams> {
        let block_number = self.last_term_finished_block_num(id).expect("The block of the parent hash should exist");
        if block_number == 0 {
            None
        } else {
            Some(self.common_params((block_number).into()).expect("Common params should exist"))
        }
    }
}

impl AccountData for Client {
    fn seq(&self, address: &Address, id: BlockId) -> Option<u64> {
        self.state_at(id).and_then(|s| s.seq(address).ok())
    }

    fn balance(&self, address: &Address, state: StateOrBlock) -> Option<u64> {
        let state = self.state_info(state)?;
        state.balance(address).ok()
    }

    fn regular_key(&self, address: &Address, state: StateOrBlock) -> Option<Public> {
        let state = self.state_info(state)?;
        state.regular_key(address).ok()?
    }

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

    fn shard_id_by_hash(&self, create_shard_tx_hash: &TxHash, state: StateOrBlock) -> Option<u16> {
        let state = self.state_info(state)?;
        state.shard_id_by_hash(&create_shard_tx_hash).ok()?
    }

    fn shard_root(&self, shard_id: ShardId, state: StateOrBlock) -> Option<H256> {
        let state = self.state_info(state)?;
        state.shard_root(shard_id).ok()?
    }

    fn shard_owners(&self, shard_id: u16, state: StateOrBlock) -> Option<Vec<Address>> {
        let state = self.state_info(state)?;
        state.shard_owners(shard_id).ok()?
    }

    fn shard_users(&self, shard_id: u16, state: StateOrBlock) -> Option<Vec<Address>> {
        let state = self.state_info(state)?;
        state.shard_users(shard_id).ok()?
    }
}

impl BlockProducer for Client {
    fn reopen_block(&self, block: ClosedBlock) -> OpenBlock {
        let engine = &*self.engine;
        block.reopen(engine)
    }

    fn prepare_open_block(&self, parent_block_id: BlockId, author: Address, extra_data: Bytes) -> OpenBlock {
        let engine = &*self.engine;
        let chain = self.block_chain();
        let parent_hash = self.block_hash(&parent_block_id).expect("parent exist always");
        let parent_header = chain.block_header(&parent_hash).expect("parent exist always");

        OpenBlock::try_new(
            engine,
            self.state_db.read().clone(&parent_header.state_root()),
            &parent_header,
            author,
            extra_data,
        ).expect("OpenBlock::new only fails if parent state root invalid; state root of best block's header is never invalid; qed")
    }
}

impl MiningBlockChainClient for Client {
    fn get_malicious_users(&self) -> Vec<Address> {
        self.importer.miner.get_malicious_users()
    }

    fn release_malicious_users(&self, prisoner_vec: Vec<Address>) {
        self.importer.miner.release_malicious_users(prisoner_vec)
    }

    fn imprison_malicious_users(&self, prisoner_vec: Vec<Address>) {
        self.importer.miner.imprison_malicious_users(prisoner_vec)
    }

    fn get_immune_users(&self) -> Vec<Address> {
        self.importer.miner.get_immune_users()
    }

    fn register_immune_users(&self, immune_user_vec: Vec<Address>) {
        self.importer.miner.register_immune_users(immune_user_vec)
    }
}

impl ChainTimeInfo for Client {
    fn transaction_block_age(&self, tracker: &Tracker, parent_block_number: BlockNumber) -> Option<u64> {
        self.transaction_block_number(tracker).map(|block_number| parent_block_number - block_number)
    }

    fn transaction_time_age(&self, tracker: &Tracker, parent_timestamp: u64) -> Option<u64> {
        self.transaction_block_timestamp(tracker).map(|block_timestamp| parent_timestamp - block_timestamp)
    }
}

impl FindActionHandler for Client {
    fn find_action_handler_for(&self, id: u64) -> Option<&dyn ActionHandler> {
        self.engine.find_action_handler_for(id)
    }
}
