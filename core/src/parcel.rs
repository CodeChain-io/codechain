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

use cbytes::Bytes;
use ccrypto::blake256;
use ckeys::{self, public_to_address, recover_ecdsa, sign_ecdsa, ECDSASignature, Private, Public};
use ctypes::{Address, H160, H256, U256, U512};
use heapsize::HeapSizeOf;
use rlp::{self, DecoderError, Encodable, RlpStream, UntrustedRlp};
use unexpected::Mismatch;

use super::types::BlockNumber;

#[derive(Debug, PartialEq, Clone)]
/// Errors concerning parcel processing.
pub enum ParcelError {
    /// Parcel is already imported to the queue
    AlreadyImported,
    /// Parcel is not valid anymore (state already has higher nonce)
    Old,
    /// Parcel has too low fee
    /// (there is already a parcel with the same sender-nonce but higher gas price)
    TooCheapToReplace,
    /// Invalid chain ID given.
    InvalidNetworkId,
    /// Parcel was not imported to the queue because limit has been reached.
    LimitReached,
    /// Parcel's fee is below currently set minimal fee requirement.
    InsufficientFee {
        /// Minimal expected fee
        minimal: U256,
        /// Parcel fee
        got: U256,
    },
    /// Sender doesn't have enough funds to pay for this Parcel
    InsufficientBalance {
        /// Senders balance
        balance: U256,
        /// Parcel cost
        cost: U256,
    },
    /// Returned when parcel nonce does not match state nonce.
    InvalidNonce {
        /// Nonce expected.
        expected: U256,
        /// Nonce found.
        got: U256,
    },
    /// Returned when cost of parcel exceeds current sender balance.
    NotEnoughCash {
        /// Minimum required balance.
        required: U512,
        /// Actual balance.
        got: U512,
    },
    InvalidAssetAmount {
        address: H256,
        expected: u64,
        got: u64,
    },
    /// Not enough permissions given by permission contract.
    NotAllowed,
    /// Signature error
    InvalidSignature(String),
    /// Desired input asset not found
    AssetNotFound(H256),
    /// Desired input asset scheme not found
    AssetSchemeNotFound(H256),
    InvalidAssetType(H256),
    /// Script hash does not match with provided lock script
    ScriptHashMismatch(Mismatch<H256>),
    /// Failed to decode script
    InvalidScript,
    /// Script execution result is `Fail`
    FailedToUnlock(H256),
}

pub fn parcel_error_message(error: &ParcelError) -> String {
    use self::ParcelError::*;
    match *error {
        AlreadyImported => "Already imported".into(),
        Old => "No longer valid".into(),
        TooCheapToReplace => "Gas price too low to replace".into(),
        InvalidNetworkId => "Parcel of this network ID is not allowed on this chain.".into(),
        LimitReached => "Parcel limit reached".into(),
        InsufficientFee {
            minimal,
            got,
        } => format!("Insufficient fee. Min={}, Given={}", minimal, got),
        InsufficientBalance {
            balance,
            cost,
        } => format!("Insufficient balance for parcel. Balance={}, Cost={}", balance, cost),
        InvalidNonce {
            ref expected,
            ref got,
        } => format!("Invalid parcel nonce: expected {}, found {}", expected, got),
        NotEnoughCash {
            ref required,
            ref got,
        } => format!(
            "Cost of parcel exceeds sender balance. {} is required \
             but the sender only has {}",
            required, got
        ),
        InvalidAssetAmount {
            ref address,
            ref expected,
            ref got,
        } => format!(
            "AssetTransfer must consume input asset completely. The amount of asset({}) must be {}, but {}.",
            address, expected, got
        ),
        NotAllowed => "Sender does not have permissions to execute this type of transction".into(),
        InvalidSignature(ref err) => format!("Parcel has invalid signature: {}.", err),
        AssetNotFound(ref addr) => format!("Asset not found: {}", addr),
        AssetSchemeNotFound(ref addr) => format!("Asset scheme not found: {}", addr),
        InvalidAssetType(ref addr) => format!("Asset type is invalid: {}", addr),
        // FIXME: show more information about script
        ScriptHashMismatch(mismatch) => {
            format!("Expected script with hash {}, but got {}", mismatch.expected, mismatch.found)
        }
        InvalidScript => "Failed to decode script".into(),
        FailedToUnlock(ref hash) => format!("Failed to unlock asset {}", hash),
    }
}

impl fmt::Display for ParcelError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg: String = parcel_error_message(self);

        f.write_fmt(format_args!("Parcel error ({})", msg))
    }
}

