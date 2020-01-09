// Copyright 2019. Kodebox, Inc.
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

use super::TaggedRlp;
use crate::util::unexpected::Mismatch;
use crate::{ShardId, Tracker};
use ckey::Address;
use primitives::H160;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use std::fmt::{Display, Formatter, Result as FormatResult};

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum Error {
    /// Desired input asset not found
    AssetNotFound {
        shard_id: ShardId,
        tracker: Tracker,
        index: usize,
    },
    AssetSchemeDuplicated {
        tracker: Tracker,
        shard_id: ShardId,
    },
    /// Desired input asset scheme not found
    AssetSchemeNotFound {
        asset_type: H160,
        shard_id: ShardId,
    },
    InvalidSeqOfAssetScheme {
        asset_type: H160,
        shard_id: ShardId,
        expected: usize,
        actual: usize,
    },
    AssetSupplyOverflow,
    CannotBurnRegulatedAsset,
    FailedToHandleCustomAction(String),
    /// Script execution result is `Fail`
    FailedToUnlock {
        shard_id: ShardId,
        tracker: Tracker,
        index: usize,
        reason: UnlockFailureReason,
    },
    /// Sender doesn't have enough funds to pay for this Transaction
    InsufficientBalance {
        address: Address,
        /// Senders balance
        balance: u64,
        /// Transaction cost
        cost: u64,
    },
    InsufficientPermission,
    InvalidAssetQuantity {
        shard_id: ShardId,
        tracker: Tracker,
        index: usize,
        expected: u64,
        got: u64,
    },
    /// AssetType error other than format.
    UnexpectedAssetType {
        index: usize,
        mismatch: Mismatch<H160>,
    },
    /// Failed to decode script.
    InvalidScript,
    /// Returned when transaction seq does not match state seq
    InvalidSeq(Mismatch<u64>),
    InvalidShardId(ShardId),
    InvalidTransferDestination,
    NewOwnersMustContainSender,
    NotApproved(Address),
    RegularKeyAlreadyInUse,
    RegularKeyAlreadyInUseAsPlatformAccount,
    /// Script hash does not match with provided lock script
    ScriptHashMismatch(Mismatch<H160>),
    ScriptNotAllowed(H160),
    TextNotExist,
    /// Remove Text error
    TextVerificationFail(String),
    /// Tried to use master key even register key is registered
    CannotUseMasterKey,
    NonActiveAccount {
        address: Address,
        name: String,
    },
    SignatureOfInvalidAccount(Address),
    InsufficientStakes(Mismatch<u64>),
    InvalidValidatorIndex {
        idx: usize,
        parent_height: u64,
    },
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum ErrorID {
    AssetNotFound = 1,
    AssetSchemeDuplicated = 2,
    AssetSchemeNotFound = 3,
    CannotBurnRegulatedAsset = 4,
    /// Deprecated
    // CANNOT_COMPOSE_REGULATED_ASSET = 5,
    FailedToUnlock = 6,
    InvalidSeqOfAssetScheme = 7,
    InsufficientBalance = 8,
    InsufficientPermission = 9,
    InvalidAssetQuantity = 10,
    UnexpectedAssetType = 11,
    /// Deprecated
    // INVALID_DECOMPOSED_INPUT = 13,
    // INVALID_DECOMPOSED_OUTPUT = 14,
    InvalidShardID = 15,
    InvalidTransferDestination = 16,
    NewOwnersMustContainSender = 17,
    NotApproved = 18,
    RegularKeyAlreadyInUse = 19,
    RegularKeyAlreadyInUseAsPlatform = 20,
    ScriptHashMismatch = 21,
    ScriptNotAllowed = 22,
    TextNotExist = 23,
    TextVerificationFail = 24,
    CannotUseMasterKey = 25,
    InvalidScript = 27,
    InvalidSeq = 28,
    AssetSupplyOverflow = 29,
    NonActiveAccount = 30,
    FailedToHandleCustomAction = 31,
    SignatureOfInvalid = 32,
    InsufficientStakes = 33,
    InvalidValidatorIndex = 34,
}

impl Encodable for ErrorID {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_single_value(&(*self as u8));
    }
}

