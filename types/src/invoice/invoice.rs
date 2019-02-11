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

use primitives::Bytes;
use rlp::{DecoderError, RlpStream, UntrustedRlp};
use serde::{Serialize, Serializer};

#[derive(Clone, Debug, PartialEq)]
pub enum Invoice {
    Success,
    Failure(String),
}

const INVOICE_ID_SINGLE_SUCCESS: u8 = 1u8;
const INVOICE_ID_SINGLE_FAIL: u8 = 2u8;

impl Serialize for Invoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        match self {
            Invoice::Success => serializer.serialize_bool(true),
            Invoice::Failure(_) => serializer.serialize_bool(false),
        }
    }
}

impl Invoice {
    pub fn bytes_to_store(&self) -> Bytes {
        match self {
            Invoice::Success => {
                let mut s = RlpStream::new_list(1);
                s.append(&INVOICE_ID_SINGLE_SUCCESS);
                s.drain().to_vec()
            }
            Invoice::Failure(err) => {
                let mut s = RlpStream::new_list(2);
                s.append(&INVOICE_ID_SINGLE_FAIL);
                s.append(err);
                s.drain().to_vec()
            }
        }
    }

    pub fn recover_from_bytes(bytes: &[u8]) -> Result<Invoice, DecoderError> {
        let rlp = UntrustedRlp::new(bytes);
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
            _ => Err(DecoderError::Custom("Unknown invoice")),
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Invoice::Success => true,
            Invoice::Failure(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_decode_single_success_tx_invoice() {
        let origin = Invoice::Success;
        let bytes = origin.bytes_to_store();
        assert_eq!(Ok(origin), Invoice::recover_from_bytes(&bytes));
    }

    #[test]
    fn encode_and_decode_single_failed_tx_invoice() {
        let origin = Invoice::Failure("It failed because it's an invalid transaction.".to_string());
        let bytes = origin.bytes_to_store();
        assert_eq!(Ok(origin), Invoice::recover_from_bytes(&bytes));
    }
}
