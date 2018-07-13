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
use ckey::{self, public_to_address, recover, sign, Private, Public, Signature, SignatureData};
use ctypes::Address;
use heapsize::HeapSizeOf;
use primitives::{Bytes, H160, H256, U256};
use rlp::{self, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::spec::CommonParams;
use super::types::BlockNumber;
use super::Transaction;

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
    /// Max metadata size is exceeded.
    MetadataTooBig,
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
        address: Address,
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
    InvalidShardId(u32),
    /// Not enough permissions given by permission contract.
    NotAllowed,
    /// Signature error
    InvalidSignature(String),
}

pub fn parcel_error_message(error: &ParcelError) -> String {
    use self::ParcelError::*;
    match error {
        AlreadyImported => "Already imported".into(),
        Old => "No longer valid".into(),
        TooCheapToReplace => "Fee too low to replace".into(),
        InvalidNetworkId => "This network ID is not allowed on this chain".into(),
        MetadataTooBig => "Metadata size is too big.".into(),
        LimitReached => "Parcel limit reached".into(),
        InsufficientFee {
            minimal,
            got,
        } => format!("Insufficient fee. Min={}, Given={}", minimal, got),
        InsufficientBalance {
            address,
            balance,
            cost,
        } => format!("{} has only {:?} but it must be larger than {:?}", address, balance, cost),
        InvalidNonce {
            expected,
            got,
        } => format!("Invalid parcel nonce: expected {}, found {}", expected, got),
        InvalidShardId(shard_id) => format!("{} is an invalid shard id", shard_id),
        NotAllowed => "Sender does not have permissions to execute this type of transaction".into(),
        InvalidSignature(err) => format!("Parcel has invalid signature: {}.", err),
    }
}

impl fmt::Display for ParcelError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg: String = parcel_error_message(self);

        f.write_fmt(format_args!("Parcel error ({})", msg))
    }
}

impl From<ckey::Error> for ParcelError {
    fn from(err: ckey::Error) -> Self {
        ParcelError::InvalidSignature(format!("{}", err))
    }
}

/// Fake address for unsigned parcel as defined by EIP-86.
pub const UNSIGNED_SENDER: Address = H160([0xff; 20]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parcel {
    /// Nonce.
    pub nonce: U256,
    /// Amount of CCC to be paid as a cost for distributing this parcel to the network.
    pub fee: U256,
    /// Mainnet or Testnet
    pub network_id: u64,

    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum Action {
    ChangeShardState {
        /// Transaction, can be either asset mint or asset transfer
        transactions: Vec<Transaction>,
    },
    Payment {
        receiver: Address,
        /// Transferred amount.
        amount: U256,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
}

const CHANGE_SHARD_STATE: u8 = 1;
const PAYMENT: u8 = 2;
const SET_REGULAR_KEY: u8 = 3;
const CREATE_SHARD: u8 = 4;

impl HeapSizeOf for Parcel {
    fn heap_size_of_children(&self) -> usize {
        0
    }
}

impl rlp::Encodable for Action {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Action::ChangeShardState {
                transactions,
            } => {
                s.begin_list(2);
                s.append(&CHANGE_SHARD_STATE);
                s.append_list(transactions);
            }
            Action::Payment {
                receiver,
                amount,
            } => {
                s.begin_list(3);
                s.append(&PAYMENT);
                s.append(receiver);
                s.append(amount);
            }
            Action::SetRegularKey {
                key,
            } => {
                s.begin_list(2);
                s.append(&SET_REGULAR_KEY);
                s.append(key);
            }
            Action::CreateShard => {
                s.begin_list(1);
                s.append(&CREATE_SHARD);
            }
        }
    }
}

impl rlp::Decodable for Action {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.val_at(0)? {
            CHANGE_SHARD_STATE => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::ChangeShardState {
                    transactions: rlp.list_at(1)?,
                })
            }
            PAYMENT => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::Payment {
                    receiver: rlp.val_at(1)?,
                    amount: rlp.val_at(2)?,
                })
            }
            SET_REGULAR_KEY => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::SetRegularKey {
                    key: rlp.val_at(1)?,
                })
            }
            CREATE_SHARD => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::CreateShard)
            }
            _ => Err(DecoderError::Custom("Unexpected action prefix")),
        }
    }
}