impl From<ckeys::Error> for ParcelError {
    fn from(err: ckeys::Error) -> Self {
        ParcelError::InvalidSignature(format!("{}", err))
    }
}

/// Fake address for unsigned parcel as defined by EIP-86.
pub const UNSIGNED_SENDER: Address = H160([0xff; 20]);

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Parcel {
    /// Nonce.
    pub nonce: U256,
    /// Amount of CCC to be paid as a cost for distributing this parcel to the network.
    pub fee: U256,
    /// Transaction, can be either payment or asset transfer
    pub transaction: Transaction,
    /// Mainnet or Testnet
    pub network_id: u64,
}

/// Parcel transaction type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Transaction {
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
        lock_script_hash: H256,
        parameters: Vec<Bytes>,
        amount: Option<u64>,
        registrar: Option<Address>,
    },
    AssetTransfer {
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
    },
}

impl Default for Transaction {
    fn default() -> Transaction {
        Transaction::Noop
    }
}

impl Transaction {
    fn without_script(&self) -> Self {
        match self {
            &Transaction::AssetTransfer {
                ref inputs,
                ref outputs,
            } => {
                let new_inputs: Vec<_> = inputs
                    .iter()
                    .map(|input| AssetTransferInput {
                        prev_out: input.prev_out.clone(),
                        lock_script: Vec::new(),
                        unlock_script: Vec::new(),
                    })
                    .collect();
                Transaction::AssetTransfer {
                    inputs: new_inputs,
                    outputs: outputs.clone(),
                }
            }
            _ => unreachable!(),
        }
    }
}

type TransactionId = u8;
const PAYMENT_ID: TransactionId = 0x01;
const SET_REGULAR_KEY_ID: TransactionId = 0x02;
const ASSET_MINT_ID: TransactionId = 0x03;
const ASSET_TRANSFER_ID: TransactionId = 0x04;

impl rlp::Decodable for Transaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.is_empty() {
            return Ok(Transaction::Noop)
        }

        match d.val_at(0)? {
            PAYMENT_ID => {
                if d.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::Payment {
                    address: d.val_at(1)?,
                    value: d.val_at(2)?,
                })
            }
            SET_REGULAR_KEY_ID => {
                if d.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::SetRegularKey {
                    key: d.val_at(1)?,
                })
            }
            ASSET_MINT_ID => {
                if d.item_count()? != 6 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetMint {
                    metadata: d.val_at(1)?,
                    lock_script_hash: d.val_at(2)?,
                    parameters: d.val_at(3)?,
                    amount: d.val_at(4)?,
                    registrar: d.val_at(5)?,
                })
            }
            ASSET_TRANSFER_ID => {
                if d.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetTransfer {
                    inputs: d.list_at(1)?,
                    outputs: d.list_at(2)?,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected transaction")),
        }
    }
}

impl rlp::Encodable for Transaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        match *self {
            Transaction::Noop => s.append_internal(&""),
            Transaction::Payment {
                ref address,
                ref value,
            } => s.begin_list(3).append(&PAYMENT_ID).append(address).append(value),
            Transaction::SetRegularKey {
                ref key,
            } => s.begin_list(2).append(&SET_REGULAR_KEY_ID).append(key),
            Transaction::AssetMint {
                ref metadata,
                ref lock_script_hash,
                ref parameters,
                ref amount,
                ref registrar,
            } => s.begin_list(6)
                .append(&ASSET_MINT_ID)
                .append(metadata)
                .append(lock_script_hash)
                .append(parameters)
                .append(amount)
                .append(registrar),
            Transaction::AssetTransfer {
                ref inputs,
                ref outputs,
            } => s.begin_list(3).append(&ASSET_TRANSFER_ID).append_list(inputs).append_list(outputs),
        };
    }
}

impl HeapSizeOf for Parcel {
    fn heap_size_of_children(&self) -> usize {
        0
    }
}

impl Parcel {
    /// Append object with a without signature into RLP stream
    pub fn rlp_append_unsigned_parcel(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.nonce);
        s.append(&self.fee);
        s.append(&self.transaction);
        s.append(&self.network_id);
    }

    /// The message hash of the parcel.
    pub fn hash(&self) -> H256 {
        let mut stream = RlpStream::new();
        self.rlp_append_unsigned_parcel(&mut stream);
        blake256(stream.as_raw())
    }

    /// Get hash of parcel excluding script field
    pub fn hash_without_script(&self) -> H256 {
        let mut stream = RlpStream::new();
        stream.begin_list(4);
        stream.append(&self.nonce);
        stream.append(&self.fee);
        stream.append(&self.transaction.without_script());
        stream.append(&self.network_id);
        blake256(stream.as_raw())
    }

    /// Signs the parcel as coming from `sender`.
    pub fn sign(self, private: &Private) -> SignedParcel {
        let sig = sign_ecdsa(&private, &self.hash()).expect("data is valid and context has signing capabilities; qed");
        SignedParcel::new(self.with_signature(sig)).expect("secret is valid so it's recoverable")
    }

    /// Signs the parcel with signature.
    pub fn with_signature(self, sig: ECDSASignature) -> UnverifiedParcel {
        UnverifiedParcel {
            unsigned: self,
            r: sig.r().into(),
            s: sig.s().into(),
            v: sig.v() as u64 + 27,
            hash: 0.into(),
        }.compute_hash()
    }
}