impl Decodable for ErrorID {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let tag = rlp.as_val()?;
        match tag {
            1u8 => Ok(ErrorID::AssetNotFound),
            2 => Ok(ErrorID::AssetSchemeDuplicated),
            3 => Ok(ErrorID::AssetSchemeNotFound),
            4 => Ok(ErrorID::InvalidSeqOfAssetScheme),
            6 => Ok(ErrorID::AssetSupplyOverflow),
            7 => Ok(ErrorID::CannotBurnRegulatedAsset),
            8 => Ok(ErrorID::FailedToHandleCustomAction),
            9 => Ok(ErrorID::FailedToUnlock),
            10 => Ok(ErrorID::InsufficientBalance),
            11 => Ok(ErrorID::InsufficientPermission),
            15 => Ok(ErrorID::InvalidAssetQuantity),
            16 => Ok(ErrorID::UnexpectedAssetType),
            17 => Ok(ErrorID::InvalidScript),
            18 => Ok(ErrorID::InvalidSeq),
            19 => Ok(ErrorID::InvalidShardID),
            20 => Ok(ErrorID::InvalidTransferDestination),
            21 => Ok(ErrorID::NewOwnersMustContainSender),
            22 => Ok(ErrorID::NotApproved),
            23 => Ok(ErrorID::RegularKeyAlreadyInUse),
            24 => Ok(ErrorID::RegularKeyAlreadyInUseAsPlatform),
            25 => Ok(ErrorID::ScriptHashMismatch),
            27 => Ok(ErrorID::ScriptNotAllowed),
            28 => Ok(ErrorID::TextNotExist),
            29 => Ok(ErrorID::TextVerificationFail),
            30 => Ok(ErrorID::CannotUseMasterKey),
            31 => Ok(ErrorID::NonActiveAccount),
            32 => Ok(ErrorID::SignatureOfInvalid),
            33 => Ok(ErrorID::InsufficientStakes),
            34 => Ok(ErrorID::InvalidValidatorIndex),
            _ => Err(DecoderError::Custom("Unexpected ActionTag Value")),
        }
    }
}

struct RlpHelper;
impl TaggedRlp for RlpHelper {
    type Tag = ErrorID;

