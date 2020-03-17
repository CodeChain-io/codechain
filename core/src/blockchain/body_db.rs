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

use super::block_info::BestBlockChanged;
use super::extras::{TransactionAddress, TransactionAddresses};
use crate::db::{self, CacheUpdatePolicy, Readable, Writable};
use crate::views::BlockView;
use crate::{encoded, UnverifiedTransaction};
use ctypes::{BlockHash, Tracker, TxHash};
use kvdb::{DBTransaction, KeyValueDB};
use lru_cache::LruCache;
use parking_lot::{Mutex, RwLock};
use primitives::Bytes;
use rlp::RlpStream;
use rlp_compress::{blocks_swapper, compress, decompress};
use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::Arc;

const BODY_CACHE_SIZE: usize = 1000;

pub struct BodyDB {
    // block cache
    body_cache: Mutex<LruCache<BlockHash, Bytes>>,
    address_by_hash_cache: RwLock<HashMap<TxHash, TransactionAddress>>,
    pending_addresses_by_hash: RwLock<HashMap<TxHash, Option<TransactionAddress>>>,

    addresses_by_tracker_cache: Mutex<HashMap<Tracker, TransactionAddresses>>,
    pending_addresses_by_tracker: Mutex<HashMap<Tracker, Option<TransactionAddresses>>>,

    db: Arc<dyn KeyValueDB>,
}

type TrackerAndAddress = (Tracker, TransactionAddresses);

impl BodyDB {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &BlockView, db: Arc<dyn KeyValueDB>) -> Self {
        let bdb = Self {
            body_cache: Mutex::new(LruCache::new(BODY_CACHE_SIZE)),
            address_by_hash_cache: RwLock::new(HashMap::new()),
            pending_addresses_by_hash: RwLock::new(HashMap::new()),

            addresses_by_tracker_cache: Default::default(),
            pending_addresses_by_tracker: Default::default(),

            db,
        };

        let genesis_hash = genesis.hash();
        if bdb.block_body(&genesis_hash).is_none() {
            let mut batch = DBTransaction::new();
            batch.put(db::COL_BODIES, &genesis_hash, &Self::block_to_body(genesis));

            bdb.db.write(batch).expect("Low level database error. Some issue with disk?");
        }

