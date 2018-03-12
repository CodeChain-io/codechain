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

use std::fmt;
use std::ops::Deref;

use codechain_types::{H160, H256, U256};
use crypto::blake256;
use keys::{self, Private, Signature, Public, Address, Network};
use rlp::{self, UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

use super::Bytes;

#[derive(Debug, PartialEq, Clone)]
/// Errors concerning transaction processing.
pub enum TransactionError {
    /// Transaction is already imported to the queue
    AlreadyImported,
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::TransactionError::*;
        let msg: String = match *self {
            AlreadyImported => "Already imported".into(),
        };

        f.write_fmt(format_args!("Transaction error ({})", msg))
    }
}

/// Fake address for unsigned transactions.
fn unsigned_sender(network: Network) -> Address {
    Address {
        network,
        account_id: H160([0xff; 20]),
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Nonce.
    pub nonce: U256,
    /// Transaction data.
    pub data: Bytes,
    /// Mainnet or Testnet
    network: Network,
}

impl Decodable for Transaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.item_count()? != 3 {
            return Err(DecoderError::RlpIncorrectListLen);
        }
        Ok(Transaction {
                nonce: d.val_at(0)?,
                data: d.val_at(1)?,
                network: d.val_at(2)?,
        })
    }
}

impl Transaction {
    /// Append object with a without signature into RLP stream
    pub fn rlp_append_unsigned_transaction(&self, s: &mut RlpStream) {
        s.begin_list(3);
        s.append(&self.nonce);
        s.append(&self.data);
        s.append(&self.network);
    }

    /// The message hash of the transaction.
    pub fn hash(&self) -> H256 {
        let mut stream = RlpStream::new();
        self.rlp_append_unsigned_transaction(&mut stream);
        blake256(stream.as_raw())
    }

    /// Signs the transaction as coming from `sender`.
    pub fn sign(self, private: &Private) -> SignedTransaction {
        let sig = private.sign(&self.hash())
            .expect("data is valid and context has signing capabilities; qed");
        SignedTransaction::new(self.with_signature(sig))
            .expect("secret is valid so it's recoverable")
    }

    /// Signs the transaction with signature.
    pub fn with_signature(self, sig: Signature) -> UnverifiedTransaction {
        UnverifiedTransaction {
            unsigned: self,
            r: sig.r().into(),
            s: sig.s().into(),
            v: sig.v() as u64 + 27,
            hash: 0.into(),
        }.compute_hash()
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
        if d.item_count()? != 6 {
            return Err(DecoderError::RlpIncorrectListLen);
        }
        let hash = blake256(d.as_raw());
        Ok(UnverifiedTransaction {
            unsigned: Transaction {
                nonce: d.val_at(0)?,
                data: d.val_at(1)?,
                network: d.val_at(2)?,
            },
            v: d.val_at(3)?,
            r: d.val_at(4)?,
            s: d.val_at(5)?,
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

    /// 0 if `v` would have been 27 under "Electrum" notation, 1 if 28 or 4 if invalid.
    pub fn standard_v(&self) -> u8 { match self.v { v if v == 27 || v == 28 => ((v - 1) % 2) as u8, _ => 4 } }

    /// Construct a signature object from the sig.
    pub fn signature(&self) -> Signature {
        Signature::from_rsv(&self.r.into(), &self.s.into(), self.standard_v())
    }

    /// Recovers the public key of the sender.
    pub fn recover_public(&self) -> Result<Public, keys::Error> {
        Ok(self.signature().recover(&self.unsigned.hash())?)
    }
}

/// A `UnverifiedTransaction` with successfully recovered `sender`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedTransaction {
    transaction: UnverifiedTransaction,
    sender: Address,
    public: Option<Public>,
}

impl rlp::Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) { self.transaction.rlp_append_sealed_transaction(s) }
}

impl Deref for SignedTransaction {
    type Target = UnverifiedTransaction;
    fn deref(&self) -> &Self::Target {
        &self.transaction
    }
}

impl From<SignedTransaction> for UnverifiedTransaction {
    fn from(tx: SignedTransaction) -> Self {
        tx.transaction
    }
}

impl SignedTransaction {
    /// Try to verify transaction and recover sender.
    pub fn new(transaction: UnverifiedTransaction) -> Result<Self, keys::Error> {
        let network = transaction.network;
        if transaction.is_unsigned() {
            Ok(SignedTransaction {
                transaction,
                sender: unsigned_sender(network),
                public: None,
            })
        } else {
            let public = transaction.recover_public()?;
            let sender = public.address(network);
            Ok(SignedTransaction {
                transaction,
                sender,
                public: Some(public),
            })
        }
    }

    /// Returns transaction sender.
    pub fn sender(&self) -> Address {
        self.sender.clone()
    }

    /// Returns a public key of the sender.
    pub fn public_key(&self) -> Option<Public> {
        self.public.clone()
    }

    /// Checks is signature is empty.
    pub fn is_unsigned(&self) -> bool {
        self.transaction.is_unsigned()
    }

    /// Deconstructs this transaction back into `UnverifiedTransaction`
    pub fn deconstruct(self) -> (UnverifiedTransaction, Address, Option<Public>) {
        (self.transaction, self.sender, self.public)
    }
}
