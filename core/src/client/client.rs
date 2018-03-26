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

use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use cbytes::Bytes;
use cio::IoChannel;
use ctypes::{Address, H256};
use kvdb::KeyValueDB;
use parking_lot::{Mutex, RwLock};

use super::{EngineClient, BlockChainInfo, BlockInfo, TransactionInfo,
            ChainInfo, ChainNotify, ClientConfig, ImportBlock,
            BlockChainClient, BlockChain as BlockChainTrait
};
use super::importer::Importer;
use super::super::blockchain::{BlockChain, BlockProvider, TransactionAddress};
use super::super::codechain_machine::CodeChainMachine;
use super::super::consensus::{CodeChainEngine, Solo};
use super::super::encoded;
use super::super::error::{Error, BlockImportError, ImportError};
use super::super::service::ClientIoMessage;
use super::super::spec::Spec;
use super::super::types::{BlockId, TransactionId};

const MAX_TX_QUEUE_SIZE: usize = 4096;

pub struct Client {
    engine: Arc<CodeChainEngine>,
    io_channel: Mutex<IoChannel<ClientIoMessage>>,

    chain: RwLock<Arc<BlockChain>>,

    /// List of actors to be notified on certain chain events
    notify: RwLock<Vec<Weak<ChainNotify>>>,

    /// Count of pending transactions in the queue
    queue_transactions: AtomicUsize,

    importer: Importer,
}

impl Client {
    pub fn new(
        config: ClientConfig,
        spec: &Spec,
        db: Arc<KeyValueDB>,
        message_channel: IoChannel<ClientIoMessage>,
    ) -> Result<Arc<Client>, Error> {
        let engine = spec.engine.clone();
        let gb = spec.genesis_block();
        let chain = Arc::new(BlockChain::new(&gb, db.clone()));

        let importer = Importer::new(&config, engine.clone())?;

        let client = Arc::new(Client {
            engine,
            io_channel: Mutex::new(message_channel),
            chain: RwLock::new(chain),
            notify: RwLock::new(Vec::new()),
            queue_transactions: AtomicUsize::new(0),
            importer,
        });

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

    fn notify<F>(&self, f: F) where F: Fn(&ChainNotify) {
        for np in self.notify.read().iter() {
            if let Some(n) = np.upgrade() {
                f(&*n);
            }
        }
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

    fn transaction_address(&self, id: TransactionId) -> Option<TransactionAddress> {
        match id {
            TransactionId::Hash(ref hash) => self.chain.read().transaction_address(hash),
            TransactionId::Location(id, index) => Self::block_hash(&self.chain.read(), id).map(|hash| TransactionAddress {
                block_hash: hash,
                index,
            })
        }
    }

    /// Import transactions from the IO queue
    pub fn import_queued_transactions(&self, transactions: &[Bytes], peer_id: usize) -> usize {
        self.queue_transactions.fetch_sub(transactions.len(), AtomicOrdering::SeqCst);
        unimplemented!();
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
    /// Broadcast a consensus message to the network.
    fn broadcast_consensus_message(&self, message: Bytes) {
        self.notify(|notify| notify.broadcast(message.clone()));
    }

    /// Make a new block and seal it.
    fn update_sealing(&self) {
        unimplemented!()
    }

    /// Submit a seal for a block in the mining queue.
    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>) {
        unimplemented!()
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

        Self::block_hash(&chain, id).and_then(|hash| {
            chain.block(&hash)
        })
    }
}

impl TransactionInfo for Client {
    fn transaction_block(&self, id: TransactionId) -> Option<H256> {
        self.transaction_address(id).map(|addr| addr.block_hash)
    }
}

impl ImportBlock for Client {
    fn import_block(&self, bytes: Bytes) -> Result<H256, BlockImportError> {
        use super::super::verification::queue::kind::BlockLike;
        use super::super::verification::queue::kind::blocks::Unverified;

        let unverified = Unverified::new(bytes);
        {
            if self.chain.read().is_known(&unverified.hash()) {
                return Err(BlockImportError::Import(ImportError::AlreadyInChain));
            }
        }
        Ok(self.importer.block_queue.import(unverified)?)
    }
}

impl BlockChainTrait for Client {}

impl BlockChainClient for Client {
    fn queue_transactions(&self, transactions: Vec<Bytes>, peer_id: usize) {
        let queue_size = self.queue_transactions.load(AtomicOrdering::Relaxed);
        trace!(target: "external_tx", "Queue size: {}", queue_size);
        if queue_size > MAX_TX_QUEUE_SIZE {
            debug!("Ignoring {} transactions: queue is full", transactions.len());
        } else {
            let len = transactions.len();
            match self.io_channel.lock().send(ClientIoMessage::NewTransactions(transactions, peer_id)) {
                Ok(_) => {
                    self.queue_transactions.fetch_add(len, AtomicOrdering::SeqCst);
                }
                Err(e) => {
                    debug!("Ignoring {} transactions: error queueing: {}", len, e);
                }
            }
        }
    }

    fn queue_consensus_message(&self, message: Bytes) {
        let channel = self.io_channel.lock().clone();
        if let Err(e) = channel.send(ClientIoMessage::NewConsensusMessage(message)) {
            debug!("Ignoring the message, error queueing: {}", e);
        }
    }
}
