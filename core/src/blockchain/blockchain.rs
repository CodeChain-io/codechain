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

use std::collections::HashMap;
use std::mem;
use std::sync::Arc;

use cbytes::Bytes;
use ctypes::{H256, U256};
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;
use rlp::{Rlp, RlpStream};
use rlp_compress::{compress, decompress, blocks_swapper};

use super::best_block::BestBlock;
use super::extras::{BlockDetails, TransactionAddress, BlockInvoices};
use super::block_info::{BlockInfo, BlockLocation, BranchBecomingCanonChainData};
use super::super::blockchain_info::BlockChainInfo;
use super::super::db::{self, Readable, Writable, CacheUpdatePolicy};
use super::super::encoded;
use super::super::header::Header;
use super::super::invoice::Invoice;
use super::super::transaction::{LocalizedTransaction};
use super::super::types::BlockNumber;
use super::super::views::{BlockView, HeaderView};

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct BlockChain {
    // All locks must be captured in the order declared here.
    best_block: RwLock<BestBlock>,

    // block cache
    block_headers: RwLock<HashMap<H256, Bytes>>,
    block_bodies: RwLock<HashMap<H256, Bytes>>,

    // extra caches
    block_details: RwLock<HashMap<H256, BlockDetails>>,
    block_hashes: RwLock<HashMap<BlockNumber, H256>>,
    transaction_addresses: RwLock<HashMap<H256, TransactionAddress>>,
    block_invoices: RwLock<HashMap<H256, BlockInvoices>>,

    db: Arc<KeyValueDB>,

    pending_best_block: RwLock<Option<BestBlock>>,
    pending_block_hashes: RwLock<HashMap<BlockNumber, H256>>,
    pending_block_details: RwLock<HashMap<H256, BlockDetails>>,
    pending_transaction_addresses: RwLock<HashMap<H256, Option<TransactionAddress>>>,
}

impl BlockChain {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &[u8], db: Arc<KeyValueDB>) -> BlockChain {
        let bc = BlockChain {
            best_block: RwLock::new(BestBlock::default()),
            block_headers: RwLock::new(HashMap::new()),
            block_bodies: RwLock::new(HashMap::new()),
            block_details: RwLock::new(HashMap::new()),
            block_hashes: RwLock::new(HashMap::new()),
            transaction_addresses: RwLock::new(HashMap::new()),
            block_invoices: RwLock::new(HashMap::new()),
            db: db.clone(),
            pending_best_block: RwLock::new(None),
            pending_block_hashes: RwLock::new(HashMap::new()),
            pending_block_details: RwLock::new(HashMap::new()),
            pending_transaction_addresses: RwLock::new(HashMap::new()),
        };

        // load best block
        let best_block_hash = match bc.db.get(db::COL_EXTRA, b"best").unwrap() {
            Some(best) => {
                H256::from_slice(&best)
            }
            None => {
                // best block does not exist
                // we need to insert genesis into the cache
                let block = BlockView::new(genesis);
                let header = block.header_view();
                let hash = block.hash();

                let details = BlockDetails {
                    number: header.number(),
                    total_score: header.score(),
                    parent: header.parent_hash(),
                    children: vec![],
                };

                let mut batch = DBTransaction::new();
                batch.put(db::COL_HEADERS, &hash, block.header_rlp().as_raw());
                batch.put(db::COL_BODIES, &hash, &Self::block_to_body(genesis));

                batch.write(db::COL_EXTRA, &hash, &details);
                batch.write(db::COL_EXTRA, &header.number(), &hash);

                batch.put(db::COL_EXTRA, b"best", &hash);
                bc.db.write(batch).expect("Low level database error. Some issue with disk?");
                hash
            }
        };
        {
            // Fetch best block details
            let best_block_number = bc.block_number(&best_block_hash).unwrap();
            let best_block_total_score = bc.block_details(&best_block_hash).unwrap().total_score;
            let best_block_rlp = bc.block(&best_block_hash).unwrap().into_inner();
            let best_block_timestamp = BlockView::new(&best_block_rlp).header().timestamp();

            // and write them
            let mut best_block = bc.best_block.write();
            *best_block = BestBlock {
                number: best_block_number,
                total_score: best_block_total_score,
                hash: best_block_hash,
                timestamp: best_block_timestamp,
                block: best_block_rlp,
            };
        }