        bdb
    }

    /// Inserts the block body into backing cache database.
    /// Expects the body to be valid and already verified.
    /// If the body is already known, does nothing.
    pub fn insert_body(&self, batch: &mut DBTransaction, block: &BlockView) {
        let hash = block.hash();

        if self.is_known_body(&hash) {
            return
        }

        let compressed_body = compress(&Self::block_to_body(block), blocks_swapper());

        // store block in db
        batch.put(db::COL_BODIES, &hash, &compressed_body);
    }

    pub fn update_best_block(&self, batch: &mut DBTransaction, best_block_changed: &BestBlockChanged) {
        let mut pending_addresses_by_hash = self.pending_addresses_by_hash.write();
        let mut pending_addresses_by_tracker = self.pending_addresses_by_tracker.lock();
        batch.extend_with_option_cache(
            db::COL_EXTRA,
            &mut *pending_addresses_by_hash,
            self.new_transaction_address_entries(best_block_changed),
            CacheUpdatePolicy::Overwrite,
        );
        batch.extend_with_option_cache(
            db::COL_EXTRA,
            &mut *pending_addresses_by_tracker,
            self.new_transaction_addresses_entries(best_block_changed),
            CacheUpdatePolicy::Overwrite,
        );
    }

    /// Apply pending insertion updates
    pub fn commit(&self) {
        let mut address_by_hash_cache = self.address_by_hash_cache.write();
        let mut pending_addresses_by_hash = self.pending_addresses_by_hash.write();

        let mut addresses_by_tracker_cache = self.addresses_by_tracker_cache.lock();
        let mut pending_addresses_by_tracker = self.pending_addresses_by_tracker.lock();

        let new_txs_by_hash = mem::replace(&mut *pending_addresses_by_hash, HashMap::new());
        let (retracted_txs, enacted_txs) =
            new_txs_by_hash.into_iter().partition::<HashMap<_, _>, _>(|&(_, ref value)| value.is_none());

        address_by_hash_cache
            .extend(enacted_txs.into_iter().map(|(k, v)| (k, v.expect("Transactions were partitioned; qed"))));

        for hash in retracted_txs.keys() {
            address_by_hash_cache.remove(hash);
        }

        let new_txs_by_tracker = mem::replace(&mut *pending_addresses_by_tracker, HashMap::new());
        let (removed_transactions, added_transactions) =
            new_txs_by_tracker.into_iter().partition::<HashMap<_, _>, _>(|&(_, ref value)| value.is_none());

        addresses_by_tracker_cache
            .extend(added_transactions.into_iter().map(|(k, v)| (k, v.expect("Transactions were partitioned; qed"))));

        for hash in removed_transactions.keys() {
            addresses_by_tracker_cache.remove(hash);
        }
    }

    /// This function returns modified transaction addresses.
    fn new_transaction_address_entries(
        &self,
        best_block_changed: &BestBlockChanged,
    ) -> HashMap<TxHash, Option<TransactionAddress>> {
        let block_hash = if let Some(best_block_hash) = best_block_changed.new_best_hash() {
            best_block_hash
        } else {
            return HashMap::new()
        };
        let block = match best_block_changed.best_block() {
            Some(block) => block,
            None => return HashMap::new(),
        };
        let tx_hashes = block.transaction_hashes();

        match best_block_changed {
            BestBlockChanged::CanonChainAppended {
                ..
            } => tx_hash_and_address_entries(best_block_changed.new_best_hash().unwrap(), tx_hashes).collect(),
            BestBlockChanged::BranchBecomingCanonChain {
                tree_route,
                ..
            } => {
                let enacted = tree_route.enacted.iter().flat_map(|hash| {
                    let body = self.block_body(hash).expect("Enacted block must be in database.");
                    let enacted_tx_hashes = body.transaction_hashes();
                    tx_hash_and_address_entries(*hash, enacted_tx_hashes)
                });

                let current_addresses = { tx_hash_and_address_entries(block_hash, tx_hashes) };

                let retracted = tree_route.retracted.iter().flat_map(|hash| {
                    let body = self.block_body(&hash).expect("Retracted block must be in database.");
                    let retracted_tx_hashes = body.transaction_hashes().into_iter();
                    retracted_tx_hashes.map(|hash| (hash, None))
                });

                // The order here is important! Don't remove transactions if it was part of enacted blocks as well.
                retracted.chain(enacted).chain(current_addresses).collect()
            }
            BestBlockChanged::None => HashMap::new(),
        }
    }

    fn new_transaction_addresses_entries(
        &self,
        best_block_changed: &BestBlockChanged,
    ) -> HashMap<Tracker, Option<TransactionAddresses>> {
        let block_hash = if let Some(best_block_hash) = best_block_changed.new_best_hash() {
            best_block_hash
        } else {
            return HashMap::new()
        };
        let block = match best_block_changed.best_block() {
            Some(block) => block,
            None => return HashMap::new(),
        };

        let (removed, added): (
            Box<dyn Iterator<Item = TrackerAndAddress>>,
            Box<dyn Iterator<Item = TrackerAndAddress>>,
        ) = match best_block_changed {
            BestBlockChanged::CanonChainAppended {
                ..
            } => (
                Box::new(::std::iter::empty()),
                Box::new(tracker_and_addresses_entries(block_hash, block.transactions())),
            ),
            BestBlockChanged::BranchBecomingCanonChain {
                ref tree_route,
                ..
            } => {
                let enacted = tree_route
                    .enacted
                    .iter()
                    .flat_map(|hash| {
                        let body = self.block_body(hash).expect("Enacted block must be in database.");
                        tracker_and_addresses_entries(*hash, body.transactions())
                    })
                    .chain(tracker_and_addresses_entries(block_hash, block.transactions()));

                let retracted = tree_route.retracted.iter().flat_map(|hash| {
                    let body = self.block_body(hash).expect("Retracted block must be in database.");
                    tracker_and_addresses_entries(*hash, body.transactions())
                });

                (Box::new(retracted), Box::new(enacted))
            }
            BestBlockChanged::None => return Default::default(),
        };

        let mut added_addresses: HashMap<Tracker, TransactionAddresses> = Default::default();
        let mut removed_addresses: HashMap<Tracker, TransactionAddresses> = Default::default();
        let mut trackers: HashSet<Tracker> = Default::default();
        for (tracker, address) in added {
            trackers.insert(tracker);
            *added_addresses.entry(tracker).or_insert_with(Default::default) += address;
        }
        for (tracker, address) in removed {
            trackers.insert(tracker);
            *removed_addresses.entry(tracker).or_insert_with(Default::default) += address;
        }
        let mut inserted_address: HashMap<Tracker, TransactionAddresses> = Default::default();
        for tracker in trackers.into_iter() {
            let address: TransactionAddresses = self.db.read(db::COL_EXTRA, &tracker).unwrap_or_default();
            inserted_address.insert(tracker, address);
        }

        for (tracker, removed_address) in removed_addresses.into_iter() {
            *inserted_address
                .get_mut(&tracker)
                .expect("inserted addresses are sum of added_addresses and removed_addresses") -= removed_address;
        }
        for (tracker, added_address) in added_addresses.into_iter() {
            *inserted_address
                .get_mut(&tracker)
                .expect("inserted addresses are sum of added_addresses and removed_addresses") += added_address;
        }

        inserted_address
            .into_iter()
            .map(|(hash, address)| {
                if address.is_empty() {
                    (hash, None)
                } else {
                    (hash, Some(address))
                }
            })
            .collect()
    }

    /// Create a block body from a block.
    pub fn block_to_body(block: &BlockView) -> Bytes {
        let mut body = RlpStream::new_list(1);
        body.append_raw(block.rlp().at(1).unwrap().as_raw(), 1);
        body.out()
    }
}