impl Parcel {
    /// Append object with a without signature into RLP stream
    pub fn rlp_append_unsigned_parcel(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&self.nonce);
        s.append(&self.fee);
        s.append(&self.network_id);
        s.append(&self.action);
    }

    /// The message hash of the parcel.
    pub fn hash(&self) -> H256 {
        let mut stream = RlpStream::new();
        self.rlp_append_unsigned_parcel(&mut stream);
        blake256(stream.as_raw())
    }

    /// Signs the parcel as coming from `sender`.
    pub fn sign(self, private: &Private) -> SignedParcel {
        let sig = sign(&private, &self.hash()).expect("data is valid and context has signing capabilities; qed");
        SignedParcel::new(self.with_signature(sig)).expect("secret is valid so it's recoverable")
    }

    /// Signs the parcel with signature.
    pub fn with_signature(self, sig: Signature) -> UnverifiedParcel {
        UnverifiedParcel {
            unsigned: self,
            sig: sig.into(),
            hash: 0.into(),
        }.compute_hash()
    }
}

/// Signed parcel information without verified signature.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnverifiedParcel {
    /// Plain Parcel.
    unsigned: Parcel,
    /// Signature.
    sig: SignatureData,
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
        if d.item_count()? != 5 {
            return Err(DecoderError::RlpIncorrectListLen)
        }
        let hash = blake256(d.as_raw());
        Ok(UnverifiedParcel {
            unsigned: Parcel {
                nonce: d.val_at(0)?,
                fee: d.val_at(1)?,
                network_id: d.val_at(2)?,
                action: d.val_at(3)?,
            },
            sig: d.val_at(4)?,
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
        self.signature().is_unsigned()
    }

    /// Append object with a signature into RLP stream
    fn rlp_append_sealed_parcel(&self, s: &mut RlpStream) {
        s.begin_list(5);
        s.append(&self.nonce);
        s.append(&self.fee);
        s.append(&self.network_id);
        s.append(&self.action);
        s.append(&self.sig);
    }

    /// Reference to unsigned part of this parcel.
    pub fn as_unsigned(&self) -> &Parcel {
        &self.unsigned
    }

    /// Get the hash of this header (blake256 of the RLP).
    pub fn hash(&self) -> H256 {
        self.hash
    }

    /// Construct a signature object from the sig.
    pub fn signature(&self) -> Signature {
        Signature::from(self.sig)
    }

    /// Recovers the public key of the sender.
    pub fn recover_public(&self) -> Result<Public, ckey::Error> {
        Ok(recover(&self.signature(), &self.unsigned.hash())?)
    }

    /// Checks whether the signature has a low 's' value.
    pub fn check_low_s(&self) -> Result<(), ckey::Error> {
        if !self.signature().is_low_s() {
            Err(ckey::Error::InvalidSignature.into())
        } else {
            Ok(())
        }
    }

    /// Verify basic signature params. Does not attempt sender recovery.
    pub fn verify_basic(&self, params: &CommonParams, allow_empty_signature: bool) -> Result<(), ParcelError> {
        if !(allow_empty_signature && self.is_unsigned()) {
            self.check_low_s()?;
        }
        if self.network_id != params.network_id {
            return Err(ParcelError::InvalidNetworkId)
        }
        match &self.action {
            Action::ChangeShardState {
                transactions,
            } => {
                for t in transactions {
                    match &t {
                        Transaction::AssetMint {
                            network_id,
                            metadata,
                            ..
                        } => {
                            if metadata.len() > params.max_metadata_size {
                                return Err(ParcelError::MetadataTooBig)
                            }
                            if network_id != &self.network_id {
                                return Err(ParcelError::InvalidNetworkId)
                            }
                        }
                        Transaction::AssetTransfer {
                            network_id,
                            ..
                        } => {
                            if network_id != &self.network_id {
                                return Err(ParcelError::InvalidNetworkId)
                            }
                        }
                    }
                }
            }
            _ => {}
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
    pub fn new(parcel: UnverifiedParcel) -> Result<Self, ckey::Error> {
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
#[serde(rename_all = "camelCase")]
pub struct AssetOutPoint {
    pub transaction_hash: H256,
    pub index: usize,
    pub asset_type: H256,
    pub amount: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferInput {
    pub prev_out: AssetOutPoint,
    pub lock_script: Bytes,
    pub unlock_script: Bytes,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferOutput {
    pub lock_script_hash: H256,
    pub parameters: Vec<Bytes>,
    pub asset_type: H256,
    pub amount: u64,
}

#[cfg(test)]
mod tests {
    use ckey::SignatureData;
    use ctypes::{Address, Public};
    use primitives::H256;

    use super::*;

    #[test]
    fn test_unverified_parcel_rlp() {
        rlp_encode_and_decode_test!(
            UnverifiedParcel {
                unsigned: Parcel {
                    nonce: 0.into(),
                    fee: 10.into(),
                    action: Action::CreateShard,
                    network_id: 0xBE,
                },
                sig: SignatureData::default(),
                hash: H256::default(),
            }.compute_hash()
        );
    }

    #[test]
    fn encode_and_decode_asset_mint() {
        rlp_encode_and_decode_test!(Transaction::AssetMint {
            network_id: 200,
            metadata: "mint test".to_string(),
            lock_script_hash: H256::random(),
            parameters: vec![],
            amount: Some(10000),
            registrar: None,
            nonce: 0,
        });
    }

    #[test]
    fn encode_and_decode_asset_mint_with_parameters() {
        rlp_encode_and_decode_test!(Transaction::AssetMint {
            network_id: 200,
            metadata: "mint test".to_string(),
            lock_script_hash: H256::random(),
            parameters: vec![vec![1, 2, 3], vec![4, 5, 6], vec![0, 7]],
            amount: Some(10000),
            registrar: None,
            nonce: 0,
        });
    }

    #[test]
    fn encode_and_decode_asset_transfer() {
        let burns = vec![];
        let inputs = vec![];
        let outputs = vec![];
        let network_id = 0;
        rlp_encode_and_decode_test!(Transaction::AssetTransfer {
            network_id,
            burns,
            inputs,
            outputs,
            nonce: 0,
        });
    }

    #[test]
    fn encode_and_decode_payment_action() {
        rlp_encode_and_decode_test!(Action::Payment {
            receiver: Address::random(),
            amount: 300.into(),
        });
    }

    #[test]
    fn encode_and_decode_payment_parcel() {
        rlp_encode_and_decode_test!(
            UnverifiedParcel {
                unsigned: Parcel {
                    nonce: 30.into(),
                    fee: 40.into(),
                    network_id: 50,
                    action: Action::Payment {
                        receiver: Address::random(),
                        amount: 300.into(),
                    },
                },
                sig: SignatureData::default(),
                hash: H256::default(),
            }.compute_hash()
        );
    }

    #[test]
    fn encode_and_decode_set_regular_key_parcel() {
        rlp_encode_and_decode_test!(
            UnverifiedParcel {
                unsigned: Parcel {
                    nonce: 30.into(),
                    fee: 40.into(),
                    network_id: 50,
                    action: Action::SetRegularKey {
                        key: Public::random(),
                    },
                },
                sig: SignatureData::default(),
                hash: H256::default(),
            }.compute_hash()
        );
    }

    #[test]
    fn encode_and_decode_create_shard_parcel() {
        rlp_encode_and_decode_test!(
            UnverifiedParcel {
                unsigned: Parcel {
                    nonce: 30.into(),
                    fee: 40.into(),
                    network_id: 50,
                    action: Action::CreateShard,
                },
                sig: SignatureData::default(),
                hash: H256::default(),
            }.compute_hash()
        );
    }
}