        bc
    }

    /// Returns true if the given parent block has given child
    /// (though not necessarily a part of the canon chain).
    fn is_known_child(&self, parent: &H256, hash: &H256) -> bool {
        self.db.read_with_cache(db::COL_EXTRA, &self.block_details, parent).map_or(false, |d| d.children.contains(hash))
    }

    /// Returns a tree route between `from` and `to`, which is a tuple of:
    ///
    /// - a vector of hashes of all blocks, ordered from `from` to `to`.
    ///
    /// - common ancestor of these blocks.
    ///
    /// - an index where best common ancestor would be
    ///
    /// 1.) from newer to older
    ///
    /// - bc: `A1 -> A2 -> A3 -> A4 -> A5`
    /// - from: A5, to: A4
    /// - route:
    ///
    ///   ```json
    ///   { blocks: [A5], ancestor: A4, index: 1 }
    ///   ```
    ///
    /// 2.) from older to newer
    ///
    /// - bc: `A1 -> A2 -> A3 -> A4 -> A5`
    /// - from: A3, to: A4
    /// - route:
    ///
    ///   ```json
    ///   { blocks: [A4], ancestor: A3, index: 0 }
    ///   ```
    ///
    /// 3.) fork:
    ///
    /// - bc:
    ///
    ///   ```text
    ///   A1 -> A2 -> A3 -> A4
    ///              -> B3 -> B4
    ///   ```
    /// - from: B4, to: A4
    /// - route:
    ///
    ///   ```json
    ///   { blocks: [B4, B3, A3, A4], ancestor: A2, index: 2 }
    ///   ```
    ///
    /// If the tree route verges into pruned or unknown blocks,
    /// `None` is returned.
    pub fn tree_route(&self, from: H256, to: H256) -> Option<TreeRoute> {
        let mut from_branch = vec![];
        let mut to_branch = vec![];

        let mut from_details = self.block_details(&from)?;
        let mut to_details = self.block_details(&to)?;
        let mut current_from = from;
        let mut current_to = to;

        // reset from && to to the same level
        while from_details.number > to_details.number {
            from_branch.push(current_from);
            current_from = from_details.parent.clone();
            from_details = self.block_details(&from_details.parent)?;
        }

        while to_details.number > from_details.number {
            to_branch.push(current_to);
            current_to = to_details.parent.clone();
            to_details = self.block_details(&to_details.parent)?;
        }

        assert_eq!(from_details.number, to_details.number);

        // move to shared parent
        while current_from != current_to {
            from_branch.push(current_from);
            current_from = from_details.parent.clone();
            from_details = self.block_details(&from_details.parent)?;

            to_branch.push(current_to);
            current_to = to_details.parent.clone();
            to_details = self.block_details(&to_details.parent)?;
        }

        let index = from_branch.len();

        from_branch.extend(to_branch.into_iter().rev());

        Some(TreeRoute {
            blocks: from_branch,
            ancestor: current_from,
            index: index
        })
    }
    /// Inserts the block into backing cache database.
    /// Expects the block to be valid and already verified.
    /// If the block is already known, does nothing.
    pub fn insert_block(&self, batch: &mut DBTransaction, bytes: &[u8], invoices: Vec<Invoice>) -> ImportRoute {
        // create views onto rlp
        let block = BlockView::new(bytes);
        let header = block.header_view();
        let hash = header.hash();

        if self.is_known_child(&header.parent_hash(), &hash) {
            return ImportRoute::none();
        }

        assert!(self.pending_best_block.read().is_none());

        let compressed_header = compress(block.header_rlp().as_raw(), blocks_swapper());
        let compressed_body = compress(&Self::block_to_body(bytes), blocks_swapper());

        // store block in db
        batch.put(db::COL_HEADERS, &hash, &compressed_header);
        batch.put(db::COL_BODIES, &hash, &compressed_body);

        let info = self.block_info(&header);

        self.prepare_update(batch, ExtrasUpdate {
            block_hashes: self.prepare_block_hashes_update(bytes, &info),
            block_details: self.prepare_block_details_update(bytes, &info),
            block_invoices: self.prepare_block_invoices_update(invoices, &info),
            transactions_addresses: self.prepare_transaction_addresses_update(bytes, &info),
            info: info.clone(),
            timestamp: header.timestamp(),
            block: bytes,
        }, true);

        ImportRoute::from(info)
    }

    /// Apply pending insertion updates
    pub fn commit(&self) {
        let mut pending_best_block = self.pending_best_block.write();
        let mut pending_write_hashes = self.pending_block_hashes.write();
        let mut pending_block_details = self.pending_block_details.write();
        let mut pending_write_txs = self.pending_transaction_addresses.write();

        let mut best_block = self.best_block.write();
        let mut write_block_details = self.block_details.write();
        let mut write_hashes = self.block_hashes.write();
        let mut write_txs = self.transaction_addresses.write();
        // update best block
        if let Some(block) = pending_best_block.take() {
            *best_block = block;
        }

        let pending_txs = mem::replace(&mut *pending_write_txs, HashMap::new());
        let (retracted_txs, enacted_txs) = pending_txs.into_iter().partition::<HashMap<_, _>, _>(|&(_, ref value)| value.is_none());

        write_hashes.extend(mem::replace(&mut *pending_write_hashes, HashMap::new()));
        write_txs.extend(enacted_txs.into_iter().map(|(k, v)| (k, v.expect("Transactions were partitioned; qed"))));
        write_block_details.extend(mem::replace(&mut *pending_block_details, HashMap::new()));

        for hash in retracted_txs.keys() {
            write_txs.remove(hash);
        }
    }

    /// Prepares extras update.
    fn prepare_update(&self, batch: &mut DBTransaction, update: ExtrasUpdate, is_best: bool) {
        // These cached values must be updated last with all four locks taken to avoid
        // cache decoherence
        {
            let mut best_block = self.pending_best_block.write();
            if is_best && update.info.location != BlockLocation::Branch {
                batch.put(db::COL_EXTRA, b"best", &update.info.hash);
                *best_block = Some(BestBlock {
                    hash: update.info.hash,
                    number: update.info.number,
                    total_score: update.info.total_score,
                    timestamp: update.timestamp,
                    block: update.block.to_vec(),
                });
            }

            let mut write_hashes = self.pending_block_hashes.write();
            let mut write_details = self.pending_block_details.write();
            let mut write_txs = self.pending_transaction_addresses.write();

            batch.extend_with_cache(db::COL_EXTRA, &mut *write_details, update.block_details, CacheUpdatePolicy::Overwrite);
            batch.extend_with_cache(db::COL_EXTRA, &mut *write_hashes, update.block_hashes, CacheUpdatePolicy::Overwrite);
            batch.extend_with_option_cache(db::COL_EXTRA, &mut *write_txs, update.transactions_addresses, CacheUpdatePolicy::Overwrite);
        }
    }

    /// This function returns modified block hashes.
    fn prepare_block_hashes_update(&self, block_bytes: &[u8], info: &BlockInfo) -> HashMap<BlockNumber, H256> {
        let mut block_hashes = HashMap::new();
        let block = BlockView::new(block_bytes);
        let header = block.header_view();
        let number = header.number();

        match info.location {
            BlockLocation::Branch => (),
            BlockLocation::CanonChain => {
                block_hashes.insert(number, info.hash);
            },
            BlockLocation::BranchBecomingCanonChain(ref data) => {
                let ancestor_number = self.block_number(&data.ancestor).expect("Block number of ancestor is always in DB");
                let start_number = ancestor_number + 1;

                for (index, hash) in data.enacted.iter().cloned().enumerate() {
                    block_hashes.insert(start_number + index as BlockNumber, hash);
                }

                block_hashes.insert(number, info.hash);
            }
        }

        block_hashes
    }

    /// This function returns modified block details.
    /// Uses the given parent details or attempts to load them from the database.
    fn prepare_block_details_update(&self, block_bytes: &[u8], info: &BlockInfo) -> HashMap<H256, BlockDetails> {
        let block = BlockView::new(block_bytes);
        let header = block.header_view();
        let parent_hash = header.parent_hash();

        // update parent
        let mut parent_details = self.block_details(&parent_hash).unwrap_or_else(|| panic!("Invalid parent hash: {:?}", parent_hash));
        parent_details.children.push(info.hash);

        // create current block details.
        let details = BlockDetails {
            number: header.number(),
            total_score: info.total_score,
            parent: parent_hash,
            children: vec![],
        };

        // write to batch
        let mut block_details = HashMap::new();
        block_details.insert(parent_hash, parent_details);
        block_details.insert(info.hash, details);
        block_details
    }

    /// This function returns modified block invoices.
    fn prepare_block_invoices_update(&self, invoices: Vec<Invoice>, info: &BlockInfo) -> HashMap<H256, BlockInvoices> {
        let mut block_invoices = HashMap::new();
        block_invoices.insert(info.hash, BlockInvoices::new(invoices));
        block_invoices
    }

    /// This function returns modified transaction addresses.
    fn prepare_transaction_addresses_update(&self, block_bytes: &[u8], info: &BlockInfo) -> HashMap<H256, Option<TransactionAddress>> {
        let block = BlockView::new(block_bytes);
        let transaction_hashes = block.transaction_hashes();

        match info.location {
            BlockLocation::CanonChain => {
                transaction_hashes.into_iter()
                    .enumerate()
                    .map(|(i ,tx_hash)| {
                        (tx_hash, Some(TransactionAddress {
                            block_hash: info.hash,
                            index: i
                        }))
                    })
                    .collect()
            },
            BlockLocation::BranchBecomingCanonChain(ref data) => {
                let addresses = data.enacted.iter()
                    .flat_map(|hash| {
                        let body = self.block_body(hash).expect("Enacted block must be in database.");
                        let hashes = body.transaction_hashes();
                        hashes.into_iter()
                            .enumerate()
                            .map(|(i, tx_hash)| (tx_hash, Some(TransactionAddress {
                                block_hash: *hash,
                                index: i,
                            })))
                            .collect::<HashMap<H256, Option<TransactionAddress>>>()
                    });

                let current_addresses = transaction_hashes.into_iter()
                    .enumerate()
                    .map(|(i ,tx_hash)| {
                        (tx_hash, Some(TransactionAddress {
                            block_hash: info.hash,
                            index: i
                        }))
                    });

                let retracted = data.retracted.iter().flat_map(|hash| {
                    let body = self.block_body(hash).expect("Retracted block must be in database.");
                    let hashes = body.transaction_hashes();
                    hashes.into_iter().map(|hash| (hash, None)).collect::<HashMap<H256, Option<TransactionAddress>>>()
                });

                // The order here is important! Don't remove transaction if it was part of enacted blocks as well.
                retracted.chain(addresses).chain(current_addresses).collect()
            },
            BlockLocation::Branch => HashMap::new(),
        }
    }

    /// Get inserted block info which is critical to prepare extras updates.
    fn block_info(&self, header: &HeaderView) -> BlockInfo {
        let hash = header.hash();
        let number = header.number();
        let parent_hash = header.parent_hash();
        let parent_details = self.block_details(&parent_hash).unwrap_or_else(|| panic!("Invalid parent hash: {:?}", parent_hash));
        let is_new_best = parent_details.total_score + header.score() > self.best_block_total_score();

        BlockInfo {
            hash,
            number,
            total_score: parent_details.total_score + header.score(),
            location: if is_new_best {
                // on new best block we need to make sure that all ancestors
                // are moved to "canon chain"
                // find the route between old best block and the new one
                let best_hash = self.best_block_hash();
                let route = self.tree_route(best_hash, parent_hash)
                    .expect("blocks being imported always within recent history; qed");

                assert_eq!(number, parent_details.number + 1);

                match route.blocks.len() {
                    0 => BlockLocation::CanonChain,
                    _ => {
                        let retracted = route.blocks.iter().take(route.index).cloned().collect::<Vec<_>>().into_iter().collect::<Vec<_>>();
                        let enacted = route.blocks.into_iter().skip(route.index).collect::<Vec<_>>();
                        BlockLocation::BranchBecomingCanonChain(BranchBecomingCanonChainData {
                            ancestor: route.ancestor,
                            enacted,
                            retracted,
                        })
                    }
                }
            } else {
                BlockLocation::Branch
            }
        }
    }

    /// Returns general blockchain information
    pub fn chain_info(&self) -> BlockChainInfo {
        // ensure data consistently by locking everything first
        let best_block = self.best_block.read();
        BlockChainInfo {
            total_score: best_block.total_score.clone(),
            pending_total_score: best_block.total_score.clone(),
            genesis_hash: self.genesis_hash(),
            best_block_hash: best_block.hash,
            best_block_number: best_block.number,
            best_block_timestamp: best_block.timestamp,
        }
    }

    /// Create a block body from a block.
    pub fn block_to_body(block: &[u8]) -> Bytes {
        let mut body = RlpStream::new_list(1);
        let block_rlp = Rlp::new(block);
        body.append_raw(block_rlp.at(1).as_raw(), 1);
        body.out()
    }

    /// Get best block hash.
    pub fn best_block_hash(&self) -> H256 {
        self.best_block.read().hash
    }

    /// Get best block number.
    pub fn best_block_number(&self) -> BlockNumber {
        self.best_block.read().number
    }

    /// Get best block timestamp.
    pub fn best_block_timestamp(&self) -> u64 {
        self.best_block.read().timestamp
    }

    /// Get best block total score.
    pub fn best_block_total_score(&self) -> U256 {
        self.best_block.read().total_score
    }

    /// Get best block header
    pub fn best_block_header(&self) -> encoded::Header {
        let block = self.best_block.read();
        let raw = BlockView::new(&block.block).header_view().rlp().as_raw().to_vec();
        encoded::Header::new(raw)
    }
}

