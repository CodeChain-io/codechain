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

use super::invoice::Invoice;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ParcelInvoice {
    Single(Invoice),
    Multiple(Vec<Invoice>),
}

impl ParcelInvoice {
    pub fn new(invoices: Vec<Invoice>) -> Self {
        ParcelInvoice::Multiple(invoices)
    }

    pub fn iter<'a>(&'a self) -> Box<::std::iter::Iterator<Item = &'a Invoice> + 'a> {
        match self {
            ParcelInvoice::Single(invoice) => Box::new(::std::iter::once(invoice)),
            ParcelInvoice::Multiple(invoices) => Box::new(invoices.iter()),
        }
    }
}

impl Encodable for ParcelInvoice {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            ParcelInvoice::Single(invoice) => {
                s.append_single_value(invoice);
            }
            ParcelInvoice::Multiple(invoices) => {
                s.append_list(invoices);
            }
        }
    }
}

impl Decodable for ParcelInvoice {
    fn decode(rlp: &UntrustedRlp) -> Result<ParcelInvoice, DecoderError> {
        Ok(if rlp.is_list() {
            ParcelInvoice::Multiple(rlp.as_list()?)
        } else {
            ParcelInvoice::Single(rlp.as_val()?)
        })
    }
}

impl Into<Vec<Invoice>> for ParcelInvoice {
    fn into(self) -> Vec<Invoice> {
        self.iter().cloned().collect()
    }
}

impl From<Vec<Invoice>> for ParcelInvoice {
    fn from(invoices: Vec<Invoice>) -> Self {
        ParcelInvoice::Multiple(invoices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rlp_encode_and_decode_parcel_invoice() {
        let invoices = vec![
            Invoice::Success,
            Invoice::Success,
            Invoice::Failed,
            Invoice::Success,
            Invoice::Success,
            Invoice::Success,
        ];
        rlp_encode_and_decode_test!(ParcelInvoice::new(invoices));
    }

    #[test]
    fn encode_and_decode_single_success_parcel_invoice() {
        rlp_encode_and_decode_test!(ParcelInvoice::Single(Invoice::Success));
    }

    #[test]
    fn encode_and_decode_single_failed_parcel_invoice() {
        rlp_encode_and_decode_test!(ParcelInvoice::Single(Invoice::Failed));
    }

    #[test]
    fn encode_and_decode_empty_multiple_parcel_invoice() {
        rlp_encode_and_decode_test!(ParcelInvoice::Multiple(vec![]));
    }

    #[test]
    fn encode_and_decode_multiple_parcel_invoice_with_success() {
        rlp_encode_and_decode_test!(ParcelInvoice::Multiple(vec![Invoice::Success]));
    }

    #[test]
    fn encode_and_decode_multiple_parcel_invoice_with_failed() {
        rlp_encode_and_decode_test!(ParcelInvoice::Multiple(vec![Invoice::Failed]));
    }

    #[test]
    fn encode_and_decode_multiple_parcel_invoice() {
        rlp_encode_and_decode_test!(ParcelInvoice::Multiple(vec![
            Invoice::Failed,
            Invoice::Success,
            Invoice::Success,
            Invoice::Success,
            Invoice::Success,
        ]));
    }
}
