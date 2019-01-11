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

use std::fmt::{Display, Formatter, Result as FormatResult};

use ckey::Address;
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::Timelock;
use crate::util::unexpected::Mismatch;
use crate::ShardId;

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum Error {
    InvalidAssetQuantity {
        address: H256,
        expected: u64,
        got: u64,
    },
    /// Desired input asset not found
    AssetNotFound(H256),
    /// Desired input asset scheme not found
    AssetSchemeNotFound(H256),
    AssetSchemeDuplicated(H256),
    InvalidAssetType(H256),
    /// Script hash does not match with provided lock script
    ScriptHashMismatch(Mismatch<H160>),
    ScriptNotAllowed(H160),
    /// Failed to decode script
    InvalidScript,
    /// Script execution result is `Fail`
    FailedToUnlock {
        address: H256,
        reason: UnlockFailureReason,
    },
    /// Returned when the sum of the transaction's inputs is different from the sum of outputs.
    InconsistentTransactionInOut,
    /// There are burn/inputs that shares same previous output
    DuplicatedPreviousOutput {
        transaction_hash: H256,
        index: usize,
    },
    InsufficientPermission,
    EmptyShardOwners(ShardId),
    NotApproved(Address),
    /// Returned when the quantity of either input or output is 0.
    ZeroQuantity,
    TooManyOutputs(usize),
    /// AssetCompose requires at least 1 input.
    EmptyInput,
    CannotBurnCentralizedAsset,
    CannotComposeCentralizedAsset,
    InvalidDecomposedInput {
        address: H256,
        got: u64,
    },
    InvalidComposedOutput {
        got: u64,
    },
    InvalidDecomposedOutput {
        address: H256,
        expected: u64,
        got: u64,
    },
    EmptyOutput,
    Timelocked {
        timelock: Timelock,
        remaining_time: u64,
    },
    /// Errors on orders
    /// origin_outputs of order is not satisfied.
    InvalidOriginOutputs(H256),
    /// The input/output indices of the order on transfer is not valid.
    InvalidOrderInOutIndices,
    /// the input and output of tx is not consistent with its orders
    InconsistentTransactionInOutWithOrders,
    /// asset_type_from and asset_type_to is equal
    InvalidOrderAssetTypes {
        from: H256,
        to: H256,
        fee: H256,
    },
    /// invalid asset_quantity_from, asset_quantity_to because of ratio
    InvalidOrderAssetQuantities {
        from: u64,
        to: u64,
        fee: u64,
    },
    /// the lock script hash of the order is different from the output
    InvalidOrderLockScriptHash(H160),
    /// the parameters of the order is different from the output
    InvalidOrderParameters(Vec<Bytes>),
    OrderRecipientsAreSame,
    OrderExpired {
        expiration: u64,
        timestamp: u64,
    },
}

