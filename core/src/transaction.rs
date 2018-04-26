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

use ccrypto::blake256;
use ckeys::{self, public_to_address, recover_ecdsa, sign_ecdsa, ECDSASignature, Private, Public};
use ctypes::{Address, H160, H256, U256, U512};
use heapsize::HeapSizeOf;
use rlp::{self, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::types::BlockNumber;

#[derive(Debug, PartialEq, Clone)]
/// Errors concerning transaction processing.
pub enum TransactionError {
    /// Transaction is already imported to the queue
    AlreadyImported,
    /// Transaction is not valid anymore (state already has higher nonce)
    Old,
    /// Transaction has too low fee
    /// (there is already a transaction with the same sender-nonce but higher gas price)
    TooCheapToReplace,
    /// Invalid chain ID given.
    InvalidNetworkId,
    /// Transaction was not imported to the queue because limit has been reached.
    LimitReached,
    /// Transaction's fee is below currently set minimal fee requirement.
    InsufficientFee {
        /// Minimal expected fee
        minimal: U256,
        /// Transaction fee
        got: U256,
    },
    /// Sender doesn't have enough funds to pay for this transaction
    InsufficientBalance {
        /// Senders balance
        balance: U256,
        /// Transaction cost
        cost: U256,
    },
    /// Returned when transaction nonce does not match state nonce.
    InvalidNonce {
        /// Nonce expected.
        expected: U256,
        /// Nonce found.
        got: U256,
    },
    /// Returned when cost of transaction exceeds current sender balance.
    NotEnoughCash {
        /// Minimum required balance.
        required: U512,
        /// Actual balance.
        got: U512,
    },
    /// Not enough permissions given by permission contract.
    NotAllowed,
    /// Signature error
    InvalidSignature(String),
}

pub fn transaction_error_message(error: &TransactionError) -> String {
    use self::TransactionError::*;
    match *error {
        AlreadyImported => "Already imported".into(),
        Old => "No longer valid".into(),
        TooCheapToReplace => "Gas price too low to replace".into(),
        InvalidNetworkId => "Transaction of this network ID is not allowed on this chain.".into(),
        LimitReached => "Transaction limit reached".into(),
        InsufficientFee {
            minimal,
            got,
        } => format!("Insufficient fee. Min={}, Given={}", minimal, got),
        InsufficientBalance {
            balance,
            cost,
        } => format!("Insufficient balance for transaction. Balance={}, Cost={}", balance, cost),
        InvalidNonce {
            ref expected,
            ref got,
        } => format!("Invalid transaction nonce: expected {}, found {}", expected, got),
        NotEnoughCash {
            ref required,
            ref got,
        } => format!(
            "Cost of transaction exceeds sender balance. {} is required \
             but the sender only has {}",
            required, got
        ),
        NotAllowed => "Sender does not have permissions to execute this type of transction".into(),
        InvalidSignature(ref err) => format!("Transaction has invalid signature: {}.", err),
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg: String = transaction_error_message(self);

        f.write_fmt(format_args!("Transaction error ({})", msg))
    }
}

impl From<ckeys::Error> for TransactionError {
    fn from(err: ckeys::Error) -> Self {
        TransactionError::InvalidSignature(format!("{}", err))
    }
}

/// Fake address for unsigned transactions as defined by EIP-86.
pub const UNSIGNED_SENDER: Address = H160([0xff; 20]);

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Nonce.
    pub nonce: U256,
    /// Amount of CCC to be paid as a cost for distributing this transaction to the network.
    pub fee: U256,
    /// Action, can be either payment or asset transfer
    pub action: Action,
    /// Mainnet or Testnet
    pub network_id: u64,
}

/// Transaction action type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Noop,
    Payment {
        /// The receiver's address.
        address: Address,
        /// Transferred value.
        value: U256,
    },
    SetRegularKey {
        key: Public,
    },
    AssetMint {
        metadata: String,
        lock_script: H256,
        amount: Option<U256>,
        registrar: Option<Address>,
    },
}

impl Default for Action {
    fn default() -> Action {
        Action::Noop
    }
}

impl rlp::Decodable for Action {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.is_empty() {
            Ok(Action::Noop)
        } else if d.is_data() {
            Ok(Action::SetRegularKey {
                key: d.as_val()?,
            })
        } else {
            let item_count = d.item_count()?;
            if item_count == 2 {
                Ok(Action::Payment {
                    address: d.val_at(0)?,
                    value: d.val_at(1)?,
                })
            } else if item_count == 4 {
                Ok(Action::AssetMint {
                    metadata: d.val_at(0)?,
                    lock_script: d.val_at(1)?,
                    amount: d.val_at(2)?,
                    registrar: d.val_at(3)?,
                })
            } else {
                Err(DecoderError::RlpIncorrectListLen)
            }
        }
    }
}

impl rlp::Encodable for Action {
    fn rlp_append(&self, s: &mut RlpStream) {
        match *self {
            Action::Noop => s.append_internal(&""),
            Action::Payment {
                ref address,
                ref value,
            } => {
                s.begin_list(2);
                s.append(address);
                s.append(value)
            }
            Action::SetRegularKey {
                ref key,
            } => s.append_internal(key),
            Action::AssetMint {
                ref metadata,
                ref lock_script,
                ref amount,
                ref registrar,
            } => s.begin_list(4).append(metadata).append(lock_script).append(amount).append(registrar),
        };
    }
}

