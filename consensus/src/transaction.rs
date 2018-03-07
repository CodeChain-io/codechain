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

use codechain_types::{U256};
use super::Bytes;
use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Nonce.
    pub nonce: U256,
    /// Transaction data.
    pub data: Bytes,
}

impl Transaction {
    /// Append object with a signature into RLP stream
    fn rlp_append_sealed_transaction(&self, s: &mut RlpStream) {
        s.begin_list(2);
        s.append(&self.nonce);
        s.append(&self.data);
    }
}

impl Encodable for Transaction {
    fn rlp_append(&self, s: &mut RlpStream) { self.rlp_append_sealed_transaction(s) }
}

impl Decodable for Transaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.item_count()? != 2 {
            return Err(DecoderError::RlpIncorrectListLen);
        }
        Ok(Transaction {
                nonce: d.val_at(0)?,
                data: d.val_at(1)?,
        })
    }
}
