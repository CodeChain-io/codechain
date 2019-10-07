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

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;
use primitives::{H256, H264};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::db::{self, CacheUpdatePolicy, Key, Readable, Writable};

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct InvoiceDB {
    // tracker -> transaction hashe + error hint
    tracker_cache: RwLock<HashMap<H256, TrackerInvoices>>,
    // transaction hash -> error hint
    hash_cache: RwLock<HashMap<H256, Option<String>>>,

    db: Arc<dyn KeyValueDB>,
}

impl InvoiceDB {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(db: Arc<dyn KeyValueDB>) -> Self {
        Self {
            tracker_cache: Default::default(),
            hash_cache: Default::default(),

            db,
        }
    }

    /// Inserts the block into backing cache database.
    /// Expects the block to be valid and already verified.
    /// If the block is already known, does nothing.
    pub fn insert_invoice(
        &self,
        batch: &mut DBTransaction,
        hash: H256,
        tracker: Option<H256>,
        error_hint: Option<String>,
    ) {
        if self.is_known_error_hint(&hash) {
            return
        }

        let mut hashes_cache = self.tracker_cache.write();
        let mut hint_cache = self.hash_cache.write();

        if let Some(tracker) = tracker {
            let mut hashes =
                self.db.read_with_cache(db::COL_ERROR_HINT, &mut *hashes_cache, &tracker).unwrap_or_default();
            hashes.push((hash, error_hint.clone()));
            batch.write_with_cache(db::COL_ERROR_HINT, &mut *hashes_cache, tracker, hashes, CacheUpdatePolicy::Remove)
        }

        batch.write_with_cache(db::COL_ERROR_HINT, &mut *hint_cache, hash, error_hint, CacheUpdatePolicy::Remove);
    }
}

/// Interface for querying invoices.
pub trait InvoiceProvider {
    /// Returns true if invoices for given hash is known
    fn is_known_error_hint(&self, hash: &H256) -> bool;

    /// Get error hints
    fn error_hints_by_tracker(&self, tracker: &H256) -> Vec<(H256, Option<String>)>;

    /// Get error hint
    fn error_hint(&self, hash: &H256) -> Option<String>;
}

impl InvoiceProvider for InvoiceDB {
    fn is_known_error_hint(&self, hash: &H256) -> bool {
        self.db.exists_with_cache(db::COL_ERROR_HINT, &self.hash_cache, hash)
    }

    fn error_hints_by_tracker(&self, tracker: &H256) -> Vec<(H256, Option<String>)> {
        self.db
            .read_with_cache(db::COL_ERROR_HINT, &mut *self.tracker_cache.write(), tracker)
            .map(|hashes| (*hashes).clone())
            .unwrap_or_default()
    }

    fn error_hint(&self, hash: &H256) -> Option<String> {
        self.db.read_with_cache(db::COL_ERROR_HINT, &mut *self.hash_cache.write(), hash)?
    }
}

#[derive(Clone, Default)]
pub struct TrackerInvoices(Vec<(H256, Option<String>)>);

impl Encodable for TrackerInvoices {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.len() * 2);
        for (hash, error) in self.iter() {
            s.append(hash);
            s.append(error);
        }
    }
}

impl Decodable for TrackerInvoices {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if item_count % 2 == 1 {
            return Err(DecoderError::RlpInvalidLength {
                expected: item_count + 1,
                got: item_count,
            })
        }
        let mut vec = Vec::with_capacity(item_count / 2);
        // TODO: Optimzie the below code
        for i in 0..(item_count / 2) {
            vec.push((rlp.val_at(i * 2)?, rlp.val_at(i * 2 + 1)?));
        }
        Ok(vec.into())
    }
}

impl From<Vec<(H256, Option<String>)>> for TrackerInvoices {
    fn from(f: Vec<(H256, Option<String>)>) -> Self {
        TrackerInvoices(f)
    }
}

impl Deref for TrackerInvoices {
    type Target = Vec<(H256, Option<String>)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TrackerInvoices {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

enum ErrorHintIndex {
    TrackerToHashes = 0,
    HashToHint = 1,
}

impl From<ErrorHintIndex> for u8 {
    fn from(e: ErrorHintIndex) -> Self {
        e as Self
    }
}

impl Key<Option<String>> for H256 {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ErrorHintIndex::HashToHint)
    }
}

impl Key<TrackerInvoices> for H256 {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ErrorHintIndex::TrackerToHashes)
    }
}

fn with_index(hash: &H256, i: ErrorHintIndex) -> H264 {
    let mut result = H264::default();
    result[0] = i as u8;
    (*result)[1..].copy_from_slice(hash);
    result
}