/// Interface for querying blocks by hash and by number.
pub trait BlockProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known(&self, hash: &H256) -> bool;

    /// Get raw block data
    fn block(&self, hash: &H256) -> Option<encoded::Block>;

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails>;

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256>;

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress>;

    /// Get invoices of block with given hash.
    fn block_invoices(&self, hash: &H256) -> Option<BlockInvoices>;

    /// Get the partial-header of a block.
    fn block_header(&self, hash: &H256) -> Option<Header> {
        self.block_header_data(hash).map(|header| header.decode())
    }

    /// Get the header RLP of a block.
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header>;

    /// Get the block body (uncles and transactions).
    fn block_body(&self, hash: &H256) -> Option<encoded::Body>;

    /// Get the number of given block's hash.
    fn block_number(&self, hash: &H256) -> Option<BlockNumber> {
        self.block_details(hash).map(|details| details.number)
    }

    /// Get transaction with given transaction hash.
    fn transaction(&self, address: &TransactionAddress) -> Option<LocalizedTransaction> {
        self.block_body(&address.block_hash)
            .and_then(|body| self.block_number(&address.block_hash)
                .and_then(|n| body.view().localized_transaction_at(&address.block_hash, n, address.index)))
    }

    /// Get a list of transactions for a given block.
    /// Returns None if block does not exist.
    fn transactions(&self, hash: &H256) -> Option<Vec<LocalizedTransaction>> {
        self.block_body(hash)
            .and_then(|body| self.block_number(hash)
                .map(|n| body.view().localized_transactions(hash, n)))
    }

    /// Returns reference to genesis hash.
    fn genesis_hash(&self) -> H256 {
        self.block_hash(0).expect("Genesis hash should always exist")
    }

    /// Returns the header of the genesis block.
    fn genesis_header(&self) -> Header {
        self.block_header(&self.genesis_hash())
            .expect("Genesis header always stored; qed")
    }
}