    fn length_of(tag: ErrorID) -> Result<usize, DecoderError> {
        Ok(match tag {
            ErrorID::AssetNotFound => 4,
            ErrorID::AssetSchemeDuplicated => 3,
            ErrorID::AssetSchemeNotFound => 3,
            ErrorID::InvalidSeqOfAssetScheme => 5,
            ErrorID::AssetSupplyOverflow => 1,
            ErrorID::CannotBurnRegulatedAsset => 1,
            ErrorID::FailedToHandleCustomAction => 2,
            ErrorID::FailedToUnlock => 5,
            ErrorID::InsufficientBalance => 4,
            ErrorID::InsufficientPermission => 1,
            ErrorID::InvalidAssetQuantity => 6,
            ErrorID::UnexpectedAssetType => 3,
            ErrorID::InvalidScript => 1,
            ErrorID::InvalidSeq => 2,
            ErrorID::InvalidShardID => 2,
            ErrorID::InvalidTransferDestination => 1,
            ErrorID::NewOwnersMustContainSender => 1,
            ErrorID::NotApproved => 2,
            ErrorID::RegularKeyAlreadyInUse => 1,
            ErrorID::RegularKeyAlreadyInUseAsPlatform => 1,
            ErrorID::ScriptHashMismatch => 2,
            ErrorID::ScriptNotAllowed => 2,
            ErrorID::TextNotExist => 1,
            ErrorID::TextVerificationFail => 2,
            ErrorID::CannotUseMasterKey => 1,
            ErrorID::NonActiveAccount => 3,
            ErrorID::SignatureOfInvalid => 2,
            ErrorID::InsufficientStakes => 3,
            ErrorID::InvalidValidatorIndex => 3,
        })
    }
}

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::AssetNotFound {
                shard_id,
                tracker,
                index,
            } => RlpHelper::new_tagged_list(s, ErrorID::AssetNotFound).append(shard_id).append(tracker).append(index),
            Error::AssetSchemeDuplicated {
                tracker,
                shard_id,
            } => RlpHelper::new_tagged_list(s, ErrorID::AssetSchemeDuplicated).append(tracker).append(shard_id),
            Error::AssetSchemeNotFound {
                asset_type,
                shard_id,
            } => RlpHelper::new_tagged_list(s, ErrorID::AssetSchemeNotFound).append(asset_type).append(shard_id),
            Error::InvalidSeqOfAssetScheme {
                asset_type,
                shard_id,
                expected,
                actual,
            } => RlpHelper::new_tagged_list(s, ErrorID::InvalidSeqOfAssetScheme)
                .append(asset_type)
                .append(shard_id)
                .append(expected)
                .append(actual),
            Error::AssetSupplyOverflow => RlpHelper::new_tagged_list(s, ErrorID::AssetSupplyOverflow),
            Error::CannotBurnRegulatedAsset => RlpHelper::new_tagged_list(s, ErrorID::CannotBurnRegulatedAsset),
            Error::FailedToHandleCustomAction(detail) => {
                RlpHelper::new_tagged_list(s, ErrorID::FailedToHandleCustomAction).append(detail)
            }
            Error::FailedToUnlock {
                shard_id,
                tracker,
                index,
                reason,
            } => RlpHelper::new_tagged_list(s, ErrorID::FailedToUnlock)
                .append(shard_id)
                .append(tracker)
                .append(index)
                .append(reason),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => {
                RlpHelper::new_tagged_list(s, ErrorID::InsufficientBalance).append(address).append(balance).append(cost)
            }
            Error::InsufficientPermission => RlpHelper::new_tagged_list(s, ErrorID::InsufficientPermission),
            Error::InvalidAssetQuantity {
                shard_id,
                tracker,
                index,
                expected,
                got,
            } => RlpHelper::new_tagged_list(s, ErrorID::InvalidAssetQuantity)
                .append(shard_id)
                .append(tracker)
                .append(index)
                .append(expected)
                .append(got),
            Error::UnexpectedAssetType {
                index,
                mismatch,
            } => RlpHelper::new_tagged_list(s, ErrorID::UnexpectedAssetType).append(index).append(mismatch),
            Error::InvalidScript => RlpHelper::new_tagged_list(s, ErrorID::InvalidScript),
            Error::InvalidSeq(mismatch) => RlpHelper::new_tagged_list(s, ErrorID::InvalidSeq).append(mismatch),
            Error::InvalidShardId(shard_id) => RlpHelper::new_tagged_list(s, ErrorID::InvalidShardID).append(shard_id),
            Error::InvalidTransferDestination => RlpHelper::new_tagged_list(s, ErrorID::InvalidTransferDestination),
            Error::NewOwnersMustContainSender => RlpHelper::new_tagged_list(s, ErrorID::NewOwnersMustContainSender),
            Error::NotApproved(address) => RlpHelper::new_tagged_list(s, ErrorID::NotApproved).append(address),
            Error::RegularKeyAlreadyInUse => RlpHelper::new_tagged_list(s, ErrorID::RegularKeyAlreadyInUse),
            Error::RegularKeyAlreadyInUseAsPlatformAccount => {
                RlpHelper::new_tagged_list(s, ErrorID::RegularKeyAlreadyInUseAsPlatform)
            }
            Error::ScriptHashMismatch(mismatch) => {
                RlpHelper::new_tagged_list(s, ErrorID::ScriptHashMismatch).append(mismatch)
            }
            Error::ScriptNotAllowed(hash) => RlpHelper::new_tagged_list(s, ErrorID::ScriptNotAllowed).append(hash),
            Error::TextNotExist => RlpHelper::new_tagged_list(s, ErrorID::TextNotExist),
            Error::TextVerificationFail(err) => {
                RlpHelper::new_tagged_list(s, ErrorID::TextVerificationFail).append(err)
            }
            Error::CannotUseMasterKey => RlpHelper::new_tagged_list(s, ErrorID::CannotUseMasterKey),
            Error::NonActiveAccount {
                address,
                name,
            } => RlpHelper::new_tagged_list(s, ErrorID::NonActiveAccount).append(address).append(name),
            Error::SignatureOfInvalidAccount(address) => {
                RlpHelper::new_tagged_list(s, ErrorID::SignatureOfInvalid).append(address)
            }
            Error::InsufficientStakes(Mismatch {
                expected,
                found,
            }) => RlpHelper::new_tagged_list(s, ErrorID::InsufficientStakes).append(expected).append(found),
            Error::InvalidValidatorIndex {
                idx,
                parent_height,
            } => RlpHelper::new_tagged_list(s, ErrorID::InvalidValidatorIndex).append(idx).append(parent_height),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at(0)?;
        let error = match tag {
            ErrorID::AssetNotFound => Error::AssetNotFound {
                shard_id: rlp.val_at(1)?,
                tracker: rlp.val_at(2)?,
                index: rlp.val_at(3)?,
            },
            ErrorID::AssetSchemeDuplicated => Error::AssetSchemeDuplicated {
                tracker: rlp.val_at(1)?,
                shard_id: rlp.val_at(2)?,
            },
            ErrorID::AssetSchemeNotFound => Error::AssetSchemeNotFound {
                asset_type: rlp.val_at(1)?,
                shard_id: rlp.val_at(2)?,
            },
            ErrorID::InvalidSeqOfAssetScheme => Error::InvalidSeqOfAssetScheme {
                asset_type: rlp.val_at(1)?,
                shard_id: rlp.val_at(2)?,
                expected: rlp.val_at(3)?,
                actual: rlp.val_at(4)?,
            },
            ErrorID::AssetSupplyOverflow => Error::AssetSupplyOverflow,
            ErrorID::CannotBurnRegulatedAsset => Error::CannotBurnRegulatedAsset,
            ErrorID::FailedToHandleCustomAction => Error::FailedToHandleCustomAction(rlp.val_at(1)?),
            ErrorID::FailedToUnlock => Error::FailedToUnlock {
                shard_id: rlp.val_at(1)?,
                tracker: rlp.val_at(2)?,
                index: rlp.val_at(3)?,
                reason: rlp.val_at(4)?,
            },
            ErrorID::InsufficientBalance => Error::InsufficientBalance {
                address: rlp.val_at(1)?,
                balance: rlp.val_at(2)?,
                cost: rlp.val_at(3)?,
            },
            ErrorID::InsufficientPermission => Error::InsufficientPermission,
            ErrorID::InvalidAssetQuantity => Error::InvalidAssetQuantity {
                shard_id: rlp.val_at(1)?,
                tracker: rlp.val_at(2)?,
                index: rlp.val_at(3)?,
                expected: rlp.val_at(4)?,
                got: rlp.val_at(5)?,
            },
            ErrorID::UnexpectedAssetType => Error::UnexpectedAssetType {
                index: rlp.val_at(1)?,
                mismatch: rlp.val_at(2)?,
            },
            ErrorID::InvalidScript => Error::InvalidScript,
            ErrorID::InvalidSeq => Error::InvalidSeq(rlp.val_at(1)?),
            ErrorID::InvalidShardID => Error::InvalidShardId(rlp.val_at(1)?),
            ErrorID::InvalidTransferDestination => Error::InvalidTransferDestination,
            ErrorID::NewOwnersMustContainSender => Error::NewOwnersMustContainSender,
            ErrorID::NotApproved => Error::NotApproved(rlp.val_at(1)?),
            ErrorID::RegularKeyAlreadyInUse => Error::RegularKeyAlreadyInUse,
            ErrorID::RegularKeyAlreadyInUseAsPlatform => Error::RegularKeyAlreadyInUseAsPlatformAccount,
            ErrorID::ScriptHashMismatch => Error::ScriptHashMismatch(rlp.val_at(1)?),
            ErrorID::ScriptNotAllowed => Error::ScriptNotAllowed(rlp.val_at(1)?),
            ErrorID::TextNotExist => Error::TextNotExist,
            ErrorID::TextVerificationFail => Error::TextVerificationFail(rlp.val_at(1)?),
            ErrorID::CannotUseMasterKey => Error::CannotUseMasterKey,
            ErrorID::NonActiveAccount => Error::NonActiveAccount {
                address: rlp.val_at(1)?,
                name: rlp.val_at(2)?,
            },
            ErrorID::SignatureOfInvalid => Error::SignatureOfInvalidAccount(rlp.val_at(1)?),
            ErrorID::InsufficientStakes => Error::InsufficientStakes(Mismatch {
                expected: rlp.val_at(1)?,
                found: rlp.val_at(2)?,
            }),
            ErrorID::InvalidValidatorIndex => Error::InvalidValidatorIndex {
                idx: rlp.val_at(1)?,
                parent_height: rlp.val_at(2)?,
            },
        };
        RlpHelper::check_size(rlp, tag)?;
        Ok(error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            Error::AssetNotFound { shard_id, tracker, index } => write!(f, "Asset not found: {}:{}:{}", shard_id, tracker, index),
            Error::AssetSchemeDuplicated { tracker, shard_id} => write!(f, "Asset scheme already exists: {}:{}", shard_id, tracker),
            Error::AssetSchemeNotFound {
                asset_type,
                shard_id,
            } => write!(f, "Asset scheme not found: {}:{}", asset_type, shard_id),
            Error::InvalidSeqOfAssetScheme {
                asset_type,
                shard_id,
                expected,
                actual,
            } => write!(f, "Already used seq of asset scheme {}:{}. expected: {}, actual: {}", asset_type, shard_id, expected, actual),
            Error::AssetSupplyOverflow => write!(f, "Asset supply should not be overflowed"),
            Error::CannotBurnRegulatedAsset => write!(f, "Cannot burn the regulated asset"),
            Error::FailedToHandleCustomAction(detail) => write!(f, "Cannot handle custom action: {}", detail),
            Error::FailedToUnlock {
                shard_id,
                tracker,
                index,
                reason,
            } => write!(f, "Failed to unlock asset {}:{}:{}, reason: {}", shard_id, tracker, index, reason),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => write!(f, "{} has only {:?} but it must be larger than {:?}", address, balance, cost),
            Error::InsufficientPermission => write!(f, "Sender doesn't have a permission"),
            Error::InvalidAssetQuantity {
                shard_id,
                tracker,
                index,
                expected,
                got,
            } => write!(
                f,
                "AssetTransfer must consume input asset completely. The quantity of asset({}:{}:{}) must be {}, but {}.",
                shard_id, tracker, index, expected, got
            ),
            Error::UnexpectedAssetType{index, mismatch} => write!(f, "{}th input has an unexpected asset type: {}", index, mismatch),
            Error::InvalidScript => write!(f, "Failed to decode script"),
            Error::InvalidSeq(mismatch) => write!(f, "Invalid transaction seq {}", mismatch),
            Error::InvalidShardId(shard_id) => write!(f, "{} is an invalid shard id", shard_id),
            Error::InvalidTransferDestination => write!(f, "Transfer receiver is not valid account"),
            Error::NewOwnersMustContainSender => write!(f, "New owners must contain the sender"),
            Error::NotApproved(address) => write!(f, "{} should approve it.", address),
            Error::RegularKeyAlreadyInUse => write!(f, "The regular key is already registered to another account"),
            Error::RegularKeyAlreadyInUseAsPlatformAccount => {
                write!(f, "The regular key is already used as a platform account")
            }
            Error::ScriptHashMismatch(mismatch) => {
                write!(f, "Expected script with hash {}, but got {}", mismatch.expected, mismatch.found)
            }
            Error::ScriptNotAllowed(hash) => write!(f, "Output lock script hash is not allowed : {}", hash),
            Error::TextNotExist => write!(f, "The text does not exist"),
            Error::TextVerificationFail(err) => write!(f, "Text verification has failed: {}", err),
            Error::CannotUseMasterKey => {
                write!(f, "Cannot use the master key because a regular key is already registered")
            }
            Error::NonActiveAccount {
                name, address,
            } => {
                write!(f, "Non active account({}) cannot be {}", address, name)
            }
            Error::SignatureOfInvalidAccount(address) =>
                write!(f, "Signature of invalid account({}) received", address),
            Error::InsufficientStakes(mismatch) =>
                write!(f, "Insufficient stakes: {}", mismatch),
            Error::InvalidValidatorIndex {
                idx, parent_height,
            } =>  write!(f, "The validator index {} is invalid at the parent hash {}", idx, parent_height),
        }
    }
}


