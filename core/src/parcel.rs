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
use ckey::{self, recover, sign, Private, Public, Signature};
use ctypes::parcel::{Action, Error as ParcelError, Parcel};
use ctypes::transaction::Transaction;
use ctypes::BlockNumber;
use heapsize::HeapSizeOf;
use primitives::H256;
use rlp::{self, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::scheme::CommonParams;

/// Signed parcel information without verified signature.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnverifiedParcel {
    /// Plain Parcel.
    unsigned: Parcel,
    /// Signature.
    sig: Signature,
    /// Hash of the parcel
    hash: H256,
}

impl Deref for UnverifiedParcel {
    type Target = Parcel;

    fn deref(&self) -> &Self::Target {
        &self.unsigned
    }
}

impl From<UnverifiedParcel> for Parcel {
    fn from(parcel: UnverifiedParcel) -> Self {
        parcel.unsigned
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
        }
        .compute_hash()
    }

    /// Used to compute hash of created parcels
    fn compute_hash(mut self) -> UnverifiedParcel {
        let hash = blake256(&*self.rlp_bytes());
        self.hash = hash;
        self
    }

    /// Append object with a signature into RLP stream
    fn rlp_append_sealed_parcel(&self, s: &mut RlpStream) {
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
        Signature::from(self.sig)
    }

    /// Recovers the public key of the signature.
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

    /// Verify basic signature params. Does not attempt signer recovery.
    pub fn verify_basic(&self, params: &CommonParams) -> Result<(), ParcelError> {
        if self.network_id != params.network_id {
            return Err(ParcelError::InvalidNetworkId(self.network_id))
        }
        let byte_size = rlp::encode(self).to_vec().len();
        if byte_size >= params.max_body_size {
            return Err(ParcelError::ParcelsTooBig)
        }
        match &self.action {
            Action::AssetTransaction(transaction) => {
                transaction.verify()?;
                if transaction.network_id() != self.network_id {
                    return Err(ParcelError::InvalidNetworkId(transaction.network_id()))
                }
                match &transaction {
                    Transaction::AssetMint {
                        metadata,
                        ..
                    } => {
                        if metadata.len() > params.max_metadata_size {
                            return Err(ParcelError::MetadataTooBig)
                        }
                    }
                    Transaction::AssetTransfer {
                        ..
                    } => {}
                    Transaction::AssetCompose {
                        metadata,
                        ..
                    } => {
                        if metadata.len() > params.max_metadata_size {
                            return Err(ParcelError::MetadataTooBig)
                        }
                    }
                    Transaction::AssetDecompose {
                        ..
                    } => {}
                    Transaction::AssetUnwrapCCC {
                        ..
                    } => {}
                }
            }
            Action::WrapCCC {
                amount,
                ..
            } => {
                if amount == &0 {
                    return Err(ParcelError::ZeroAmount)
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// A `UnverifiedParcel` with successfully recovered `signer`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedParcel {
    parcel: UnverifiedParcel,
    signer_public: Public,
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
    /// Try to verify parcel and recover public.
    pub fn new(parcel: UnverifiedParcel) -> Result<Self, ckey::Error> {
        let public = parcel.recover_public()?;
        Ok(SignedParcel {
            parcel,
            signer_public: public,
        })
    }

    /// Signs the parcel as coming from `signer`.
    pub fn new_with_sign(parcel: Parcel, private: &Private) -> SignedParcel {
        let sig = sign(&private, &parcel.hash()).expect("data is valid and context has signing capabilities; qed");
        SignedParcel::new(UnverifiedParcel::new(parcel, sig)).expect("secret is valid so it's recoverable")
    }

    /// Returns a public key of the signer.
    pub fn signer_public(&self) -> Public {
        self.signer_public
    }

    /// Deconstructs this parcel back into `UnverifiedParcel`
    pub fn deconstruct(self) -> (UnverifiedParcel, Public) {
        (self.parcel, self.signer_public)
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
    /// Cached public
    pub cached_signer_public: Option<Public>,
}

impl LocalizedParcel {
    /// Returns parcel signer.
    /// Panics if `LocalizedParcel` is constructed using invalid `UnverifiedParcel`.
    pub fn signer(&mut self) -> Public {
        if let Some(public) = self.cached_signer_public {
            return public
        }
        let public = self.recover_public()
            .expect("LocalizedParcel is always constructed from parcel from blockchain; Blockchain only stores verified parcels; qed");
        self.cached_signer_public = Some(public);
        public
    }
}

impl Deref for LocalizedParcel {
    type Target = UnverifiedParcel;

    fn deref(&self) -> &Self::Target {
        &self.signed
    }
}

impl From<LocalizedParcel> for Parcel {
    fn from(parcel: LocalizedParcel) -> Self {
        parcel.signed.into()
    }
}

#[cfg(test)]
mod tests {
    use ckey::{Address, Public, Signature};
    use ctypes::transaction::AssetMintOutput;
    use primitives::{H160, H256};
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn unverified_parcel_rlp() {
        rlp_encode_and_decode_test!(
            UnverifiedParcel {
                unsigned: Parcel {
                    seq: 0,
                    fee: 10,
                    action: Action::CreateShard,
                    network_id: "tc".into(),
                },
                sig: Signature::default(),
                hash: H256::default(),
            }
            .compute_hash()
        );
    }

    #[test]
    fn encode_and_decode_asset_mint() {
        rlp_encode_and_decode_test!(Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id: 0xc,
            metadata: "mint test".to_string(),
            output: AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                amount: Some(10000),
            },
            registrar: None,
        });
    }

    #[test]
    fn encode_and_decode_asset_mint_with_parameters() {
        rlp_encode_and_decode_test!(Transaction::AssetMint {
            network_id: "tc".into(),
            shard_id: 3,
            metadata: "mint test".to_string(),
            output: AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![vec![1, 2, 3], vec![4, 5, 6], vec![0, 7]],
                amount: Some(10000),
            },
            registrar: None,
        });
    }

    #[test]
    fn encode_and_decode_asset_transfer() {
        let burns = vec![];
        let inputs = vec![];
        let outputs = vec![];
        let network_id = "tc".into();
        rlp_encode_and_decode_test!(Transaction::AssetTransfer {
            network_id,
            burns,
            inputs,
            outputs,
        });
    }

    #[test]
    fn encode_and_decode_payment_action() {
        rlp_encode_and_decode_test!(Action::Payment {
            receiver: Address::random(),
            amount: 300,
        });
    }

    #[test]
    fn encode_and_decode_payment_parcel() {
        rlp_encode_and_decode_test!(
            UnverifiedParcel {
                unsigned: Parcel {
                    seq: 30,
                    fee: 40,
                    network_id: "tc".into(),
                    action: Action::Payment {
                        receiver: Address::random(),
                        amount: 300,
                    },
                },
                sig: Signature::default(),
                hash: H256::default(),
            }
            .compute_hash()
        );
    }

    #[test]
    fn encode_and_decode_set_regular_key_parcel() {
        rlp_encode_and_decode_test!(
            UnverifiedParcel {
                unsigned: Parcel {
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
            .compute_hash()
        );
    }

    #[test]
    fn encode_and_decode_create_shard_parcel() {
        rlp_encode_and_decode_test!(
            UnverifiedParcel {
                unsigned: Parcel {
                    seq: 30,
                    fee: 40,
                    network_id: "tc".into(),
                    action: Action::CreateShard,
                },
                sig: Signature::default(),
                hash: H256::default(),
            }
            .compute_hash()
        );
    }
}