impl BlockProvider for BlockChain {
    fn is_known(&self, hash: &H256) -> bool {
        self.db.exists_with_cache(db::COL_EXTRA, &self.block_details, hash)
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

    /// Get block header data
    fn block_header_data(&self, hash: &H256) -> Option<encoded::Header> {
        // Check cache first
        {
            let read = self.block_headers.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Header::new(v.clone()));
            }
        }

        // Check if it's the best block
        {
            let best_block = self.best_block.read();
            if &best_block.hash == hash {
                return Some(encoded::Header::new(
                    Rlp::new(&best_block.block).at(0).as_raw().to_vec()
                ))
            }
        }

        // Read from DB and populate cache
        let b = self.db.get(db::COL_HEADERS, hash)
            .expect("Low level database error. Some issue with disk?")?;

        let bytes = decompress(&b, blocks_swapper()).into_vec();
        let mut write = self.block_headers.write();
        write.insert(*hash, bytes.clone());

        Some(encoded::Header::new(bytes))
    }

    /// Get block body data
    fn block_body(&self, hash: &H256) -> Option<encoded::Body> {
        // Check cache first
        {
            let read = self.block_bodies.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Body::new(v.clone()));
            }
        }

        // Check if it's the best block
        {
            let best_block = self.best_block.read();
            if &best_block.hash == hash {
                return Some(encoded::Body::new(Self::block_to_body(&best_block.block)));
            }
        }

        // Read from DB and populate cache
        let b = self.db.get(db::COL_BODIES, hash)
            .expect("Low level database error. Some issue with disk?")?;

        let bytes = decompress(&b, blocks_swapper()).into_vec();
        let mut write = self.block_bodies.write();
        write.insert(*hash, bytes.clone());

        Some(encoded::Body::new(bytes))
    }

    /// Get the familial details concerning a block.
    fn block_details(&self, hash: &H256) -> Option<BlockDetails> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.block_details, hash)?;
        Some(result)
    }

    /// Get the hash of given block's number.
    fn block_hash(&self, index: BlockNumber) -> Option<H256> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.block_hashes, &index)?;
        Some(result)
    }

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.transaction_addresses, hash)?;
        Some(result)
    }

    /// Get invoices of block with given hash.
    fn block_invoices(&self, hash: &H256) -> Option<BlockInvoices> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.block_invoices, hash)?;
        Some(result)
    }
}

