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

use std::sync::Arc;

use ctypes::{BlockHash, BlockNumber, Tracker, TxHash};
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;
use primitives::H256;
use rlp::RlpStream;

use super::block_info::BestBlockChanged;
use super::body_db::{BodyDB, BodyProvider};
use super::extras::{BlockDetails, TransactionAddress};
use super::headerchain::{HeaderChain, HeaderProvider};
use super::invoice_db::{InvoiceDB, InvoiceProvider};
use super::route::{tree_route, ImportRoute};
use crate::blockchain_info::BlockChainInfo;
use crate::consensus::CodeChainEngine;
use crate::db;
use crate::encoded;
use crate::invoice::Invoice;
use crate::transaction::LocalizedTransaction;
use crate::views::{BlockView, HeaderView};

const BEST_BLOCK_KEY: &[u8] = b"best-block";
const BEST_PROPOSAL_BLOCK_KEY: &[u8] = b"best-proposal-block";

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct BlockChain {
    /// The hash of the best block of the canonical chain.
    best_block_hash: RwLock<BlockHash>,
    /// The hash of the block which has the best score among the proposal blocks
    best_proposal_block_hash: RwLock<BlockHash>,

    headerchain: HeaderChain,
    body_db: BodyDB,
    invoice_db: InvoiceDB,

    pending_best_block_hash: RwLock<Option<BlockHash>>,
    pending_best_proposal_block_hash: RwLock<Option<BlockHash>>,
}

impl BlockChain {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &[u8], db: Arc<dyn KeyValueDB>) -> Self {
        let genesis_block = BlockView::new(genesis);

        // load best block
        let best_block_hash = match db.get(db::COL_EXTRA, BEST_BLOCK_KEY).unwrap() {
            Some(hash) => H256::from_slice(&hash).into(),
            None => {
                let hash = genesis_block.hash();

                let mut batch = DBTransaction::new();
                batch.put(db::COL_EXTRA, BEST_BLOCK_KEY, &hash);
                db.write(batch).expect("Low level database error. Some issue with disk?");
                hash
            }
        };

        let best_proposal_block_hash = match db.get(db::COL_EXTRA, BEST_PROPOSAL_BLOCK_KEY).unwrap() {
            Some(hash) => H256::from_slice(&hash).into(),
            None => {
                let hash = genesis_block.hash();
                let mut batch = DBTransaction::new();
                batch.put(db::COL_EXTRA, BEST_PROPOSAL_BLOCK_KEY, &hash);
                db.write(batch).expect("Low level database error. Some issue with disk?");
                hash
            }
        };

