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

use ctypes::parcel::Action;
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;
use primitives::{Bytes, H256};
use rlp::RlpStream;
use rlp_compress::{blocks_swapper, compress, decompress};

use super::super::db::{self, CacheUpdatePolicy, Readable, Writable};
use super::super::encoded;
use super::super::views::BlockView;
use super::block_info::BlockLocation;
use super::extras::{ParcelAddress, TransactionAddress};

pub struct BodyDB {
    // block cache
    body_cache: RwLock<HashMap<H256, Bytes>>,
    parcel_address_cache: RwLock<HashMap<H256, ParcelAddress>>,
    pending_parcel_addresses: RwLock<HashMap<H256, Option<ParcelAddress>>>,

    transaction_address_cache: RwLock<HashMap<H256, TransactionAddress>>,
    pending_transaction_addresses: RwLock<HashMap<H256, Option<TransactionAddress>>>,

    db: Arc<KeyValueDB>,
}

impl BodyDB {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(genesis: &BlockView, db: Arc<KeyValueDB>) -> Self {
        let bdb = Self {
            body_cache: RwLock::new(HashMap::new()),
            parcel_address_cache: RwLock::new(HashMap::new()),
            pending_parcel_addresses: RwLock::new(HashMap::new()),

            transaction_address_cache: RwLock::new(HashMap::new()),
            pending_transaction_addresses: RwLock::new(HashMap::new()),

            db,
        };

        let genesis_hash = genesis.hash();
        match bdb.block_body(&genesis_hash) {
            None => {
                let mut batch = DBTransaction::new();
                batch.put(db::COL_BODIES, &genesis_hash, &Self::block_to_body(genesis));

                bdb.db.write(batch).expect("Low level database error. Some issue with disk?");
            }
            _ => {}
        };

        bdb
    }

    /// Inserts the block body into backing cache database.
    /// Expects the body to be valid and already verified.
    /// If the body is already known, does nothing.
    pub fn insert_body(&self, batch: &mut DBTransaction, block: &BlockView, location: &BlockLocation) {
        let hash = block.hash();

        if self.is_known_body(&hash) {
            return
        }

        let compressed_body = compress(&Self::block_to_body(block), blocks_swapper());

        // store block in db
        batch.put(db::COL_BODIES, &hash, &compressed_body);

        let mut pending_parcel_addresses = self.pending_parcel_addresses.write();
        let mut pending_transaction_addresses = self.pending_transaction_addresses.write();

        batch.extend_with_option_cache(
            db::COL_EXTRA,
            &mut *pending_parcel_addresses,
            self.new_parcel_address_entries(block, location),
            CacheUpdatePolicy::Overwrite,
        );
        batch.extend_with_option_cache(
            db::COL_EXTRA,
            &mut *pending_transaction_addresses,
            self.new_transaction_address_entries(block, location),
            CacheUpdatePolicy::Overwrite,
        );
    }

    /// Apply pending insertion updates
    pub fn commit(&self) {
        let mut parcel_address_cache = self.parcel_address_cache.write();
        let mut pending_parcel_addresses = self.pending_parcel_addresses.write();

        let mut transaction_address_cache = self.transaction_address_cache.write();
        let mut pending_transaction_addresses = self.pending_transaction_addresses.write();

        let new_parcels = mem::replace(&mut *pending_parcel_addresses, HashMap::new());
        let (retracted_parcels, enacted_parcels) =
            new_parcels.into_iter().partition::<HashMap<_, _>, _>(|&(_, ref value)| value.is_none());

        parcel_address_cache
            .extend(enacted_parcels.into_iter().map(|(k, v)| (k, v.expect("Parcels were partitioned; qed"))));

        for hash in retracted_parcels.keys() {
            parcel_address_cache.remove(hash);
        }

        let new_transactions = mem::replace(&mut *pending_transaction_addresses, HashMap::new());
        let (retracted_transactions, enacted_transactions) =
            new_transactions.into_iter().partition::<HashMap<_, _>, _>(|&(_, ref value)| value.is_none());

        transaction_address_cache
            .extend(enacted_transactions.into_iter().map(|(k, v)| (k, v.expect("Parcels were partitioned; qed"))));

        for hash in retracted_transactions.keys() {
            transaction_address_cache.remove(hash);
        }
    }