/// Signed parcel information without verified signature.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnverifiedParcel {
    /// Plain Parcel.
    unsigned: Parcel,
    /// The V field of the signature; the LS bit described which half of the curve our point falls
    /// in. The MS bits describe which chain this parcel is for. If 27/28, its for all chains.
    v: u64,
    /// The R field of the signature; helps describe the point on the curve.
    r: U256,
    /// The S field of the signature; helps describe the point on the curve.
    s: U256,
    /// Hash of the parcel
    hash: H256,
}

impl Deref for UnverifiedParcel {
    type Target = Parcel;

    fn deref(&self) -> &Self::Target {
        &self.unsigned
    }
}

impl rlp::Decodable for UnverifiedParcel {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.item_count()? != 7 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        let hash = blake256(d.as_raw());
        Ok(UnverifiedParcel {
            unsigned: Parcel {
                nonce: d.val_at(0)?,
                fee: d.val_at(1)?,
                transaction: d.val_at(2)?,
                network_id: d.val_at(3)?,
            },
            v: d.val_at(4)?,
            r: d.val_at(5)?,
            s: d.val_at(6)?,
            hash,
        })
    }
}

impl rlp::Encodable for UnverifiedParcel {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.rlp_append_sealed_parcel(s)
    }
}

impl UnverifiedParcel {
    /// Used to compute hash of created parcels
    fn compute_hash(mut self) -> UnverifiedParcel {
        let hash = blake256(&*self.rlp_bytes());
        self.hash = hash;
        self
    }

    /// Checks is signature is empty.
    pub fn is_unsigned(&self) -> bool {
        self.r.is_zero() && self.s.is_zero()
    }

    /// Append object with a signature into RLP stream
    fn rlp_append_sealed_parcel(&self, s: &mut RlpStream) {
        s.begin_list(7);
        s.append(&self.nonce);
        s.append(&self.fee);
        s.append(&self.transaction);
        s.append(&self.network_id);
        s.append(&self.v);
        s.append(&self.r);
        s.append(&self.s);
    }

    /// Reference to unsigned part of this parcel.
    pub fn as_unsigned(&self) -> &Parcel {
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
    pub fn verify_basic(&self, network_id: u64, allow_empty_signature: bool) -> Result<(), ParcelError> {
        if !(allow_empty_signature && self.is_unsigned()) {
            self.check_low_s()?;
        }
        if self.network_id != network_id {
            return Err(ParcelError::InvalidNetworkId)
        }
        Ok(())
    }
}

/// A `UnverifiedParcel` with successfully recovered `sender`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedParcel {
    parcel: UnverifiedParcel,
    sender: Address,
    public: Option<Public>,
}

impl HeapSizeOf for SignedParcel {
    fn heap_size_of_children(&self) -> usize {
        self.parcel.unsigned.heap_size_of_children()
    }
}

impl rlp::Encodable for SignedParcel {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.parcel.rlp_append_sealed_parcel(s)
    }
}

impl Deref for SignedParcel {
    type Target = UnverifiedParcel;
    fn deref(&self) -> &Self::Target {
        &self.parcel
    }
}

impl From<SignedParcel> for UnverifiedParcel {
    fn from(parcel: SignedParcel) -> Self {
        parcel.parcel
    }
}

impl SignedParcel {
    /// Try to verify parcel and recover sender.
    pub fn new(parcel: UnverifiedParcel) -> Result<Self, ckeys::Error> {
        if parcel.is_unsigned() {
            Ok(SignedParcel {
                parcel,
                sender: UNSIGNED_SENDER,
                public: None,
            })
        } else {
            let public = parcel.recover_public()?;
            let sender = public_to_address(&public);
            Ok(SignedParcel {
                parcel,
                sender,
                public: Some(public),
            })
        }
    }

    /// Returns parcel sender.
    pub fn sender(&self) -> Address {
        self.sender.clone()
    }

