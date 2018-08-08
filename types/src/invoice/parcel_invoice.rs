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
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

use super::super::parcel::Error;
use super::invoice_result::InvoiceResult;
use super::transaction_invoice::TransactionInvoice;

#[derive(Clone, Debug, PartialEq)]
pub enum ParcelInvoice {
    SingleSuccess,
    SingleFail(Error),
    Multiple(Vec<TransactionInvoice>),
}

const INVOICE_ID_SINGLE_SUCCESS: u8 = 1u8;
const INVOICE_ID_SINGLE_FAIL: u8 = 2u8;
const INVOICE_ID_MULTIPLE: u8 = 3u8;

impl Serialize for ParcelInvoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        match self {
            ParcelInvoice::SingleSuccess => serializer.serialize_str("Success"),
            ParcelInvoice::SingleFail(ref _err) => serializer.serialize_str("Failed"),
            ParcelInvoice::Multiple(transaction_invoices) => {
                let mut s = serializer.serialize_seq(Some(transaction_invoices.len()))?;
                for transaction_invoice in transaction_invoices {
                    s.serialize_element(transaction_invoice)?;
                }
                s.end()
            }
        }
    }
}

impl ParcelInvoice {
    pub fn new(invoices: Vec<TransactionInvoice>) -> Self {
        ParcelInvoice::Multiple(invoices)
    }

    pub fn iter_result<'a>(&'a self) -> Box<::std::iter::Iterator<Item = InvoiceResult> + 'a> {
        match self {
            ParcelInvoice::SingleSuccess => Box::new(::std::iter::once(InvoiceResult::Success)),
            ParcelInvoice::SingleFail(_) => Box::new(::std::iter::once(InvoiceResult::Failed)),
            ParcelInvoice::Multiple(invoices) => Box::new(invoices.iter().map(|invoice| invoice.result())),
        }
    }
}

impl Encodable for ParcelInvoice {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            ParcelInvoice::SingleSuccess => {
                s.begin_list(1);
                s.append(&INVOICE_ID_SINGLE_SUCCESS);
            }
            ParcelInvoice::SingleFail(err) => {
                s.begin_list(2);
                s.append(&INVOICE_ID_SINGLE_FAIL);
                s.append(err);
            }
            ParcelInvoice::Multiple(invoices) => {
                s.begin_list(2);
                s.append(&INVOICE_ID_MULTIPLE);
                s.append_list(invoices);
            }
        }
    }
}

impl Decodable for ParcelInvoice {
    fn decode(rlp: &UntrustedRlp) -> Result<ParcelInvoice, DecoderError> {
        match rlp.val_at::<u8>(0)? {
            INVOICE_ID_SINGLE_SUCCESS => Ok(ParcelInvoice::SingleSuccess),
            INVOICE_ID_SINGLE_FAIL => Ok(ParcelInvoice::SingleFail(rlp.val_at(1)?)),
            INVOICE_ID_MULTIPLE => Ok(ParcelInvoice::Multiple(rlp.at(1)?.as_list()?)),
            _ => Err(DecoderError::Custom("Unknown parcel invoice")),
        }
    }
}

impl From<Vec<TransactionInvoice>> for ParcelInvoice {
    fn from(invoices: Vec<TransactionInvoice>) -> Self {
        ParcelInvoice::Multiple(invoices)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::transaction::Error as TransactionError;

    use super::*;

    #[test]
    fn rlp_encode_and_decode_parcel_invoice() {
        let invoices = vec![
            TransactionInvoice::Success,
            TransactionInvoice::Success,
            TransactionInvoice::Fail(TransactionError::InvalidScript),
            TransactionInvoice::Success,
            TransactionInvoice::Success,
            TransactionInvoice::Success,
        ];
        rlp_encode_and_decode_test!(ParcelInvoice::new(invoices));
    }

    #[test]
    fn encode_and_decode_single_success_parcel_invoice() {
        rlp_encode_and_decode_test!(ParcelInvoice::SingleSuccess);
    }

    #[test]
    fn encode_and_decode_single_failed_parcel_invoice() {
        rlp_encode_and_decode_test!(ParcelInvoice::SingleFail(Error::Old));
    }

    #[test]
    fn encode_and_decode_empty_multiple_parcel_invoice() {
        rlp_encode_and_decode_test!(ParcelInvoice::Multiple(vec![]));
    }

    #[test]
    fn encode_and_decode_multiple_parcel_invoice_with_success() {
        rlp_encode_and_decode_test!(ParcelInvoice::Multiple(vec![TransactionInvoice::Success]));
    }

    #[test]
    fn encode_and_decode_multiple_parcel_invoice_with_failed() {
        rlp_encode_and_decode_test!(ParcelInvoice::Multiple(vec![TransactionInvoice::Fail(
            TransactionError::InvalidScript,
        )]));
    }

    #[test]
    fn encode_and_decode_multiple_parcel_invoice() {
        rlp_encode_and_decode_test!(ParcelInvoice::Multiple(vec![
            TransactionInvoice::Fail(TransactionError::InvalidScript),
            TransactionInvoice::Success,
            TransactionInvoice::Success,
            TransactionInvoice::Success,
            TransactionInvoice::Success,
        ]));
    }
}