#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
pub enum UnlockFailureReason {
    ScriptShouldBeBurnt,
    ScriptShouldNotBeBurnt,
    ScriptError,
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum ScriptFailureReasonID {
    ShouldBeBurnt = 1u8,
    ShouldNotBeBurnt = 2u8,
    Error = 3u8,
}

impl Encodable for ScriptFailureReasonID {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_single_value(&(*self as u8));
    }
}

impl Decodable for ScriptFailureReasonID {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let tag = rlp.as_val()?;
        match tag {
            1u8 => Ok(ScriptFailureReasonID::ShouldBeBurnt),
            2 => Ok(ScriptFailureReasonID::ShouldNotBeBurnt),
            3 => Ok(ScriptFailureReasonID::Error),
            _ => Err(DecoderError::Custom("Unexpected ScriptFailureReasonID Value")),
        }
    }
}

impl Encodable for UnlockFailureReason {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            UnlockFailureReason::ScriptShouldBeBurnt => (ScriptFailureReasonID::ShouldBeBurnt).rlp_append(s),
            UnlockFailureReason::ScriptShouldNotBeBurnt => (ScriptFailureReasonID::ShouldNotBeBurnt).rlp_append(s),
            UnlockFailureReason::ScriptError => (ScriptFailureReasonID::Error).rlp_append(s),
        };
    }
}

impl Decodable for UnlockFailureReason {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        Ok(match Decodable::decode(rlp)? {
            ScriptFailureReasonID::ShouldBeBurnt => UnlockFailureReason::ScriptShouldBeBurnt,
            ScriptFailureReasonID::ShouldNotBeBurnt => UnlockFailureReason::ScriptShouldNotBeBurnt,
            ScriptFailureReasonID::Error => UnlockFailureReason::ScriptError,
        })
    }
}

impl Display for UnlockFailureReason {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            UnlockFailureReason::ScriptShouldBeBurnt => write!(f, "Script should be burnt"),
            UnlockFailureReason::ScriptShouldNotBeBurnt => write!(f, "Script should not be burnt"),
            UnlockFailureReason::ScriptError => write!(f, "Script error"),
        }
    }
}
