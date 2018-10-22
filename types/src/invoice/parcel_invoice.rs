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
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::super::parcel::Error;
use super::invoice_result::InvoiceResult;

#[derive(Clone, Debug, PartialEq)]
pub enum Invoice {
    Success,
    Failure(Error),
}

const INVOICE_ID_SINGLE_SUCCESS: u8 = 1u8;
const INVOICE_ID_SINGLE_FAIL: u8 = 2u8;

impl Serialize for Invoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        match self {
            Invoice::Success => {
                let mut s = serializer.serialize_struct("ParcelInvoice", 1)?;
                s.serialize_field("success", &true)?;
                s.end()
            }
            Invoice::Failure(ref err) => {
                let mut s = serializer.serialize_struct("ParcelInvoice", 2)?;
                s.serialize_field("success", &false)?;
                s.serialize_field("error", err)?;
                s.end()
            }
        }
    }
}

impl Invoice {
    pub fn result(&self) -> InvoiceResult {
        match self {
            Invoice::Success => InvoiceResult::Success,
            Invoice::Failure(_) => InvoiceResult::Failed,
        }
    }
}

impl Encodable for Invoice {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Invoice::Success => {
                s.begin_list(1);
                s.append(&INVOICE_ID_SINGLE_SUCCESS);
            }
            Invoice::Failure(err) => {
                s.begin_list(2);
                s.append(&INVOICE_ID_SINGLE_FAIL);
                s.append(err);
            }
        }
    }
}

impl Decodable for Invoice {
    fn decode(rlp: &UntrustedRlp) -> Result<Invoice, DecoderError> {
        match rlp.val_at::<u8>(0)? {
            INVOICE_ID_SINGLE_SUCCESS => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Ok(Invoice::Success)
            }
            INVOICE_ID_SINGLE_FAIL => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Ok(Invoice::Failure(rlp.val_at(1)?))
            }
            _ => Err(DecoderError::Custom("Unknown parcel invoice")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_single_success_parcel_invoice() {
        rlp_encode_and_decode_test!(Invoice::Success);
    }

    #[test]
    fn encode_and_decode_single_failed_parcel_invoice() {
        rlp_encode_and_decode_test!(Invoice::Failure(Error::Old));
    }
}