        Self {
            best_block_hash: RwLock::new(best_block_hash),
            best_proposal_block_hash: RwLock::new(best_proposal_block_hash),

            headerchain: HeaderChain::new(&genesis_block.header_view(), db.clone()),
            body_db: BodyDB::new(&genesis_block, db.clone()),
            invoice_db: InvoiceDB::new(db.clone()),

            pending_best_block_hash: RwLock::new(None),
            pending_best_proposal_block_hash: RwLock::new(None),
        }
    }

    pub fn insert_header(
        &self,
        batch: &mut DBTransaction,
        header: &HeaderView,
        engine: &dyn CodeChainEngine,
    ) -> ImportRoute {
        match self.headerchain.insert_header(batch, header, engine) {
            Some(c) => ImportRoute::new_from_best_header_changed(header.hash(), &c),
            None => ImportRoute::none(),
        }
    }

    pub fn insert_floating_header(&self, batch: &mut DBTransaction, header: &HeaderView) {
        self.headerchain.insert_floating_header(batch, header);
    }

    pub fn insert_floating_block(&self, batch: &mut DBTransaction, bytes: &[u8]) {
        let block = BlockView::new(bytes);
        let header = block.header_view();
        let hash = header.hash();

        ctrace!(BLOCKCHAIN, "Inserting bootstrap block #{}({}) to the blockchain.", header.number(), hash);

        if self.is_known(&hash) {
            cdebug!(BLOCKCHAIN, "Block #{}({}) is already known.", header.number(), hash);
            return
        }

        self.insert_floating_header(batch, &header);
        self.body_db.insert_body(batch, &block);
    }

    pub fn force_update_best_block(&self, batch: &mut DBTransaction, hash: &BlockHash) {
        ctrace!(BLOCKCHAIN, "Forcefully updating the best block to {}", hash);

        assert!(self.is_known(hash));
        assert!(self.pending_best_block_hash.read().is_none());
        assert!(self.pending_best_proposal_block_hash.read().is_none());

        let block = self.block(hash).expect("Target block is known");
        self.headerchain.force_update_best_header(batch, hash);
        self.body_db.update_best_block(batch, &BestBlockChanged::CanonChainAppended {
            best_block: block.into_inner(),
        });

        batch.put(db::COL_EXTRA, BEST_BLOCK_KEY, hash);
        *self.pending_best_block_hash.write() = Some(*hash);
        batch.put(db::COL_EXTRA, BEST_PROPOSAL_BLOCK_KEY, hash);
        *self.pending_best_proposal_block_hash.write() = Some(*hash);
    }

    /// Inserts the block into backing cache database.
    /// Expects the block to be valid and already verified.
    /// If the block is already known, does nothing.
    pub fn insert_block(
        &self,
        batch: &mut DBTransaction,
        bytes: &[u8],
        invoices: Vec<Invoice>,
        engine: &dyn CodeChainEngine,
    ) -> ImportRoute {
        // create views onto rlp
        let new_block = BlockView::new(bytes);
        let new_header = new_block.header_view();
        let new_block_hash = new_header.hash();

        ctrace!(BLOCKCHAIN, "Inserting block #{}({}) to the blockchain.", new_header.number(), new_block_hash);

        if self.is_known(&new_block_hash) {
            cdebug!(BLOCKCHAIN, "Block #{}({}) is already known.", new_header.number(), new_block_hash);
            return ImportRoute::none()
        }

        assert!(self.pending_best_block_hash.read().is_none());
        assert!(self.pending_best_proposal_block_hash.read().is_none());

        let best_block_changed = self.best_block_changed(&new_block, engine);

        self.headerchain.insert_header(batch, &new_header, engine);
        self.body_db.insert_body(batch, &new_block);
        self.body_db.update_best_block(batch, &best_block_changed);
        for invoice in invoices {
            self.invoice_db.insert_invoice(batch, invoice.hash, invoice.tracker, invoice.error);
        }

        if let Some(best_block_hash) = best_block_changed.new_best_hash() {
            let mut pending_best_block_hash = self.pending_best_block_hash.write();
            batch.put(db::COL_EXTRA, BEST_BLOCK_KEY, &best_block_hash);
            *pending_best_block_hash = Some(best_block_hash);

            let mut pending_best_proposal_block_hash = self.pending_best_proposal_block_hash.write();
            batch.put(db::COL_EXTRA, BEST_PROPOSAL_BLOCK_KEY, &*new_block_hash);
            *pending_best_proposal_block_hash = Some(new_block_hash);
        }

        ImportRoute::new(new_block_hash, &best_block_changed)
    }

    /// Apply pending insertion updates
    pub fn commit(&self) {
        ctrace!(BLOCKCHAIN, "Committing.");
        self.headerchain.commit();
        self.body_db.commit();
        // NOTE: There are no commit for InvoiceDB

        let mut best_block_hash = self.best_block_hash.write();
        let mut pending_best_block_hash = self.pending_best_block_hash.write();
        let mut best_proposal_block_hash = self.best_proposal_block_hash.write();
        let mut pending_best_proposal_block_hash = self.pending_best_proposal_block_hash.write();

        // update best block
        if let Some(hash) = pending_best_block_hash.take() {
            *best_block_hash = hash;
        }

        if let Some(hash) = pending_best_proposal_block_hash.take() {
            *best_proposal_block_hash = hash;
        }
    }

    /// Calculate how best block is changed
    fn best_block_changed(&self, new_block: &BlockView, engine: &dyn CodeChainEngine) -> BestBlockChanged {
        let new_header = new_block.header_view();
        let parent_hash_of_new_block = new_header.parent_hash();
        let parent_details_of_new_block = self.block_details(&parent_hash_of_new_block).expect("Invalid parent hash");
        let grandparent_hash_of_new_block = parent_details_of_new_block.parent;
        let prev_best_hash = self.best_block_hash();

        if parent_details_of_new_block.total_score + new_header.score() > self.best_proposal_block_detail().total_score
            && engine.can_change_canon_chain(
                new_header.hash(),
                parent_hash_of_new_block,
                grandparent_hash_of_new_block,
                prev_best_hash,
            )
        {
            cinfo!(
                BLOCKCHAIN,
                "Block #{}({}) has higher total score, changing the best proposal/canonical chain.",
                new_header.number(),
                new_header.hash()
            );

            let route = tree_route(self, prev_best_hash, parent_hash_of_new_block)
                .expect("blocks being imported always within recent history; qed");

            let new_best_block_hash = engine.get_best_block_from_best_proposal_header(&new_header);
            let new_best_block = if new_best_block_hash != new_header.hash() {
                self.block(&new_best_block_hash)
                    .expect("Best block is already imported as a branch")
                    .rlp()
                    .as_raw()
                    .to_vec()
            } else {
                new_block.rlp().as_raw().to_vec()
            };
            match route.retracted.len() {
                0 => BestBlockChanged::CanonChainAppended {
                    best_block: new_best_block,
                },
                _ => BestBlockChanged::BranchBecomingCanonChain {
                    tree_route: route,
                    best_block: new_best_block,
                },
            }
        } else {
            BestBlockChanged::None
        }
    }

    /// Update the best block as the given block hash from the commit state
    /// in Tendermint.
    ///
    /// The new best block should be a child of the current best block.
    /// This will not change the best proposal block chain. This means it is possible
    /// to have the best block and the best proposal block in different branches.
    pub fn update_best_as_committed(&self, batch: &mut DBTransaction, block_hash: BlockHash) -> ImportRoute {
        // FIXME: If it is possible, double check with the consensus engine.
        ctrace!(BLOCKCHAIN, "Update the best block to {}", block_hash);

        assert!(self.pending_best_block_hash.read().is_none());
        let block_detail = self.block_details(&block_hash).expect("The given hash should exist");
        let prev_best_block_detail = self.best_block_detail();
        let parent_hash = block_detail.parent;
        let prev_best_hash = self.best_block_hash();

        if parent_hash != prev_best_hash {
            cwarn!(
                BLOCKCHAIN,
                "Tried to update the best block but blocks are inserted: Input - #{}({}), Current best - #{}({})",
                block_detail.number,
                block_hash,
                prev_best_block_detail.number,
                prev_best_hash
            );

            assert!(
                block_detail.number <= prev_best_block_detail.number,
                "{} <= {}",
                block_detail.number,
                prev_best_block_detail.number
            );
            return ImportRoute::none()
        }

        assert_eq!(block_detail.number, prev_best_block_detail.number + 1);

        let best_block =
            self.block(&block_hash).expect("Best block is already imported as a branch").rlp().as_raw().to_vec();

        let best_block_changed = BestBlockChanged::CanonChainAppended {
            best_block,
        };

        self.headerchain.update_best_as_committed(batch, block_hash);
        self.body_db.update_best_block(batch, &best_block_changed);

        let mut pending_best_block_hash = self.pending_best_block_hash.write();
        batch.put(db::COL_EXTRA, BEST_BLOCK_KEY, &block_hash);
        *pending_best_block_hash = Some(block_hash);

        let mut pending_best_proposal_block_hash = self.pending_best_proposal_block_hash.write();
        batch.put(db::COL_EXTRA, BEST_PROPOSAL_BLOCK_KEY, &*block_hash);
        *pending_best_proposal_block_hash = Some(block_hash);

        ImportRoute::new(block_hash, &best_block_changed)
    }

    /// Returns general blockchain information
    pub fn chain_info(&self) -> BlockChainInfo {
        let best_block_hash = self.best_block_hash();
        let best_proposal_block_hash = self.best_proposal_block_hash();

        let best_block_detail = self.block_details(&best_block_hash).expect("Best block always exists");
        let best_block_header = self.block_header_data(&best_block_hash).expect("Best block always exists");

        let best_proposal_block_detail =
            self.block_details(&best_proposal_block_hash).expect("Best proposal block always exists");

        BlockChainInfo {
            best_score: best_block_detail.total_score,
            best_proposal_score: best_proposal_block_detail.total_score,
            pending_total_score: best_block_detail.total_score,
            genesis_hash: self.genesis_hash(),
            best_block_hash: best_block_header.hash(),
            best_proposal_block_hash,
            best_block_number: best_block_detail.number,
            best_block_timestamp: best_block_header.timestamp(),
        }
    }

    /// Get best block hash.
    pub fn best_block_hash(&self) -> BlockHash {
        *self.best_block_hash.read()
    }

    /// Get best_proposal block hash.
    pub fn best_proposal_block_hash(&self) -> BlockHash {
        *self.best_proposal_block_hash.read()
    }

    /// Get best block detail
    pub fn best_block_detail(&self) -> BlockDetails {
        self.block_details(&self.best_block_hash()).expect("Best block always exists")
    }

    /// Get best_proposal block detail
    pub fn best_proposal_block_detail(&self) -> BlockDetails {
        self.block_details(&self.best_proposal_block_hash()).expect("Best proposal block always exists")
    }

    /// Get best block header
    pub fn best_block_header(&self) -> encoded::Header {
        self.block_header_data(&self.best_block_hash()).expect("Best block always exists")
    }

    /// Get the best header
    pub fn best_header(&self) -> encoded::Header {
        self.headerchain.best_header()
    }

    pub fn best_proposal_header(&self) -> encoded::Header {
        self.headerchain.best_proposal_header()
    }
}

