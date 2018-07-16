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

use ccrypto::blake256;
use ckey::{self, public_to_address, recover, sign, Address, Private, Public, Signature, SignatureData};
use ctypes::parcel::{Action, Error as ParcelError, Parcel};
use ctypes::transaction::Transaction;
use heapsize::HeapSizeOf;
use primitives::{H160, H256};
use rlp::{self, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::spec::CommonParams;
use super::types::BlockNumber;

/// Fake address for unsigned parcel as defined by EIP-86.
pub const UNSIGNED_SENDER: Address = H160([0xff; 20]);

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
    pub fn new(parcel: Parcel, sig: Signature) -> Self {
        UnverifiedParcel {
            unsigned: parcel,
            sig: sig.into(),
            hash: 0.into(),
        }.compute_hash()
    }

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

    /// Signs the parcel as coming from `sender`.
    pub fn new_with_sign(parcel: Parcel, private: &Private) -> SignedParcel {
        let sig = sign(&private, &parcel.hash()).expect("data is valid and context has signing capabilities; qed");
        SignedParcel::new(UnverifiedParcel::new(parcel, sig)).expect("secret is valid so it's recoverable")
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

#[cfg(test)]
mod tests {
    use ckey::{Address, Public, SignatureData};
    use ctypes::transaction::AssetMintOutput;
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
            output: AssetMintOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                amount: Some(10000),
            },
            registrar: None,
            nonce: 0,
        });
    }

    #[test]
    fn encode_and_decode_asset_mint_with_parameters() {
        rlp_encode_and_decode_test!(Transaction::AssetMint {
            network_id: 200,
            metadata: "mint test".to_string(),
            output: AssetMintOutput {
                lock_script_hash: H256::random(),
                parameters: vec![vec![1, 2, 3], vec![4, 5, 6], vec![0, 7]],
                amount: Some(10000),
            },
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