/// Import route for newly inserted block.
#[derive(Debug, PartialEq)]
pub struct ImportRoute {
    /// Blocks that were invalidated by new block.
    pub retracted: Vec<H256>,
    /// Blocks that were validated by new block.
    pub enacted: Vec<H256>,
    /// Blocks which are neither retracted nor enacted.
    pub omitted: Vec<H256>,
}

impl ImportRoute {
    pub fn none() -> Self {
        ImportRoute {
            retracted: vec![],
            enacted: vec![],
            omitted: vec![],
        }
    }
}

impl From<BlockInfo> for ImportRoute {
    fn from(info: BlockInfo) -> ImportRoute {
        match info.location {
            BlockLocation::CanonChain => ImportRoute {
                retracted: vec![],
                enacted: vec![info.hash],
                omitted: vec![],
            },
            BlockLocation::Branch => ImportRoute {
                retracted: vec![],
                enacted: vec![],
                omitted: vec![info.hash],
            },
            BlockLocation::BranchBecomingCanonChain(mut data) => {
                data.enacted.push(info.hash);
                ImportRoute {
                    retracted: data.retracted,
                    enacted: data.enacted,
                    omitted: vec![],
                }
            }
        }
    }
}

/// Block extras update info.
pub struct ExtrasUpdate<'a> {
    /// Block info.
    pub info: BlockInfo,
    /// Block timestamp.
    pub timestamp: u64,
    /// Current block uncompressed rlp bytes
    pub block: &'a [u8],
    /// Modified block hashes.
    pub block_hashes: HashMap<BlockNumber, H256>,
    /// Modified block details.
    pub block_details: HashMap<H256, BlockDetails>,
    /// Modified block invoices.
    pub block_invoices: HashMap<H256, BlockInvoices>,
    /// Modified transaction addresses (None signifies removed transactions).
    pub transactions_addresses: HashMap<H256, Option<TransactionAddress>>,
}

/// Represents a tree route between `from` block and `to` block:
#[derive(Debug)]
pub struct TreeRoute {
    /// A vector of hashes of all blocks, ordered from `from` to `to`.
    pub blocks: Vec<H256>,
    /// Best common ancestor of these blocks.
    pub ancestor: H256,
    /// An index where best common ancestor would be.
    pub index: usize,
}
