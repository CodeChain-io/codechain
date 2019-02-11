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

use std::fmt::{Display, Formatter, Result as FormatResult};

use ckey::Address;
use primitives::{H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::TaggedRlp;
use crate::util::unexpected::Mismatch;
use crate::ShardId;

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum Error {
    /// Desired input asset not found
    AssetNotFound {
        shard_id: ShardId,
        tracker: H256,
        index: usize,
    },
    AssetSchemeDuplicated {
        tracker: H256,
        shard_id: ShardId,
    },
    /// Desired input asset scheme not found
    AssetSchemeNotFound {
        asset_type: H160,
        shard_id: ShardId,
    },
    AssetSupplyOverflow,
    CannotBurnCentralizedAsset,
    CannotComposeCentralizedAsset,
    /// Script execution result is `Fail`
    FailedToUnlock {
        shard_id: ShardId,
        tracker: H256,
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
        tracker: H256,
        index: usize,
        expected: u64,
        got: u64,
    },
    /// AssetType error other than format.
    InvalidAssetType(H160),
    InvalidDecomposedInput {
        asset_type: H160,
        shard_id: ShardId,
        got: u64,
    },
    InvalidDecomposedOutput {
        asset_type: H160,
        shard_id: ShardId,
        expected: u64,
        got: u64,
    },
    /// Errors on orders: origin_outputs of order is not satisfied.
    InvalidOriginOutputs(H256),
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
}

const ERROR_ID_ASSET_NOT_FOUND: u8 = 1;
const ERROR_ID_ASSET_SCHEME_DUPLICATED: u8 = 2;
const ERROR_ID_ASSET_SCHEME_NOT_FOUND: u8 = 3;
const ERROR_ID_CANNOT_BURN_CENTRALIZED_ASSET: u8 = 4;
const ERROR_ID_CANNOT_COMPOSE_CENTRALIZED_ASSET: u8 = 5;
const ERROR_ID_FAILED_TO_UNLOCK: u8 = 6;
const ERROR_ID_INSUFFICIENT_BALANCE: u8 = 8;
const ERROR_ID_INSUFFICIENT_PERMISSION: u8 = 9;
const ERROR_ID_INVALID_ASSET_QUANTITY: u8 = 10;
const ERROR_ID_INVALID_ASSET_TYPE: u8 = 11;
const ERROR_ID_INVALID_DECOMPOSED_INPUT: u8 = 13;
const ERROR_ID_INVALID_DECOMPOSED_OUTPUT: u8 = 14;
const ERROR_ID_INVALID_SHARD_ID: u8 = 15;
const ERROR_ID_INVALID_TRANSFER_DESTINATION: u8 = 16;
const ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER: u8 = 17;
const ERROR_ID_NOT_APPROVED: u8 = 18;
const ERROR_ID_REGULAR_KEY_ALREADY_IN_USE: u8 = 19;
const ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM: u8 = 20;
const ERROR_ID_SCRIPT_HASH_MISMATCH: u8 = 21;
const ERROR_ID_SCRIPT_NOT_ALLOWED: u8 = 22;
const ERROR_ID_TEXT_NOT_EXIST: u8 = 23;
const ERROR_ID_TEXT_VERIFICATION_FAIL: u8 = 24;
const ERROR_ID_CANNOT_USE_MASTER_KEY: u8 = 25;
const ERROR_ID_INVALID_ORIGIN_OUTPUTS: u8 = 26;
const ERROR_ID_INVALID_SCRIPT: u8 = 27;
const ERROR_ID_INVALID_SEQ: u8 = 28;
const ERROR_ID_ASSET_SUPPLY_OVERFLOW: u8 = 29;

struct RlpHelper;
impl TaggedRlp for RlpHelper {
    type Tag = u8;

    fn length_of(tag: u8) -> Result<usize, DecoderError> {
        Ok(match tag {
            ERROR_ID_ASSET_NOT_FOUND => 4,
            ERROR_ID_ASSET_SCHEME_DUPLICATED => 3,
            ERROR_ID_ASSET_SCHEME_NOT_FOUND => 3,
            ERROR_ID_ASSET_SUPPLY_OVERFLOW => 1,
            ERROR_ID_CANNOT_BURN_CENTRALIZED_ASSET => 1,
            ERROR_ID_CANNOT_COMPOSE_CENTRALIZED_ASSET => 1,
            ERROR_ID_FAILED_TO_UNLOCK => 5,
            ERROR_ID_INSUFFICIENT_BALANCE => 4,
            ERROR_ID_INSUFFICIENT_PERMISSION => 1,
            ERROR_ID_INVALID_ASSET_QUANTITY => 6,
            ERROR_ID_INVALID_ASSET_TYPE => 2,
            ERROR_ID_INVALID_DECOMPOSED_INPUT => 4,
            ERROR_ID_INVALID_DECOMPOSED_OUTPUT => 5,
            ERROR_ID_INVALID_ORIGIN_OUTPUTS => 2,
            ERROR_ID_INVALID_SCRIPT => 1,
            ERROR_ID_INVALID_SEQ => 2,
            ERROR_ID_INVALID_SHARD_ID => 2,
            ERROR_ID_INVALID_TRANSFER_DESTINATION => 1,
            ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER => 1,
            ERROR_ID_NOT_APPROVED => 2,
            ERROR_ID_REGULAR_KEY_ALREADY_IN_USE => 1,
            ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM => 1,
            ERROR_ID_SCRIPT_HASH_MISMATCH => 2,
            ERROR_ID_SCRIPT_NOT_ALLOWED => 2,
            ERROR_ID_TEXT_NOT_EXIST => 1,
            ERROR_ID_TEXT_VERIFICATION_FAIL => 2,
            ERROR_ID_CANNOT_USE_MASTER_KEY => 1,
            _ => return Err(DecoderError::Custom("Invalid RuntimeError")),
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
            } => RlpHelper::new_tagged_list(s, ERROR_ID_ASSET_NOT_FOUND).append(shard_id).append(tracker).append(index),
            Error::AssetSchemeDuplicated {
                tracker,
                shard_id,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_ASSET_SCHEME_DUPLICATED).append(tracker).append(shard_id),
            Error::AssetSchemeNotFound {
                asset_type,
                shard_id,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_ASSET_SCHEME_NOT_FOUND).append(asset_type).append(shard_id),
            Error::AssetSupplyOverflow => RlpHelper::new_tagged_list(s, ERROR_ID_ASSET_SUPPLY_OVERFLOW),
            Error::CannotBurnCentralizedAsset => RlpHelper::new_tagged_list(s, ERROR_ID_CANNOT_BURN_CENTRALIZED_ASSET),
            Error::CannotComposeCentralizedAsset => {
                RlpHelper::new_tagged_list(s, ERROR_ID_CANNOT_COMPOSE_CENTRALIZED_ASSET)
            }
            Error::FailedToUnlock {
                shard_id,
                tracker,
                index,
                reason,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_FAILED_TO_UNLOCK)
                .append(shard_id)
                .append(tracker)
                .append(index)
                .append(reason),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INSUFFICIENT_BALANCE)
                .append(address)
                .append(balance)
                .append(cost),
            Error::InsufficientPermission => RlpHelper::new_tagged_list(s, ERROR_ID_INSUFFICIENT_PERMISSION),
            Error::InvalidAssetQuantity {
                shard_id,
                tracker,
                index,
                expected,
                got,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ASSET_QUANTITY)
                .append(shard_id)
                .append(tracker)
                .append(index)
                .append(expected)
                .append(got),
            Error::InvalidAssetType(addr) => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ASSET_TYPE).append(addr),
            Error::InvalidDecomposedInput {
                asset_type,
                shard_id,
                got,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_DECOMPOSED_INPUT)
                .append(asset_type)
                .append(shard_id)
                .append(got),
            Error::InvalidDecomposedOutput {
                asset_type,
                shard_id,
                expected,
                got,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_DECOMPOSED_OUTPUT)
                .append(asset_type)
                .append(shard_id)
                .append(expected)
                .append(got),
            Error::InvalidOriginOutputs(addr) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ORIGIN_OUTPUTS).append(addr)
            }
            Error::InvalidScript => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_SCRIPT),
            Error::InvalidSeq(mismatch) => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_SEQ).append(mismatch),
            Error::InvalidShardId(shard_id) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_SHARD_ID).append(shard_id)
            }
            Error::InvalidTransferDestination => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_TRANSFER_DESTINATION),
            Error::NewOwnersMustContainSender => RlpHelper::new_tagged_list(s, ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER),
            Error::NotApproved(address) => RlpHelper::new_tagged_list(s, ERROR_ID_NOT_APPROVED).append(address),
            Error::RegularKeyAlreadyInUse => RlpHelper::new_tagged_list(s, ERROR_ID_REGULAR_KEY_ALREADY_IN_USE),
            Error::RegularKeyAlreadyInUseAsPlatformAccount => {
                RlpHelper::new_tagged_list(s, ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM)
            }
            Error::ScriptHashMismatch(mismatch) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_SCRIPT_HASH_MISMATCH).append(mismatch)
            }
            Error::ScriptNotAllowed(hash) => RlpHelper::new_tagged_list(s, ERROR_ID_SCRIPT_NOT_ALLOWED).append(hash),
            Error::TextNotExist => RlpHelper::new_tagged_list(s, ERROR_ID_TEXT_NOT_EXIST),
            Error::TextVerificationFail(err) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_TEXT_VERIFICATION_FAIL).append(err)
            }
            Error::CannotUseMasterKey => RlpHelper::new_tagged_list(s, ERROR_ID_CANNOT_USE_MASTER_KEY),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        let error = match tag {
            ERROR_ID_ASSET_NOT_FOUND => Error::AssetNotFound {
                shard_id: rlp.val_at(1)?,
                tracker: rlp.val_at(2)?,
                index: rlp.val_at(3)?,
            },
            ERROR_ID_ASSET_SCHEME_DUPLICATED => Error::AssetSchemeDuplicated {
                tracker: rlp.val_at(1)?,
                shard_id: rlp.val_at(2)?,
            },
            ERROR_ID_ASSET_SCHEME_NOT_FOUND => Error::AssetSchemeNotFound {
                asset_type: rlp.val_at(1)?,
                shard_id: rlp.val_at(2)?,
            },
            ERROR_ID_ASSET_SUPPLY_OVERFLOW => Error::AssetSupplyOverflow,
            ERROR_ID_CANNOT_BURN_CENTRALIZED_ASSET => Error::CannotBurnCentralizedAsset,
            ERROR_ID_CANNOT_COMPOSE_CENTRALIZED_ASSET => Error::CannotComposeCentralizedAsset,
            ERROR_ID_FAILED_TO_UNLOCK => Error::FailedToUnlock {
                shard_id: rlp.val_at(1)?,
                tracker: rlp.val_at(2)?,
                index: rlp.val_at(3)?,
                reason: rlp.val_at(4)?,
            },
            ERROR_ID_INSUFFICIENT_BALANCE => Error::InsufficientBalance {
                address: rlp.val_at(1)?,
                balance: rlp.val_at(2)?,
                cost: rlp.val_at(3)?,
            },
            ERROR_ID_INSUFFICIENT_PERMISSION => Error::InsufficientPermission,
            ERROR_ID_INVALID_ASSET_QUANTITY => Error::InvalidAssetQuantity {
                shard_id: rlp.val_at(1)?,
                tracker: rlp.val_at(2)?,
                index: rlp.val_at(3)?,
                expected: rlp.val_at(4)?,
                got: rlp.val_at(5)?,
            },
            ERROR_ID_INVALID_ASSET_TYPE => Error::InvalidAssetType(rlp.val_at(1)?),
            ERROR_ID_INVALID_DECOMPOSED_INPUT => Error::InvalidDecomposedInput {
                asset_type: rlp.val_at(1)?,
                shard_id: rlp.val_at(2)?,
                got: rlp.val_at(3)?,
            },
            ERROR_ID_INVALID_DECOMPOSED_OUTPUT => Error::InvalidDecomposedOutput {
                asset_type: rlp.val_at(1)?,
                shard_id: rlp.val_at(2)?,
                expected: rlp.val_at(3)?,
                got: rlp.val_at(4)?,
            },
            ERROR_ID_INVALID_ORIGIN_OUTPUTS => Error::InvalidOriginOutputs(rlp.val_at(1)?),
            ERROR_ID_INVALID_SCRIPT => Error::InvalidScript,
            ERROR_ID_INVALID_SEQ => Error::InvalidSeq(rlp.val_at(1)?),
            ERROR_ID_INVALID_SHARD_ID => Error::InvalidShardId(rlp.val_at(1)?),
            ERROR_ID_INVALID_TRANSFER_DESTINATION => Error::InvalidTransferDestination,
            ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER => Error::NewOwnersMustContainSender,
            ERROR_ID_NOT_APPROVED => Error::NotApproved(rlp.val_at(1)?),
            ERROR_ID_REGULAR_KEY_ALREADY_IN_USE => Error::RegularKeyAlreadyInUse,
            ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM => Error::RegularKeyAlreadyInUseAsPlatformAccount,
            ERROR_ID_SCRIPT_HASH_MISMATCH => Error::ScriptHashMismatch(rlp.val_at(1)?),
            ERROR_ID_SCRIPT_NOT_ALLOWED => Error::ScriptNotAllowed(rlp.val_at(1)?),
            ERROR_ID_TEXT_NOT_EXIST => Error::TextNotExist,
            ERROR_ID_TEXT_VERIFICATION_FAIL => Error::TextVerificationFail(rlp.val_at(1)?),
            ERROR_ID_CANNOT_USE_MASTER_KEY => Error::CannotUseMasterKey,
            _ => return Err(DecoderError::Custom("Invalid RuntimeError")),
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
            Error::AssetSupplyOverflow => write!(f, "Asset supply should not be overflowed"),
            Error::CannotBurnCentralizedAsset => write!(f, "Cannot burn the centralized asset"),
            Error::CannotComposeCentralizedAsset => write!(f, "Cannot compose the centralized asset"),
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
            Error::InvalidAssetType(addr) => write!(f, "Asset type is invalid: {}", addr),
            Error::InvalidDecomposedInput {
                asset_type,
                shard_id,
                got,
            } => write!(
                f,
                "The inputs are not valid. The quantity of asset({}, shard #{}) input must be 1, but {}.",
                asset_type, shard_id, got
            ),
            Error::InvalidDecomposedOutput {
                asset_type,
                shard_id,
                expected,
                got,
            } => write!(
                f,
                "The decomposed output is not balid. The quantity of asset({}, shard #{}) must be {}, but {}.",
                asset_type, shard_id, expected, got
            ),
            Error::InvalidOriginOutputs(addr) => write!(f, "origin_outputs of order({}) is not satisfied", addr),
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
        }
    }
}


