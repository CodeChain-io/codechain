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
use std::sync::Arc;

use ctypes::H256;
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::RwLock;

use super::super::db::{self, CacheUpdatePolicy, Readable, Writable};
use super::super::invoice::Invoice;
use super::extras::{BlockInvoices, ParcelAddress};

/// Structure providing fast access to blockchain data.
///
/// **Does not do input data verification.**
pub struct InvoiceDB {
    invoice_cache: RwLock<HashMap<H256, BlockInvoices>>,

    db: Arc<KeyValueDB>,
}

impl InvoiceDB {
    /// Create new instance of blockchain from given Genesis.
    pub fn new(db: Arc<KeyValueDB>) -> Self {
        Self {
            invoice_cache: RwLock::new(HashMap::new()),

            db,
        }
    }

    /// Inserts the block into backing cache database.
    /// Expects the block to be valid and already verified.
    /// If the block is already known, does nothing.
    pub fn insert_invoice(&self, batch: &mut DBTransaction, hash: &H256, invoices: Vec<Invoice>) {
        if self.is_known_invoice(hash) {
            return
        }

        let mut invoice_map = HashMap::new();
        invoice_map.insert(*hash, BlockInvoices::new(invoices));

        let mut invoice_cache = self.invoice_cache.write();
        batch.extend_with_cache(db::COL_EXTRA, &mut *invoice_cache, invoice_map, CacheUpdatePolicy::Remove);
    }
}

/// Interface for querying invoices.
pub trait InvoiceProvider {
    /// Returns true if invoices for given hash is known
    fn is_known_invoice(&self, hash: &H256) -> bool;

    /// Get invoices of block with given hash.
    fn block_invoices(&self, hash: &H256) -> Option<BlockInvoices>;

    /// Get parcel invoice.
    fn parcel_invoice(&self, address: &ParcelAddress) -> Option<Invoice>;
}

impl InvoiceProvider for InvoiceDB {
    fn is_known_invoice(&self, hash: &H256) -> bool {
        self.db.exists_with_cache(db::COL_EXTRA, &self.invoice_cache, hash)
    }

    /// Get invoices of block with given hash.
    fn block_invoices(&self, hash: &H256) -> Option<BlockInvoices> {
        let result = self.db.read_with_cache(db::COL_EXTRA, &self.invoice_cache, hash)?;
        Some(result)
    }

    /// Get parcel invoice.
    fn parcel_invoice(&self, address: &ParcelAddress) -> Option<Invoice> {
        self.block_invoices(&address.block_hash).and_then(|bi| bi.invoices.into_iter().nth(address.index))
    }
}