    /// This function returns modified parcel addresses.
    fn new_parcel_address_entries(
        &self,
        block: &BlockView,
        location: &BlockLocation,
    ) -> HashMap<H256, Option<ParcelAddress>> {
        let parcel_hashes = block.parcel_hashes();

        match location {
            BlockLocation::CanonChain => parcel_hashes
                .into_iter()
                .enumerate()
                .map(|(i, parcel_hash)| {
                    (
                        parcel_hash,
                        Some(ParcelAddress {
                            block_hash: block.hash(),
                            index: i,
                        }),
                    )
                })
                .collect(),
            BlockLocation::BranchBecomingCanonChain(ref data) => {
                let addresses = data.enacted.iter().flat_map(|hash| {
                    let body = self.block_body(hash).expect("Enacted block must be in database.");
                    let hashes = body.parcel_hashes();
                    hashes
                        .into_iter()
                        .enumerate()
                        .map(|(i, parcel_hash)| {
                            (
                                parcel_hash,
                                Some(ParcelAddress {
                                    block_hash: *hash,
                                    index: i,
                                }),
                            )
                        })
                        .collect::<HashMap<H256, Option<ParcelAddress>>>()
                });

                let current_addresses = parcel_hashes.into_iter().enumerate().map(|(i, parcel_hash)| {
                    (
                        parcel_hash,
                        Some(ParcelAddress {
                            block_hash: block.hash(),
                            index: i,
                        }),
                    )
                });

                let retracted = data.retracted.iter().flat_map(|hash| {
                    let body = self.block_body(hash).expect("Retracted block must be in database.");
                    let hashes = body.parcel_hashes();
                    hashes.into_iter().map(|hash| (hash, None)).collect::<HashMap<H256, Option<ParcelAddress>>>()
                });

                // The order here is important! Don't remove parcel if it was part of enacted blocks as well.
                retracted.chain(addresses).chain(current_addresses).collect()
            }
            BlockLocation::Branch => HashMap::new(),
        }
    }

