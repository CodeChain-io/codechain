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

use std::mem;
use std::sync::Arc;

use ctypes::invoice::{BlockInvoices, ParcelInvoice};
use ctypes::parcel::Action;
use ctypes::transaction::Transaction;
use ctypes::BlockNumber;
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;
use primitives::H256;
use rlp::RlpStream;

use super::super::blockchain_info::BlockChainInfo;
use super::super::consensus::epoch::{PendingTransition as PendingEpochTransition, Transition as EpochTransition};
use super::super::db::{self, Readable, Writable};
use super::super::encoded;
use super::super::parcel::LocalizedParcel;
use super::super::views::{BlockView, HeaderView};
use super::block_info::BlockLocation;
use super::body_db::{BodyDB, BodyProvider};
use super::extras::{BlockDetails, EpochTransitions, ParcelAddress, TransactionAddress, EPOCH_KEY_PREFIX};
use super::headerchain::{HeaderChain, HeaderProvider};
use super::invoice_db::{InvoiceDB, InvoiceProvider};
use super::route::{tree_route, ImportRoute};

const BEST_BLOCK_KEY: &[u8] = b"best-block";

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct BlockChain {
    best_block_hash: RwLock<H256>,

    headerchain: HeaderChain,
    body_db: BodyDB,
    invoice_db: InvoiceDB,

    db: Arc<KeyValueDB>,

    pending_best_block_hash: RwLock<Option<H256>>,
}

impl BlockChain {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &[u8], db: Arc<KeyValueDB>) -> Self {
        let genesis_block = BlockView::new(genesis);

        // load best block
        let best_block_hash = match db.get(db::COL_EXTRA, BEST_BLOCK_KEY).unwrap() {
            Some(hash) => H256::from_slice(&hash),
            None => {
                let hash = genesis_block.hash();

                let mut batch = DBTransaction::new();
                batch.put(db::COL_EXTRA, BEST_BLOCK_KEY, &hash);
                db.write(batch).expect("Low level database error. Some issue with disk?");
                hash
            }
        };

