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

use ctypes::transaction::{Error as TransactionError, Transaction};

use error::Error;

use super::super::invoice::Invoice;
use super::ShardBackend;

pub trait ShardState<B>
where
    B: ShardBackend, {
    fn apply(&mut self, transaction: &Transaction, parcel_network_id: &u64) -> Result<TransactionOutcome, Error>;
}

#[derive(Debug, PartialEq)]
pub struct TransactionOutcome {
    /// The invoice for the applied parcel.
    pub invoice: Invoice,
    /// The output of the applied parcel.
    pub error: Option<TransactionError>,
}