/// Interface for querying blocks by hash and by number.
pub trait BlockProvider: HeaderProvider + BodyProvider + InvoiceProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known(&self, hash: &BlockHash) -> bool {
        self.is_known_header(hash) && self.is_known_body(hash)
    }

    /// Get raw block data
    fn block(&self, hash: &BlockHash) -> Option<encoded::Block> {
        let header = self.block_header_data(hash)?;
        let body = self.block_body(hash)?;

        let mut block = RlpStream::new_list(2);
        let body_rlp = body.rlp();
        block.append_raw(header.rlp().as_raw(), 1);
        block.append_raw(body_rlp.at(0).unwrap().as_raw(), 1);
        let encoded_block = encoded::Block::new(block.out());
        debug_assert_eq!(*hash, encoded_block.hash());
        Some(encoded_block)
    }

    /// Get transaction with given transaction hash.
    fn transaction(&self, address: &TransactionAddress) -> Option<LocalizedTransaction> {
        self.block_body(&address.block_hash).and_then(|body| {
            self.block_number(&address.block_hash)
                .and_then(|n| body.view().localized_transaction_at(&address.block_hash, n, address.index))
        })
    }

    /// Get a list of transactions for a given block.
    /// Returns None if block does not exist.
    fn transactions(&self, block_hash: &BlockHash) -> Option<Vec<LocalizedTransaction>> {
        self.block_body(block_hash)
            .and_then(|body| self.block_number(block_hash).map(|n| body.view().localized_transactions(block_hash, n)))
    }
}