/// Interface for querying block bodiess by hash and by number.
pub trait BodyProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known_body(&self, hash: &BlockHash) -> bool;

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &TxHash) -> Option<TransactionAddress>;

    fn transaction_address_by_tracker(&self, tracker: &Tracker) -> Option<TransactionAddress>;

    /// Get the block body (transactions).
    fn block_body(&self, hash: &BlockHash) -> Option<encoded::Body>;
}

impl BodyProvider for BodyDB {
    fn is_known_body(&self, hash: &BlockHash) -> bool {
        self.block_body(hash).is_some()
    }

    /// Get the address of transaction with given hash.
    fn transaction_address(&self, hash: &TxHash) -> Option<TransactionAddress> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &mut *self.address_by_hash_cache.write(), hash)?;
        Some(result)
    }

    fn transaction_address_by_tracker(&self, tracker: &Tracker) -> Option<TransactionAddress> {
        let addresses =
            self.db.read_with_cache(db::COL_EXTRA, &mut *self.addresses_by_tracker_cache.lock(), tracker)?;
        addresses.into_iter().next()
    }

    /// Get block body data
    fn block_body(&self, hash: &BlockHash) -> Option<encoded::Body> {
        // Check cache first
        {
            let mut lock = self.body_cache.lock();
            if let Some(v) = lock.get_mut(hash) {
                return Some(encoded::Body::new(v.clone()))
            }
        }

        // Read from DB and populate cache
        let compressed_body =
            self.db.get(db::COL_BODIES, hash).expect("Low level database error. Some issue with disk?")?;

        let raw_body = decompress(&compressed_body, blocks_swapper());
        let mut lock = self.body_cache.lock();
        lock.insert(*hash, raw_body.clone());

        Some(encoded::Body::new(raw_body))
    }
}

fn tx_hash_and_address_entries(
    block_hash: BlockHash,
    tx_hashes: impl IntoIterator<Item = TxHash>,
) -> impl Iterator<Item = (TxHash, Option<TransactionAddress>)> {
    tx_hashes.into_iter().enumerate().map(move |(index, tx_hash)| {
        (
            tx_hash,
            Some(TransactionAddress {
                block_hash,
                index,
            }),
        )
    })
}

fn tracker_and_addresses_entries(
    block_hash: BlockHash,
    transactions: impl IntoIterator<Item = UnverifiedTransaction>,
) -> impl Iterator<Item = TrackerAndAddress> {
    transactions.into_iter().enumerate().filter_map(move |(index, tx)| {
        tx.tracker().map(|tracker| {
            (
                tracker,
                TransactionAddresses::new(TransactionAddress {
                    block_hash,
                    index,
                }),
            )
        })
    })
}
