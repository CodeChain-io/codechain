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

use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};
use serde::{Serialize, Serializer};

use super::super::transaction::Error;
use super::invoice_result::InvoiceResult;

/// Information describing execution of a parcel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionInvoice {
    Success,
    Fail(Error),
}

const INVOICE_ID_SUCCESS: u8 = 1u8;
const INVOICE_ID_FAIL: u8 = 2u8;

impl Serialize for TransactionInvoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        match self {
            TransactionInvoice::Success => serializer.serialize_str("Success"),
            TransactionInvoice::Fail(_err) => serializer.serialize_str("Failed"),
        }
    }
}

impl TransactionInvoice {
    pub fn result(&self) -> InvoiceResult {
        match self {
            TransactionInvoice::Success => InvoiceResult::Success,
            TransactionInvoice::Fail(_) => InvoiceResult::Failed,
        }
    }
}

impl Encodable for TransactionInvoice {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            TransactionInvoice::Success => {
                s.begin_list(1);
                s.append(&INVOICE_ID_SUCCESS);
            }
            TransactionInvoice::Fail(err) => {
                s.begin_list(2);
                s.append(&INVOICE_ID_FAIL);
                s.append(err);
            }
        };
    }
}

impl Decodable for TransactionInvoice {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(match rlp.val_at::<u8>(0)? {
            INVOICE_ID_SUCCESS => TransactionInvoice::Success,
            INVOICE_ID_FAIL => TransactionInvoice::Fail(rlp.val_at::<Error>(1)?),
            _ => return Err(DecoderError::Custom("Invalid parcel outcome")),
        })
    }
}