#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
pub enum UnlockFailureReason {
    ScriptShouldBeBurnt,
    ScriptShouldNotBeBurnt,
    ScriptError,
}

const FAILURE_REASON_ID_SCRIPT_SHOULD_BE_BURNT: u8 = 1u8;
const FAILURE_REASON_ID_SCRIPT_SHOULD_NOT_BE_BURNT: u8 = 2u8;
const FAILURE_REASON_ID_SCRIPT_ERROR: u8 = 3u8;

impl Encodable for UnlockFailureReason {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            UnlockFailureReason::ScriptShouldBeBurnt => FAILURE_REASON_ID_SCRIPT_SHOULD_BE_BURNT.rlp_append(s),
            UnlockFailureReason::ScriptShouldNotBeBurnt => FAILURE_REASON_ID_SCRIPT_SHOULD_NOT_BE_BURNT.rlp_append(s),
            UnlockFailureReason::ScriptError => FAILURE_REASON_ID_SCRIPT_ERROR.rlp_append(s),
        };
    }
}

impl Decodable for UnlockFailureReason {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(match Decodable::decode(rlp)? {
            FAILURE_REASON_ID_SCRIPT_SHOULD_BE_BURNT => UnlockFailureReason::ScriptShouldBeBurnt,
            FAILURE_REASON_ID_SCRIPT_SHOULD_NOT_BE_BURNT => UnlockFailureReason::ScriptShouldNotBeBurnt,
            FAILURE_REASON_ID_SCRIPT_ERROR => UnlockFailureReason::ScriptError,
            _ => return Err(DecoderError::Custom("Invalid failure reason tag")),
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
