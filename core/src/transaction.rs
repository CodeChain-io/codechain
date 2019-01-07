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

use std::ops::Deref;

use ccrypto::blake256;
use ckey::{self, recover, sign, Private, Public, Signature};
use ctypes::transaction::{ParcelError, Transaction};
use ctypes::BlockNumber;
use heapsize::HeapSizeOf;
use primitives::H256;
use rlp::{self, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::scheme::CommonParams;

/// Signed transaction information without verified signature.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnverifiedTransaction {
    /// Plain Transaction.
    unsigned: Transaction,
    /// Signature.
    sig: Signature,
    /// Hash of the transaction
    hash: H256,
}

impl Deref for UnverifiedTransaction {
    type Target = Transaction;

    fn deref(&self) -> &Self::Target {
        &self.unsigned
    }
}

impl From<UnverifiedTransaction> for Transaction {
    fn from(tx: UnverifiedTransaction) -> Self {
        tx.unsigned
    }
}

impl rlp::Decodable for UnverifiedTransaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.item_count()? != 5 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        let hash = blake256(d.as_raw());
        Ok(UnverifiedTransaction {
            unsigned: Transaction {
                seq: d.val_at(0)?,
                fee: d.val_at(1)?,
                network_id: d.val_at(2)?,
                action: d.val_at(3)?,
            },
            sig: d.val_at(4)?,
            hash,
        })
    }
}

impl rlp::Encodable for UnverifiedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.rlp_append_sealed_transaction(s)
    }
}

impl UnverifiedTransaction {
    pub fn new(unsigned: Transaction, sig: Signature) -> Self {
        UnverifiedTransaction {
            unsigned,
            sig,
            hash: 0.into(),
        }
        .compute_hash()
    }

    /// Used to compute hash of created transactions
    fn compute_hash(mut self) -> UnverifiedTransaction {
        let hash = blake256(&*self.rlp_bytes());
        self.hash = hash;
        self
    }

    /// Append object with a signature into RLP stream
    fn rlp_append_sealed_transaction(&self, s: &mut RlpStream) {
        s.begin_list(5);
        s.append(&self.seq);
        s.append(&self.fee);
        s.append(&self.network_id);
        s.append(&self.action);
        s.append(&self.sig);
    }

    /// Get the hash of this header (blake256 of the RLP).
    pub fn hash(&self) -> H256 {
        self.hash
    }

    /// Construct a signature object from the sig.
    pub fn signature(&self) -> Signature {
        self.sig
    }

    /// Recovers the public key of the signature.
    pub fn recover_public(&self) -> Result<Public, ckey::Error> {
        Ok(recover(&self.signature(), &self.unsigned.hash())?)
    }

    /// Checks whether the signature has a low 's' value.
    pub fn check_low_s(&self) -> Result<(), ckey::Error> {
        if !self.signature().is_low_s() {
            Err(ckey::Error::InvalidSignature)
        } else {
            Ok(())
        }
    }

    /// Verify basic signature params. Does not attempt signer recovery.
    pub fn verify_basic(&self, params: &CommonParams) -> Result<(), ParcelError> {
        if self.network_id != params.network_id {
            return Err(ParcelError::InvalidNetworkId(self.network_id))
        }
        let byte_size = rlp::encode(self).to_vec().len();
        if byte_size >= params.max_body_size {
            return Err(ParcelError::TransactionIsTooBig)
        }
        self.action.verify(params.network_id, params.max_metadata_size, params.max_text_content_size)
    }
}

/// A `UnverifiedTransaction` with successfully recovered `signer`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedTransaction {
    tx: UnverifiedTransaction,
    signer_public: Public,
}

impl HeapSizeOf for SignedTransaction {
    fn heap_size_of_children(&self) -> usize {
        self.tx.unsigned.heap_size_of_children()
    }
}

impl rlp::Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.tx.rlp_append_sealed_transaction(s)
    }
}

