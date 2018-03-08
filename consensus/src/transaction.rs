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

use std::ops::Deref;
use codechain_crypto::blake256;
use codechain_types::{H256, U256};
use super::Bytes;
use rlp::{self, UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Nonce.
    pub nonce: U256,
    /// Transaction data.
    pub data: Bytes,
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

impl Transaction {
    /// Append object with a without signature into RLP stream
    pub fn rlp_append_unsigned_transaction(&self, s: &mut RlpStream) {
        s.begin_list(2);
        s.append(&self.nonce);
        s.append(&self.data);
    }

    /// The message hash of the transaction.
    pub fn hash(&self) -> H256 {
        let mut stream = RlpStream::new();
        self.rlp_append_unsigned_transaction(&mut stream);
        blake256(stream.as_raw())
    }
}

/// Signed transaction information without verified signature.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnverifiedTransaction {
    /// Plain Transaction.
    unsigned: Transaction,
    /// The V field of the signature; the LS bit described which half of the curve our point falls
    /// in. The MS bits describe which chain this transaction is for. If 27/28, its for all chains.
    v: u64,
    /// The R field of the signature; helps describe the point on the curve.
    r: U256,
    /// The S field of the signature; helps describe the point on the curve.
    s: U256,
    /// Hash of the transaction
    hash: H256,
}

impl Deref for UnverifiedTransaction {
    type Target = Transaction;

    fn deref(&self) -> &Self::Target {
        &self.unsigned
    }
}

impl rlp::Decodable for UnverifiedTransaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.item_count()? != 5 {
            return Err(DecoderError::RlpIncorrectListLen);
        }
        let hash = blake256(d.as_raw());
        Ok(UnverifiedTransaction {
            unsigned: Transaction {
                nonce: d.val_at(0)?,
                data: d.val_at(1)?,
            },
            v: d.val_at(2)?,
            r: d.val_at(3)?,
            s: d.val_at(4)?,
            hash,
        })
    }
}

impl rlp::Encodable for UnverifiedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) { self.rlp_append_sealed_transaction(s) }
}

impl UnverifiedTransaction {
    /// Used to compute hash of created transactions
    fn compute_hash(mut self) -> UnverifiedTransaction {
        let hash = blake256(&*self.rlp_bytes());
        self.hash = hash;
        self
    }

    /// Checks is signature is empty.
    pub fn is_unsigned(&self) -> bool {
        self.r.is_zero() && self.s.is_zero()
    }

    /// Append object with a signature into RLP stream
    fn rlp_append_sealed_transaction(&self, s: &mut RlpStream) {
        s.begin_list(5);
        s.append(&self.nonce);
        s.append(&self.data);
        s.append(&self.v);
        s.append(&self.r);
        s.append(&self.s);
    }

    /// Reference to unsigned part of this transaction.
    pub fn as_unsigned(&self) -> &Transaction {
        &self.unsigned
    }

    /// Get the hash of this header (blake256 of the RLP).
    pub fn hash(&self) -> H256 {
        self.hash
    }
}