        Self {
            best_block_hash: RwLock::new(best_block_hash),

            headerchain: HeaderChain::new(&genesis_block.header_view(), db.clone()),
            body_db: BodyDB::new(&genesis_block, db.clone()),
            invoice_db: InvoiceDB::new(db.clone()),

            db,

            pending_best_block_hash: RwLock::new(None),
        }
    }

    pub fn insert_header(&self, batch: &mut DBTransaction, header: &HeaderView) -> ImportRoute {
        match self.headerchain.insert_header(batch, header) {
            Some(l) => ImportRoute::new(&header.hash(), &l),
            None => ImportRoute::none(),
        }
    }

    /// Inserts the block into backing cache database.
    /// Expects the block to be valid and already verified.
    /// If the block is already known, does nothing.
    pub fn insert_block(&self, batch: &mut DBTransaction, bytes: &[u8], invoices: Vec<ParcelInvoice>) -> ImportRoute {
        // create views onto rlp
        let block = BlockView::new(bytes);
        let header = block.header_view();
        let hash = header.hash();

        if self.is_known(&hash) {
            return ImportRoute::none()
        }

        assert!(self.pending_best_block_hash.read().is_none());

        let location = self.block_location(&block);

        self.headerchain.insert_header(batch, &header);
        self.body_db.insert_body(batch, &block, &location);
        self.invoice_db.insert_invoice(batch, &hash, invoices);

        if location != BlockLocation::Branch {
            let mut pending_best_block_hash = self.pending_best_block_hash.write();
            batch.put(db::COL_EXTRA, BEST_BLOCK_KEY, &header.hash());
            *pending_best_block_hash = Some(header.hash());
        }

        ImportRoute::new(&hash, &location)
    }

    /// Apply pending insertion updates
    pub fn commit(&self) {
        self.headerchain.commit();
        self.body_db.commit();
        // NOTE: There are no commit for InvoiceDB

        let mut best_block_hash = self.best_block_hash.write();
        let mut pending_best_block_hash = self.pending_best_block_hash.write();
        // update best block
        if let Some(hash) = pending_best_block_hash.take() {
            *best_block_hash = hash;
        }
    }

    /// Calculate insert location for new block
    fn block_location(&self, block: &BlockView) -> BlockLocation {
        let header = block.header_view();
        let parent_hash = header.parent_hash();
        let parent_details = self.block_details(&parent_hash).expect("Invalid parent hash");

        if parent_details.total_score + header.score() > self.best_block_detail().total_score {
            let best_hash = self.best_block_hash();
            let route = tree_route(self, best_hash, parent_hash)
                .expect("blocks being imported always within recent history; qed");

            match route.retracted.len() {
                0 => BlockLocation::CanonChain,
                _ => BlockLocation::BranchBecomingCanonChain(route),
            }
        } else {
            BlockLocation::Branch
        }
    }

    /// Returns general blockchain information
    pub fn chain_info(&self) -> BlockChainInfo {
        let best_block_header = self.best_block_header();
        let best_block_detail = self.best_block_detail();

        BlockChainInfo {
            total_score: best_block_detail.total_score.clone(),
            pending_total_score: best_block_detail.total_score.clone(),
            genesis_hash: self.genesis_hash(),
            best_block_hash: best_block_header.hash(),
            best_block_number: best_block_detail.number,
            best_block_timestamp: best_block_header.timestamp(),
        }
    }

    /// Get best block hash.
    pub fn best_block_hash(&self) -> H256 {
        self.best_block_hash.read().clone()
    }

    /// Get best block detail
    pub fn best_block_detail(&self) -> BlockDetails {
        self.block_details(&self.best_block_hash()).expect("Best block always exists")
    }

    /// Get best block header
    pub fn best_block_header(&self) -> encoded::Header {
        self.block_header_data(&self.best_block_hash()).expect("Best block always exists")
    }

    /// Insert an epoch transition. Provide an epoch number being transitioned to
    /// and epoch transition object.
    ///
    /// The block the transition occurred at should have already been inserted into the chain.
    pub fn insert_epoch_transition(&self, batch: &mut DBTransaction, epoch_num: u64, transition: EpochTransition) {
        let mut transitions = match self.db.read(db::COL_EXTRA, &epoch_num) {
            Some(existing) => existing,
            None => EpochTransitions {
                number: epoch_num,
                candidates: Vec::with_capacity(1),
            },
        };

        // ensure we don't write any duplicates.
        if transitions.candidates.iter().find(|c| c.block_hash == transition.block_hash).is_none() {
            transitions.candidates.push(transition);
            batch.write(db::COL_EXTRA, &epoch_num, &transitions);
        }
    }

    /// Iterate over all epoch transitions.
    /// This will only return transitions within the canonical chain.
    #[allow(dead_code)]
    pub fn epoch_transitions(&self) -> EpochTransitionIter {
        let iter = self.db.iter_from_prefix(db::COL_EXTRA, &EPOCH_KEY_PREFIX[..]);
        EpochTransitionIter {
            chain: self,
            prefix_iter: iter,
        }
    }

    /// Get a specific epoch transition by block number and provided block hash.
    pub fn epoch_transition(&self, block_num: u64, block_hash: H256) -> Option<EpochTransition> {
        ctrace!(BLOCKCHAIN, "Loading epoch transition at block {}, {}", block_num, block_hash);

        self.db.read(db::COL_EXTRA, &block_num).and_then(|transitions: EpochTransitions| {
            transitions.candidates.into_iter().find(|c| c.block_hash == block_hash)
        })
    }

    /// Get the transition to the epoch the given parent hash is part of
    /// or transitions to.
    /// This will give the epoch that any children of this parent belong to.
    ///
    /// The block corresponding the the parent hash must be stored already.
    #[allow(dead_code)]
    pub fn epoch_transition_for(&self, parent_hash: H256) -> Option<EpochTransition> {
        // slow path: loop back block by block
        for hash in self.ancestry_iter(parent_hash)? {
            let details = self.block_details(&hash)?;

            // look for transition in database.
            if let Some(transition) = self.epoch_transition(details.number, hash) {
                return Some(transition)
            }

            // canonical hash -> fast breakout:
            // get the last epoch transition up to this block.
            //
            // if `block_hash` is canonical it will only return transitions up to
            // the parent.
            if self.block_hash(details.number)? == hash {
                return self.epoch_transitions().map(|(_, t)| t).take_while(|t| t.block_number <= details.number).last()
            }
        }

        // should never happen as the loop will encounter genesis before concluding.
        None
    }

    /// Iterator that lists `first` and then all of `first`'s ancestors, by hash.
    #[allow(dead_code)]
    pub fn ancestry_iter(&self, first: H256) -> Option<AncestryIter> {
        if self.is_known(&first) {
            Some(AncestryIter {
                current: first,
                chain: self,
            })
        } else {
            None
        }
    }

    /// Write a pending epoch transition by block hash.
    pub fn insert_pending_transition(&self, batch: &mut DBTransaction, hash: H256, t: PendingEpochTransition) {
        batch.write(db::COL_EXTRA, &hash, &t);
    }

    /// Get a pending epoch transition by block hash.
    // TODO: implement removal safely: this can only be done upon finality of a block
    // that _uses_ the pending transition.
    pub fn get_pending_transition(&self, hash: H256) -> Option<PendingEpochTransition> {
        self.db.read(db::COL_EXTRA, &hash)
    }
}

/// An iterator which walks the blockchain towards the genesis.
#[derive(Clone)]
pub struct AncestryIter<'a> {
    current: H256,
    chain: &'a BlockChain,
}