const ERROR_ID_CANNOT_BURN_CENTRALIZED_ASSET: u8 = 2u8;
const ERROR_ID_CANNOT_COMPOSE_CENTRALIZED_ASSET: u8 = 3u8;
const ERROR_ID_INVALID_ASSET_QUANTITY: u8 = 4u8;
const ERROR_ID_ASSET_NOT_FOUND: u8 = 5u8;
const ERROR_ID_ASSET_SCHEME_NOT_FOUND: u8 = 6u8;
const ERROR_ID_INVALID_ASSET_TYPE: u8 = 7u8;
const ERROR_ID_SCRIPT_HASH_MISMATCH: u8 = 8u8;
const ERROR_ID_INVALID_SCRIPT: u8 = 9u8;
const ERROR_ID_FAILED_TO_UNLOCK: u8 = 10u8;
const ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT: u8 = 11u8;
const ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT: u8 = 12u8;
const ERROR_ID_INSUFFICIENT_PERMISSION: u8 = 13u8;
const ERROR_ID_EMPTY_SHARD_OWNERS: u8 = 16u8;
const ERROR_ID_NOT_APPROVED: u8 = 17u8;
const ERROR_ID_ZERO_QUANTITY: u8 = 18u8;
const ERROR_ID_TOO_MANY_OUTPUTS: u8 = 19u8;
const ERROR_ID_ASSET_SCHEME_DUPLICATED: u8 = 20u8;
const ERROR_ID_EMPTY_INPUT: u8 = 21u8;
const ERROR_ID_INVALID_DECOMPOSED_INPUT: u8 = 22u8;
const ERROR_ID_INVALID_COMPOSED_OUTPUT: u8 = 23u8;
const ERROR_ID_INVALID_DECOMPOSED_OUTPUT: u8 = 24u8;
const ERROR_ID_EMPTY_OUTPUT: u8 = 25u8;
const ERROR_ID_TIMELOCKED: u8 = 26u8;
const ERROR_ID_INVALID_ORIGIN_OUTPUTS: u8 = 27u8;
const ERROR_ID_INVALID_ORDER_IN_OUT_INDICES: u8 = 28u8;
const ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS: u8 = 29u8;
const ERROR_ID_INVALID_ORDER_ASSET_TYPES: u8 = 30u8;
const ERROR_ID_INVALID_ORDER_ASSET_QUANTITIES: u8 = 31u8;
const ERROR_ID_INVALID_ORDER_LOCK_SCRIPT_HASH: u8 = 32u8;
const ERROR_ID_INVALID_ORDER_PARAMETERS: u8 = 33u8;
const ERROR_ID_ORDER_RECIPIENTS_ARE_SAME: u8 = 34u8;
const ERROR_ID_ORDER_EXPIRED: u8 = 35u8;
const ERROR_ID_SCRIPT_NOT_ALLOWED: u8 = 36u8;

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::InvalidAssetQuantity {
                address,
                expected,
                got,
            } => s.begin_list(4).append(&ERROR_ID_INVALID_ASSET_QUANTITY).append(address).append(expected).append(got),
            Error::AssetNotFound(addr) => s.begin_list(2).append(&ERROR_ID_ASSET_NOT_FOUND).append(addr),
            Error::AssetSchemeNotFound(addr) => s.begin_list(2).append(&ERROR_ID_ASSET_SCHEME_NOT_FOUND).append(addr),
            Error::AssetSchemeDuplicated(addr) => {
                s.begin_list(2).append(&ERROR_ID_ASSET_SCHEME_DUPLICATED).append(addr)
            }
            Error::InvalidAssetType(addr) => s.begin_list(2).append(&ERROR_ID_INVALID_ASSET_TYPE).append(addr),
            Error::ScriptHashMismatch(mismatch) => {
                s.begin_list(2).append(&ERROR_ID_SCRIPT_HASH_MISMATCH).append(mismatch)
            }
            Error::ScriptNotAllowed(hash) => s.begin_list(2).append(&ERROR_ID_SCRIPT_NOT_ALLOWED).append(hash),
            Error::InvalidScript => s.begin_list(1).append(&ERROR_ID_INVALID_SCRIPT),
            Error::FailedToUnlock {
                address,
                reason,
            } => s.begin_list(3).append(&ERROR_ID_FAILED_TO_UNLOCK).append(address).append(reason),
            Error::InconsistentTransactionInOut => s.begin_list(1).append(&ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT),
            Error::DuplicatedPreviousOutput {
                transaction_hash,
                index,
            } => s.begin_list(3).append(&ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT).append(transaction_hash).append(index),
            Error::InsufficientPermission => s.begin_list(1).append(&ERROR_ID_INSUFFICIENT_PERMISSION),
            Error::EmptyShardOwners(shard_id) => s.begin_list(2).append(&ERROR_ID_EMPTY_SHARD_OWNERS).append(shard_id),
            Error::NotApproved(address) => s.begin_list(2).append(&ERROR_ID_NOT_APPROVED).append(address),
            Error::ZeroQuantity => s.begin_list(1).append(&ERROR_ID_ZERO_QUANTITY),
            Error::TooManyOutputs(num) => s.begin_list(2).append(&ERROR_ID_TOO_MANY_OUTPUTS).append(num),
            Error::EmptyInput => s.begin_list(1).append(&ERROR_ID_EMPTY_INPUT),
            Error::CannotBurnCentralizedAsset => s.begin_list(1).append(&ERROR_ID_CANNOT_BURN_CENTRALIZED_ASSET),
            Error::CannotComposeCentralizedAsset => s.begin_list(1).append(&ERROR_ID_CANNOT_COMPOSE_CENTRALIZED_ASSET),
            Error::InvalidDecomposedInput {
                address,
                got,
            } => s.begin_list(3).append(&ERROR_ID_INVALID_DECOMPOSED_INPUT).append(address).append(got),
            Error::InvalidComposedOutput {
                got,
            } => s.begin_list(2).append(&ERROR_ID_INVALID_COMPOSED_OUTPUT).append(got),
            Error::InvalidDecomposedOutput {
                address,
                expected,
                got,
            } => {
                s.begin_list(4).append(&ERROR_ID_INVALID_DECOMPOSED_OUTPUT).append(address).append(expected).append(got)
            }
            Error::EmptyOutput => s.begin_list(1).append(&ERROR_ID_EMPTY_OUTPUT),
            Error::Timelocked {
                timelock,
                remaining_time,
            } => s.begin_list(3).append(&ERROR_ID_TIMELOCKED).append(timelock).append(remaining_time),
            Error::InvalidOriginOutputs(order_hash) => {
                s.begin_list(2).append(&ERROR_ID_INVALID_ORIGIN_OUTPUTS).append(order_hash)
            }
            Error::InvalidOrderInOutIndices => s.begin_list(1).append(&ERROR_ID_INVALID_ORDER_IN_OUT_INDICES),
            Error::InconsistentTransactionInOutWithOrders => {
                s.begin_list(1).append(&ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS)
            }
            Error::InvalidOrderAssetTypes {
                from,
                to,
                fee,
            } => s.begin_list(4).append(&ERROR_ID_INVALID_ORDER_ASSET_TYPES).append(from).append(to).append(fee),
            Error::InvalidOrderAssetQuantities {
                from,
                to,
                fee,
            } => s.begin_list(4).append(&ERROR_ID_INVALID_ORDER_ASSET_QUANTITIES).append(from).append(to).append(fee),
            Error::InvalidOrderLockScriptHash(lock_script_hash) => {
                s.begin_list(2).append(&ERROR_ID_INVALID_ORDER_LOCK_SCRIPT_HASH).append(lock_script_hash)
            }
            Error::InvalidOrderParameters(parameters) => {
                s.begin_list(2).append(&ERROR_ID_INVALID_ORDER_PARAMETERS).append(parameters)
            }
            Error::OrderRecipientsAreSame => s.begin_list(1).append(&ERROR_ID_ORDER_RECIPIENTS_ARE_SAME),
            Error::OrderExpired {
                expiration,
                timestamp,
            } => s.begin_list(3).append(&ERROR_ID_ORDER_EXPIRED).append(expiration).append(timestamp),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        Ok(match tag {
            ERROR_ID_INVALID_ASSET_QUANTITY => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidAssetQuantity {
                    address: rlp.val_at(1)?,
                    expected: rlp.val_at(2)?,
                    got: rlp.val_at(3)?,
                }
            }
            ERROR_ID_ASSET_NOT_FOUND => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::AssetNotFound(rlp.val_at(1)?)
            }
            ERROR_ID_ASSET_SCHEME_NOT_FOUND => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::AssetSchemeNotFound(rlp.val_at(1)?)
            }
            ERROR_ID_ASSET_SCHEME_DUPLICATED => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::AssetSchemeDuplicated(rlp.val_at(1)?)
            }
            ERROR_ID_INVALID_ASSET_TYPE => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidAssetType(rlp.val_at(1)?)
            }
            ERROR_ID_SCRIPT_HASH_MISMATCH => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::ScriptHashMismatch(rlp.val_at(1)?)
            }
            ERROR_ID_SCRIPT_NOT_ALLOWED => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::ScriptNotAllowed(rlp.val_at(1)?)
            }
            ERROR_ID_INVALID_SCRIPT => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidScript
            }
            ERROR_ID_FAILED_TO_UNLOCK => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::FailedToUnlock {
                    address: rlp.val_at(1)?,
                    reason: rlp.val_at(2)?,
                }
            }
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InconsistentTransactionInOut
            }
            ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::DuplicatedPreviousOutput {
                    transaction_hash: rlp.val_at(1)?,
                    index: rlp.val_at(2)?,
                }
            }
            ERROR_ID_INSUFFICIENT_PERMISSION => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InsufficientPermission
            }
            ERROR_ID_EMPTY_SHARD_OWNERS => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::EmptyShardOwners(rlp.val_at(1)?)
            }
            ERROR_ID_NOT_APPROVED => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::NotApproved(rlp.val_at(1)?)
            }
            ERROR_ID_ZERO_QUANTITY => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::ZeroQuantity
            }
            ERROR_ID_TOO_MANY_OUTPUTS => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::TooManyOutputs(rlp.val_at(1)?)
            }
            ERROR_ID_EMPTY_INPUT => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::EmptyInput
            }
            ERROR_ID_CANNOT_BURN_CENTRALIZED_ASSET => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::CannotBurnCentralizedAsset
            }
            ERROR_ID_CANNOT_COMPOSE_CENTRALIZED_ASSET => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::CannotComposeCentralizedAsset
            }
            ERROR_ID_INVALID_DECOMPOSED_INPUT => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidDecomposedInput {
                    address: rlp.val_at(1)?,
                    got: rlp.val_at(2)?,
                }
            }
            ERROR_ID_INVALID_COMPOSED_OUTPUT => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidComposedOutput {
                    got: rlp.val_at(1)?,
                }
            }
            ERROR_ID_INVALID_DECOMPOSED_OUTPUT => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidDecomposedOutput {
                    address: rlp.val_at(1)?,
                    expected: rlp.val_at(2)?,
                    got: rlp.val_at(3)?,
                }
            }
            ERROR_ID_EMPTY_OUTPUT => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::EmptyOutput
            }
            ERROR_ID_TIMELOCKED => Error::Timelocked {
                timelock: rlp.val_at(1)?,
                remaining_time: rlp.val_at(2)?,
            },
            ERROR_ID_INVALID_ORIGIN_OUTPUTS => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidOriginOutputs(rlp.val_at(1)?)
            }
            ERROR_ID_INVALID_ORDER_IN_OUT_INDICES => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidOrderInOutIndices
            }
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InconsistentTransactionInOutWithOrders
            }
            ERROR_ID_INVALID_ORDER_ASSET_TYPES => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidOrderAssetTypes {
                    from: rlp.val_at(1)?,
                    to: rlp.val_at(2)?,
                    fee: rlp.val_at(3)?,
                }
            }
            ERROR_ID_INVALID_ORDER_ASSET_QUANTITIES => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidOrderAssetQuantities {
                    from: rlp.val_at(1)?,
                    to: rlp.val_at(2)?,
                    fee: rlp.val_at(3)?,
                }
            }
            ERROR_ID_INVALID_ORDER_LOCK_SCRIPT_HASH => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidOrderLockScriptHash(rlp.val_at(1)?)
            }
            ERROR_ID_INVALID_ORDER_PARAMETERS => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidOrderParameters(rlp.val_at(1)?)
            }
            ERROR_ID_ORDER_RECIPIENTS_ARE_SAME => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::OrderRecipientsAreSame
            }
            ERROR_ID_ORDER_EXPIRED => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::OrderExpired {
                    expiration: rlp.val_at(1)?,
                    timestamp: rlp.val_at(2)?,
                }
            }
            _ => return Err(DecoderError::Custom("Invalid transaction error")),
        })
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            Error::InvalidAssetQuantity {
                address,
                expected,
                got,
            } => write!(
                f,
                "AssetTransfer must consume input asset completely. The quantity of asset({}) must be {}, but {}.",
                address, expected, got
            ),
            Error::AssetNotFound(addr) => write!(f, "Asset not found: {}", addr),
            Error::AssetSchemeNotFound(addr) => write!(f, "Asset scheme not found: {}", addr),
            Error::AssetSchemeDuplicated(addr) => write!(f, "Asset scheme already exists: {}", addr),
            Error::InvalidAssetType(addr) => write!(f, "Asset type is invalid: {}", addr),
            Error::ScriptHashMismatch(mismatch) => {
                write!(f, "Expected script with hash {}, but got {}", mismatch.expected, mismatch.found)
            }
            Error::ScriptNotAllowed(hash) => {
                write!(f, "Output lock script hash is not allowed : {}", hash)
            }
            Error::InvalidScript => write!(f, "Failed to decode script"),
            Error::FailedToUnlock {
                address,
                reason,
            } => write!(f, "Failed to unlock asset {}, reason: {}", address, reason),
            Error::InconsistentTransactionInOut => {
                write!(f, "The sum of the transaction's inputs is different from the sum of the transaction's outputs")
            }
            Error::DuplicatedPreviousOutput {
                transaction_hash,
                index,
            } => write!(f, "The previous output of inputs/burns are duplicated: ({}, {})", transaction_hash, index),
            Error::InsufficientPermission => write!(f, "The current sender doesn't have the permission"),
            Error::EmptyShardOwners(shard_id) => write!(f, "Shard({}) must have at least one owner", shard_id),
            Error::NotApproved(address) => write!(f, "{} should approve it.", address),
            Error::ZeroQuantity => write!(f, "An quantity cannot be 0"),
            Error::TooManyOutputs(num) => write!(f, "The number of outputs is {}. It should be 126 or less.", num),
            Error::EmptyInput => write!(f, "The input is empty"),
            Error::CannotBurnCentralizedAsset => write!(f, "Cannot burn the centralized asset"),
            Error::CannotComposeCentralizedAsset => write!(f, "Cannot compose the centralized asset"),
            Error::InvalidDecomposedInput {
                address,
                got,
            } => write!(f, "The inputs are not valid. The quantity of asset({}) input must be 1, but {}.", address, got),
            Error::InvalidComposedOutput {
                got,
            } => write!(f, "The composed output is note valid. The supply must be 1, but {}.", got),
            Error::InvalidDecomposedOutput {
                address,
                expected,
                got,
            } => write!(
                f,
                "The decomposed output is not balid. The quantity of asset({}) must be {}, but {}.",
                address, expected, got
            ),
            Error::EmptyOutput => writeln!(f, "The output is empty"),
            Error::Timelocked {
                timelock,
                remaining_time,
            } => write!(
                f,
                "The transaction cannot be executed because of the timelock({:?}). The remaining time is {}",
                timelock, remaining_time
            ),
            Error::InvalidOriginOutputs(order_hash) => {
                write!(f, "The order({}) is invalid because its origin outputs are wrong", order_hash)
            }
            Error::InvalidOrderInOutIndices => {
                write!(f, "The order on transfer is invalid because its input/output indices are wrong or overlapped with other orders")
            }
            Error::InconsistentTransactionInOutWithOrders => {
                write!(f, "The transaction's input and output do not follow its orders")
            }
            Error::InvalidOrderAssetTypes {
                from,
                to,
                fee,
            } => write!(f, "There are asset types in the order which are same: from:{}, to:{}, fee:{}", from, to, fee),
            Error::InvalidOrderAssetQuantities {
                from,
                to,
                fee,
            } => write!(f, "The asset exchange ratio of the order is invalid: from:to:fee = {}:{}:{}", from, to, fee),
            Error::InvalidOrderLockScriptHash(lock_script_hash) => {
                write!(f, "The lock script hash of the order is different from the output: {}", lock_script_hash)
            }
            Error::InvalidOrderParameters(parameters) => {
                write!(f, "The parameters of the order is different from the output: {:?}", parameters)
            }
            Error::OrderRecipientsAreSame => {
                write!(f, "Both the lock script hash and parameters should not be same between maker and relayer")
            }
            Error::OrderExpired {
                expiration,
                timestamp,
            } => write!(f, "The order is expired. Expiration: {}, Block timestamp: {}", expiration, timestamp),
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

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_insufficient_permission() {
        rlp_encode_and_decode_test!(Error::InsufficientPermission);
    }

    #[test]
    fn encode_and_decode_too_many_outpus() {
        rlp_encode_and_decode_test!(Error::TooManyOutputs(127));
    }
}