impl HeaderProvider for BlockChain {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known_header(&self, hash: &BlockHash) -> bool {
        self.headerchain.is_known_header(hash)
    }

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &BlockHash) -> Option<BlockDetails> {
        self.headerchain.block_details(hash)
    }

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<BlockHash> {
        self.headerchain.block_hash(index)
    }

    /// Get the header RLP of a block.
    fn block_header_data(&self, hash: &BlockHash) -> Option<encoded::Header> {
        self.headerchain.block_header_data(hash)
    }
}

impl BodyProvider for BlockChain {
    fn is_known_body(&self, hash: &BlockHash) -> bool {
        self.body_db.is_known_body(hash)
    }

    fn transaction_address(&self, hash: &TxHash) -> Option<TransactionAddress> {
        self.body_db.transaction_address(hash)
    }

    fn transaction_address_by_tracker(&self, tracker: &Tracker) -> Option<TransactionAddress> {
        self.body_db.transaction_address_by_tracker(tracker)
    }

    fn block_body(&self, hash: &BlockHash) -> Option<encoded::Body> {
        self.body_db.block_body(hash)
    }
}

impl InvoiceProvider for BlockChain {
    /// Returns true if invoices for given hash is known
    fn is_known_error_hint(&self, hash: &TxHash) -> bool {
        self.invoice_db.is_known_error_hint(hash)
    }

    fn error_hints_by_tracker(&self, tracker: &Tracker) -> Vec<(TxHash, Option<String>)> {
        self.invoice_db.error_hints_by_tracker(tracker)
    }

    fn error_hint(&self, hash: &TxHash) -> Option<String> {
        self.invoice_db.error_hint(hash)
    }
}

impl BlockProvider for BlockChain {}