impl HeapSizeOf for Transaction {
    fn heap_size_of_children(&self) -> usize {
        0
    }
}

impl Transaction {
    /// Append object with a without signature into RLP stream
    pub fn rlp_append_unsigned_transaction(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.nonce);
        s.append(&self.fee);
        s.append(&self.action);
        s.append(&self.network_id);
    }

    /// The message hash of the transaction.
    pub fn hash(&self) -> H256 {
        let mut stream = RlpStream::new();
        self.rlp_append_unsigned_transaction(&mut stream);
        blake256(stream.as_raw())
    }

    /// Signs the transaction as coming from `sender`.
    pub fn sign(self, private: &Private) -> SignedTransaction {
        let sig = sign_ecdsa(&private, &self.hash()).expect("data is valid and context has signing capabilities; qed");
        SignedTransaction::new(self.with_signature(sig)).expect("secret is valid so it's recoverable")
    }

    /// Signs the transaction with signature.
    pub fn with_signature(self, sig: ECDSASignature) -> UnverifiedTransaction {
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
        if d.item_count()? != 7 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        let hash = blake256(d.as_raw());
        Ok(UnverifiedTransaction {
            unsigned: Transaction {
                nonce: d.val_at(0)?,
                fee: d.val_at(1)?,
                action: d.val_at(2)?,
                network_id: d.val_at(3)?,
            },
            v: d.val_at(4)?,
            r: d.val_at(5)?,
            s: d.val_at(6)?,
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
        s.begin_list(7);
        s.append(&self.nonce);
        s.append(&self.fee);
        s.append(&self.action);
        s.append(&self.network_id);
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
    pub fn standard_v(&self) -> u8 {
        match self.v {
            v if v == 27 || v == 28 => ((v - 1) % 2) as u8,
            _ => 4,
        }
    }

    /// Construct a signature object from the sig.
    pub fn signature(&self) -> ECDSASignature {
        ECDSASignature::from_rsv(&self.r.into(), &self.s.into(), self.standard_v())
    }

    /// Recovers the public key of the sender.
    pub fn recover_public(&self) -> Result<Public, ckeys::Error> {
        Ok(recover_ecdsa(&self.signature(), &self.unsigned.hash())?)
    }

    /// Checks whether the signature has a low 's' value.
    pub fn check_low_s(&self) -> Result<(), ckeys::Error> {
        if !self.signature().is_low_s() {
            Err(ckeys::Error::InvalidSignature.into())
        } else {
            Ok(())
        }
    }

    /// Verify basic signature params. Does not attempt sender recovery.
    pub fn verify_basic(&self, network_id: u64, allow_empty_signature: bool) -> Result<(), TransactionError> {
        if !(allow_empty_signature && self.is_unsigned()) {
            self.check_low_s()?;
        }
        if self.network_id != network_id {
            return Err(TransactionError::InvalidNetworkId)
        }
        Ok(())
    }
}

/// A `UnverifiedTransaction` with successfully recovered `sender`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedTransaction {
    transaction: UnverifiedTransaction,
    sender: Address,
    public: Option<Public>,
}

impl HeapSizeOf for SignedTransaction {
    fn heap_size_of_children(&self) -> usize {
        self.transaction.unsigned.heap_size_of_children()
    }
}

impl rlp::Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.transaction.rlp_append_sealed_transaction(s)
    }
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
    pub fn new(transaction: UnverifiedTransaction) -> Result<Self, ckeys::Error> {
        if transaction.is_unsigned() {
            Ok(SignedTransaction {
                transaction,
                sender: UNSIGNED_SENDER,
                public: None,
            })
        } else {
            let public = transaction.recover_public()?;
            let sender = public_to_address(&public);
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
    /// Cached sender
    pub cached_sender: Option<Address>,
}

impl LocalizedTransaction {
    /// Returns transaction sender.
    /// Panics if `LocalizedTransaction` is constructed using invalid `UnverifiedTransaction`.
    pub fn sender(&mut self) -> Address {
        if let Some(sender) = self.cached_sender {
            return sender
        }
        if self.is_unsigned() {
            return UNSIGNED_SENDER.clone()
        }
        let sender = public_to_address(&self.recover_public()
            .expect("LocalizedTransaction is always constructed from transaction from blockchain; Blockchain only stores verified transactions; qed"));
        self.cached_sender = Some(sender);
        sender
    }
}

impl Deref for LocalizedTransaction {
    type Target = UnverifiedTransaction;

    fn deref(&self) -> &Self::Target {
        &self.signed
    }
}

#[cfg(test)]
mod tests {
    use ctypes::{H256, U256};
    use rlp::Encodable;

    use super::{Action, Transaction, UnverifiedTransaction};

    #[test]
    fn test_unverified_transaction_rlp() {
        let tx = UnverifiedTransaction {
            unsigned: Transaction::default(),
            v: 0,
            r: U256::default(),
            s: U256::default(),
            hash: H256::default(),
        }.compute_hash();
        assert_eq!(tx, ::rlp::decode(tx.rlp_bytes().as_ref()));
    }

    #[test]
    fn encode_and_decode_asset_mint() {
        let action = Action::AssetMint {
            metadata: "mint test".to_string(),
            lock_script: H256::random(),
            amount: Some(10000.into()),
            registrar: None,
        };

        assert_eq!(action, ::rlp::decode(action.rlp_bytes().as_ref()))
    }
}