    /// Returns a public key of the sender.
    pub fn public_key(&self) -> Option<Public> {
        self.public.clone()
    }

    /// Checks is signature is empty.
    pub fn is_unsigned(&self) -> bool {
        self.parcel.is_unsigned()
    }

    /// Deconstructs this parcel back into `UnverifiedParcel`
    pub fn deconstruct(self) -> (UnverifiedParcel, Address, Option<Public>) {
        (self.parcel, self.sender, self.public)
    }
}

/// Signed Parcel that is a part of canon blockchain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedParcel {
    /// Signed part.
    pub signed: UnverifiedParcel,
    /// Block number.
    pub block_number: BlockNumber,
    /// Block hash.
    pub block_hash: H256,
    /// Parcel index within block.
    pub parcel_index: usize,
    /// Cached sender
    pub cached_sender: Option<Address>,
}

impl LocalizedParcel {
    /// Returns parcel sender.
    /// Panics if `LocalizedParcel` is constructed using invalid `UnverifiedParcel`.
    pub fn sender(&mut self) -> Address {
        if let Some(sender) = self.cached_sender {
            return sender
        }
        if self.is_unsigned() {
            return UNSIGNED_SENDER.clone()
        }
        let sender = public_to_address(&self.recover_public()
            .expect("LocalizedParcel is always constructed from parcel from blockchain; Blockchain only stores verified parcels; qed"));
        self.cached_sender = Some(sender);
        sender
    }
}

impl Deref for LocalizedParcel {
    type Target = UnverifiedParcel;

    fn deref(&self) -> &Self::Target {
        &self.signed
    }
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Serialize)]
pub struct AssetOutPoint {
    pub parcel_hash: H256,
    pub index: usize,
    pub asset_type: H256,
    pub amount: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Serialize)]
pub struct AssetTransferInput {
    pub prev_out: AssetOutPoint,
    pub lock_script: Bytes,
    pub unlock_script: Bytes,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Serialize)]
pub struct AssetTransferOutput {
    pub lock_script_hash: H256,
    pub parameters: Vec<Bytes>,
    pub asset_type: H256,
    pub amount: u64,
}

#[cfg(test)]
mod tests {
    use ctypes::{Address, H256, Public, U256};
    use rlp::Encodable;

    use super::{Parcel, Transaction, UnverifiedParcel};

    #[test]
    fn test_unverified_parcel_rlp() {
        let parcel = UnverifiedParcel {
            unsigned: Parcel::default(),
            v: 0,
            r: U256::default(),
            s: U256::default(),
            hash: H256::default(),
        }.compute_hash();
        assert_eq!(parcel, ::rlp::decode(parcel.rlp_bytes().as_ref()));
    }

    #[test]
    fn encode_and_decode_noop() {
        let transaction = Transaction::Noop;
        assert_eq!(transaction, ::rlp::decode(transaction.rlp_bytes().as_ref()))
    }

    #[test]
    fn encode_and_decode_payment() {
        let address = Address::random();
        let value = U256::from(12345);
        let transaction = Transaction::Payment {
            address,
            value,
        };
        assert_eq!(transaction, ::rlp::decode(transaction.rlp_bytes().as_ref()))
    }

    #[test]
    fn encode_and_decode_set_regular_key() {
        let key = Public::random();
        let transaction = Transaction::SetRegularKey {
            key,
        };
        assert_eq!(transaction, ::rlp::decode(transaction.rlp_bytes().as_ref()))
    }

    #[test]
    fn encode_and_decode_asset_mint() {
        let transaction = Transaction::AssetMint {
            metadata: "mint test".to_string(),
            lock_script_hash: H256::random(),
            parameters: vec![],
            amount: Some(10000),
            registrar: None,
        };

        assert_eq!(transaction, ::rlp::decode(transaction.rlp_bytes().as_ref()))
    }

    #[test]
    fn encode_and_decode_asset_mint_with_parameters() {
        let transaction = Transaction::AssetMint {
            metadata: "mint test".to_string(),
            lock_script_hash: H256::random(),
            parameters: vec![vec![1, 2, 3], vec![4, 5, 6], vec![0, 7]],
            amount: Some(10000),
            registrar: None,
        };

        assert_eq!(transaction, ::rlp::decode(transaction.rlp_bytes().as_ref()))
    }

    #[test]
    fn encode_and_decode_asset_transfer() {
        let inputs = vec![];
        let outputs = vec![];
        let transaction = Transaction::AssetTransfer {
            inputs,
            outputs,
        };

        assert_eq!(transaction, ::rlp::decode(transaction.rlp_bytes().as_ref()))
    }
}