    fn new_transaction_address_entries(
        &self,
        block: &BlockView,
        location: &BlockLocation,
    ) -> HashMap<H256, Option<TransactionAddress>> {
        match location {
            BlockLocation::CanonChain => block
                .parcels()
                .into_iter()
                .enumerate()
                .flat_map(|(parcel_index, parcel)| {
                    match &parcel.action {
                        Action::AssetTransactionGroup {
                            transactions,
                            ..
                        } => Some(transactions),
                        _ => None,
                    }.iter()
                        .flat_map(|transactions| transactions.iter())
                        .enumerate()
                        .map(|(index, transaction)| {
                            let parcel_address = ParcelAddress {
                                block_hash: block.hash(),
                                index: parcel_index,
                            };
                            (
                                transaction.hash(),
                                Some(TransactionAddress {
                                    parcel_address,
                                    index,
                                }),
                            )
                        })
                        .collect::<Vec<_>>() // FIXME: Find a way to remove collect.
                })
                .collect(),
            BlockLocation::BranchBecomingCanonChain(ref data) => {
                let addresses = data.enacted.iter().flat_map(|hash| {
                    let body = self.block_body(hash).expect("Enacted block must be in database.");
                    body.parcels()
                        .into_iter()
                        .enumerate()
                        .flat_map(|(parcel_index, parcel)| {
                            match &parcel.action {
                                Action::AssetTransactionGroup {
                                    transactions,
                                    ..
                                } => Some(transactions),
                                _ => None,
                            }.iter()
                                .flat_map(|transactions| transactions.iter())
                                .enumerate()
                                .map(|(index, transaction)| {
                                    let parcel_address = ParcelAddress {
                                        block_hash: *hash,
                                        index: parcel_index,
                                    };
                                    (
                                        transaction.hash(),
                                        Some(TransactionAddress {
                                            parcel_address,
                                            index,
                                        }),
                                    )
                                })
                                .collect::<Vec<_>>() // FIXME: Find a way to remove collect
                        })
                        .collect::<Vec<_>>()
                });

                let current_addresses = block.parcels().into_iter().enumerate().flat_map(|(parcel_index, parcel)| {
                    match &parcel.action {
                        Action::AssetTransactionGroup {
                            transactions,
                            ..
                        } => Some(transactions),
                        _ => None,
                    }.iter()
                        .flat_map(|transactions| transactions.iter())
                        .enumerate()
                        .map(|(index, transaction)| {
                            let parcel_address = ParcelAddress {
                                block_hash: block.hash(),
                                index: parcel_index,
                            };
                            (
                                transaction.hash(),
                                Some(TransactionAddress {
                                    parcel_address,
                                    index,
                                }),
                            )
                        })
                        .collect::<Vec<_>>() // FIXME: Find a way to remove collect
                });

                let retracted = data.retracted.iter().flat_map(|hash| {
                    let body = self.block_body(hash).expect("Retracted block must be in database.");
                    body.parcels()
                        .into_iter()
                        .map(|parcel| (*parcel).clone())
                        .filter_map(|parcel| match parcel.action {
                            Action::AssetTransactionGroup {
                                transactions,
                                ..
                            } => Some(transactions),
                            _ => None,
                        })
                        .flat_map(|transactions| transactions.into_iter().map(|transaction| (transaction.hash(), None)))
                });

                // The order here is important! Don't remove parcel if it was part of enacted blocks as well.
                retracted.chain(addresses).chain(current_addresses).collect()
            }
            BlockLocation::Branch => HashMap::new(),
        }
    }

    /// Create a block body from a block.
    pub fn block_to_body(block: &BlockView) -> Bytes {
        let mut body = RlpStream::new_list(1);
        body.append_raw(block.rlp().at(1).as_raw(), 1);
        body.out()
    }
}

/// Interface for querying block bodiess by hash and by number.
pub trait BodyProvider {
    /// Returns true if the given block is known
    /// (though not necessarily a part of the canon chain).
    fn is_known_body(&self, hash: &H256) -> bool;

    /// Get the address of parcel with given hash.
    fn parcel_address(&self, hash: &H256) -> Option<ParcelAddress>;

    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress>;

    /// Get the block body (uncles and parcels).
    fn block_body(&self, hash: &H256) -> Option<encoded::Body>;
}

impl BodyProvider for BodyDB {
    fn is_known_body(&self, hash: &H256) -> bool {
        self.block_body(hash).is_some()
    }

    /// Get the address of parcel with given hash.
    fn parcel_address(&self, hash: &H256) -> Option<ParcelAddress> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.parcel_address_cache, hash)?;
        Some(result)
    }

    fn transaction_address(&self, hash: &H256) -> Option<TransactionAddress> {
        Some(self.db.read_with_cache(db::COL_EXTRA, &self.transaction_address_cache, hash)?)
    }

    /// Get block body data
    fn block_body(&self, hash: &H256) -> Option<encoded::Body> {
        // Check cache first
        {
            let read = self.body_cache.read();
            if let Some(v) = read.get(hash) {
                return Some(encoded::Body::new(v.clone()))
            }
        }

        // Read from DB and populate cache
        let compressed_body =
            self.db.get(db::COL_BODIES, hash).expect("Low level database error. Some issue with disk?")?;

        let raw_body = decompress(&compressed_body, blocks_swapper()).into_vec();
        let mut write = self.body_cache.write();
        write.insert(*hash, raw_body.clone());

        Some(encoded::Body::new(raw_body))
    }
}