impl Deref for SignedTransaction {
    type Target = UnverifiedTransaction;
    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl From<SignedTransaction> for UnverifiedTransaction {
    fn from(tx: SignedTransaction) -> Self {
        tx.tx
    }
}

impl SignedTransaction {
    /// Try to verify transaction and recover public.
    pub fn try_new(tx: UnverifiedTransaction) -> Result<Self, ckey::Error> {
        let public = tx.recover_public()?;
        Ok(SignedTransaction {
            tx,
            signer_public: public,
        })
    }

    /// Signs the transaction as coming from `signer`.
    pub fn new_with_sign(tx: Transaction, private: &Private) -> SignedTransaction {
        let sig = sign(&private, &tx.hash()).expect("data is valid and context has signing capabilities; qed");
        SignedTransaction::try_new(UnverifiedTransaction::new(tx, sig)).expect("secret is valid so it's recoverable")
    }

    /// Returns a public key of the signer.
    pub fn signer_public(&self) -> Public {
        self.signer_public
    }

    /// Deconstructs this transaction back into `UnverifiedTransaction`
    pub fn deconstruct(self) -> (UnverifiedTransaction, Public) {
        (self.tx, self.signer_public)
    }
}

/// Signed Transaction that is a part of canon blockchain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedTransaction {
    /// Signed part.
    pub signed: UnverifiedTransaction,
    /// Block number.
    pub block_number: BlockNumber,
    /// Block hash.
    pub block_hash: H256,
    /// Transaction index within block.
    pub transaction_index: usize,
    /// Cached public
    pub cached_signer_public: Option<Public>,
}

impl LocalizedTransaction {
    /// Returns transaction signer.
    /// Panics if `LocalizedTransaction` is constructed using invalid `UnverifiedTransaction`.
    pub fn signer(&mut self) -> Public {
        if let Some(public) = self.cached_signer_public {
            return public
        }
        let public = self.recover_public()
            .expect("LocalizedTransaction is always constructed from transaction from blockchain; Blockchain only stores verified transactions; qed");
        self.cached_signer_public = Some(public);
        public
    }
}

impl Deref for LocalizedTransaction {
    type Target = UnverifiedTransaction;

    fn deref(&self) -> &Self::Target {
        &self.signed
    }
}

impl From<LocalizedTransaction> for Transaction {
    fn from(tx: LocalizedTransaction) -> Self {
        tx.signed.into()
    }
}

#[cfg(test)]
mod tests {
    use ckey::{Address, Public, Signature};
    use ctypes::transaction::Action;
    use primitives::H256;
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn unverified_transaction_rlp() {
        rlp_encode_and_decode_test!(UnverifiedTransaction {
            unsigned: Transaction {
                seq: 0,
                fee: 10,
                action: Action::CreateShard,
                network_id: "tc".into(),
            },
            sig: Signature::default(),
            hash: H256::default(),
        }
        .compute_hash());
    }

    #[test]
    fn encode_and_decode_pay_transaction() {
        rlp_encode_and_decode_test!(UnverifiedTransaction {
            unsigned: Transaction {
                seq: 30,
                fee: 40,
                network_id: "tc".into(),
                action: Action::Pay {
                    receiver: Address::random(),
                    amount: 300,
                },
            },
            sig: Signature::default(),
            hash: H256::default(),
        }
        .compute_hash());
    }

    #[test]
    fn encode_and_decode_set_regular_key_transaction() {
        rlp_encode_and_decode_test!(UnverifiedTransaction {
            unsigned: Transaction {
                seq: 30,
                fee: 40,
                network_id: "tc".into(),
                action: Action::SetRegularKey {
                    key: Public::random(),
                },
            },
            sig: Signature::default(),
            hash: H256::default(),
        }
        .compute_hash());
    }

    #[test]
    fn encode_and_decode_create_shard_transaction() {
        rlp_encode_and_decode_test!(UnverifiedTransaction {
            unsigned: Transaction {
                seq: 30,
                fee: 40,
                network_id: "tc".into(),
                action: Action::CreateShard,
            },
            sig: Signature::default(),
            hash: H256::default(),
        }
        .compute_hash());
    }
}