impl<'a> Iterator for AncestryIter<'a> {
    type Item = H256;
    fn next(&mut self) -> Option<H256> {
        if self.current.is_zero() {
            None
        } else {
            self.chain.block_details(&self.current).map(|details| mem::replace(&mut self.current, details.parent))
        }
    }
}

/// An iterator which walks all epoch transitions.
/// Returns epoch transitions.
#[allow(dead_code)]
pub struct EpochTransitionIter<'a> {
    chain: &'a BlockChain,
    prefix_iter: Box<Iterator<Item = (Box<[u8]>, Box<[u8]>)> + 'a>,
}

impl<'a> Iterator for EpochTransitionIter<'a> {
    type Item = (u64, EpochTransition);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // some epochs never occurred on the main chain.
            let (key, val) = self.prefix_iter.next()?;

            // iterator may continue beyond values beginning with this
            // prefix.
            if !key.starts_with(&EPOCH_KEY_PREFIX[..]) {
                return None
            }

            let transitions: EpochTransitions = ::rlp::decode(&val[..]);

            // if there are multiple candidates, at most one will be on the
            // canon chain.
            for transition in transitions.candidates.into_iter() {
                let is_in_canon_chain =
                    self.chain.block_hash(transition.block_number).map_or(false, |hash| hash == transition.block_hash);

                if is_in_canon_chain {
                    return Some((transitions.number, transition))
                }
            }
        }
    }
}

/// Interface for querying blocks by hash and by number.
pub trait BlockProvider: HeaderProvider + BodyProvider + InvoiceProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known(&self, hash: &H256) -> bool {
        self.is_known_header(hash) && self.is_known_body(hash)
    }

    /// Get raw block data
    fn block(&self, hash: &H256) -> Option<encoded::Block> {
        let header = self.block_header_data(hash)?;
        let body = self.block_body(hash)?;

        let mut block = RlpStream::new_list(2);
        let body_rlp = body.rlp();
        block.append_raw(header.rlp().as_raw(), 1);
        block.append_raw(body_rlp.at(0).as_raw(), 1);
        Some(encoded::Block::new(block.out()))
    }

    /// Get parcel with given parcel hash.
    fn parcel(&self, address: &ParcelAddress) -> Option<LocalizedParcel> {
        self.block_body(&address.block_hash).and_then(|body| {
            self.block_number(&address.block_hash)
                .and_then(|n| body.view().localized_parcel_at(&address.block_hash, n, address.index))
        })
    }

    /// Get the transaction with given transaction hash.
    fn transaction(&self, transaction: &TransactionAddress) -> Option<Transaction> {
        self.parcel(&transaction.parcel_address).and_then(|parcel| match &parcel.signed.as_unsigned().action {
            Action::ChangeShardState {
                transactions,
                changes: _,
                signatures: _,
            }
                if transaction.index < transactions.len() =>
            {
                Some(transactions[transaction.index].clone())
            }
            _ => None,
        })
    }

    /// Get a list of parcels for a given block.
    /// Returns None if block does not exist.
    fn parcels(&self, hash: &H256) -> Option<Vec<LocalizedParcel>> {
        self.block_body(hash).and_then(|body| self.block_number(hash).map(|n| body.view().localized_parcels(hash, n)))
    }
}

impl HeaderProvider for BlockChain {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known_header(&self, hash: &H256) -> bool {
        self.headerchain.is_known_header(hash)
    }

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails> {
        self.headerchain.block_details(hash)
    }

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        self.headerchain.block_hash(index)
    }

    /// Get the header RLP of a block.
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
        self.headerchain.block_header_data(hash)
    }
}

impl BodyProvider for BlockChain {
    fn is_known_body(&self, hash: &H256) -> bool {
        self.body_db.is_known_body(hash)
    }

    fn parcel_address(&self, hash: &H256) -> Option<ParcelAddress> {
        self.body_db.parcel_address(hash)
    }

    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        self.body_db.transaction_address(hash)
    }

    fn block_body(&self, hash: &H256) -> Option<encoded::Body> {
        self.body_db.block_body(hash)
    }
}

impl InvoiceProvider for BlockChain {
    /// Returns true if invoices for given hash is known
    fn is_known_invoice(&self, hash: &H256) -> bool {
        self.invoice_db.is_known_invoice(hash)
    }

    /// Get invoices of block with given hash.
    fn block_invoices(&self, hash: &H256) -> Option<BlockInvoices> {
        self.invoice_db.block_invoices(hash)
    }

    /// Get parcel invoice.
    fn parcel_invoice(&self, address: &ParcelAddress) -> Option<ParcelInvoice> {
        self.invoice_db.parcel_invoice(address)
    }
}

impl BlockProvider for BlockChain {}
