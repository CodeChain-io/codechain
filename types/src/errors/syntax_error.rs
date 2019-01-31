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

use ckey::NetworkId;
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::TaggedRlp;
use crate::ShardId;

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum Error {
    /// There are burn/inputs that shares same previous output
    DuplicatedPreviousOutput {
        transaction_hash: H256,
        index: usize,
    },
    /// AssetCompose requires at least 1 input.
    EmptyInput,
    EmptyOutput,
    EmptyShardOwners(ShardId),
    /// Returned when the sum of the transaction's inputs is different from the sum of outputs.
    InconsistentTransactionInOut,
    /// the input and output of tx is not consistent with its orders
    InconsistentTransactionInOutWithOrders,
    /// Transaction's fee is below currently set minimal fee requirement.
    InsufficientFee {
        /// Minimal expected fee
        minimal: u64,
        /// Transaction fee
        got: u64,
    },
    /// AssetType format error
    InvalidAssetType(H160),
    InvalidComposedOutputAmount {
        got: u64,
    },
    InvalidDecomposedInputAmount {
        asset_type: H160,
        shard_id: ShardId,
        got: u64,
    },
    /// Invalid network ID given.
    InvalidNetworkId(NetworkId),
    /// invalid asset_quantity_from, asset_quantity_to because of ratio
    InvalidOrderAssetQuantities {
        from: u64,
        to: u64,
        fee: u64,
    },
    /// two of {asset_type_from, asset_type_to, asset_type_fee) are equal
    InvalidOrderAssetTypes,
    /// The input/output indices of the order on transfer is not valid.
    InvalidOrderInOutIndices,
    /// the lock script hash of the order is different from the output
    InvalidOrderLockScriptHash(H160),
    /// the parameters of the order is different from the output
    InvalidOrderParameters(Vec<Bytes>),
    /// Errors on orders
    /// origin_outputs of order is not satisfied.
    InvalidOriginOutputs(H256),
    /// Signature error
    InvalidSignature(String),
    /// Max metadata size is exceeded.
    MetadataTooBig,
    OrderRecipientsAreSame,
    TextContentTooBig,
    /// Store Text error
    TextVerificationFail(String),
    TooManyOutputs(usize),
    TransactionIsTooBig,
    /// Returned when the quantity of either input or output is 0.
    ZeroQuantity,
}

const ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT: u8 = 1;
const ERROR_ID_EMPTY_INPUT: u8 = 2;
const ERROR_ID_EMPTY_OUTPUT: u8 = 3;
const ERROR_ID_EMPTY_SHARD_OWNERS: u8 = 4;
const ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT: u8 = 5;
const ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS: u8 = 6;
const ERROR_ID_INSUFFICIENT_FEE: u8 = 7;
const ERROR_ID_INVALID_ASSET_TYPE: u8 = 8;
const ERROR_ID_INVALID_COMPOSED_OUTPUT_AMOUNT: u8 = 9;
const ERROR_ID_INVALID_DECOMPOSED_INPUT_AMOUNT: u8 = 10;
const ERROR_ID_INVALID_NETWORK_ID: u8 = 11;
const ERROR_ID_INVALID_ORDER_ASSET_QUANTITIES: u8 = 12;
const ERROR_ID_INVALID_ORDER_ASSET_TYPES: u8 = 13;
const ERROR_ID_INVALID_ORDER_IN_OUT_INDICES: u8 = 14;
const ERROR_ID_INVALID_ORDER_LOCK_SCRIPT_HASH: u8 = 15;
const ERROR_ID_INVALID_ORDER_PARAMETERS: u8 = 16;
const ERROR_ID_INVALID_ORIGIN_OUTPUTS: u8 = 17;
const ERROR_ID_INVALID_SIGNATURE: u8 = 19;
const ERROR_ID_METADATA_TOO_BIG: u8 = 20;
const ERROR_ID_ORDER_RECIPIENTS_ARE_SAME: u8 = 21;
const ERROR_ID_TEXT_CONTENT_TOO_BIG: u8 = 22;
const ERROR_ID_TEXT_VERIFICATION_FAIL: u8 = 23;
const ERROR_ID_TOO_MANY_OUTPUTS: u8 = 24;
const ERROR_ID_TX_IS_TOO_BIG: u8 = 25;
const ERROR_ID_ZERO_QUANTITY: u8 = 26;

struct RlpHelper;
impl TaggedRlp for RlpHelper {
    type Tag = u8;

    fn length_of(tag: u8) -> Result<usize, DecoderError> {
        Ok(match tag {
            ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT => 3,
            ERROR_ID_EMPTY_INPUT => 1,
            ERROR_ID_EMPTY_OUTPUT => 1,
            ERROR_ID_EMPTY_SHARD_OWNERS => 2,
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT => 1,
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS => 1,
            ERROR_ID_INSUFFICIENT_FEE => 3,
            ERROR_ID_INVALID_ASSET_TYPE => 2,
            ERROR_ID_INVALID_COMPOSED_OUTPUT_AMOUNT => 2,
            ERROR_ID_INVALID_DECOMPOSED_INPUT_AMOUNT => 4,
            ERROR_ID_INVALID_NETWORK_ID => 2,
            ERROR_ID_INVALID_ORDER_ASSET_QUANTITIES => 4,
            ERROR_ID_INVALID_ORDER_ASSET_TYPES => 1,
            ERROR_ID_INVALID_ORDER_IN_OUT_INDICES => 1,
            ERROR_ID_INVALID_ORDER_LOCK_SCRIPT_HASH => 2,
            ERROR_ID_INVALID_ORDER_PARAMETERS => 2,
            ERROR_ID_INVALID_ORIGIN_OUTPUTS => 2,
            ERROR_ID_INVALID_SIGNATURE => 2,
            ERROR_ID_METADATA_TOO_BIG => 1,
            ERROR_ID_ORDER_RECIPIENTS_ARE_SAME => 1,
            ERROR_ID_TEXT_CONTENT_TOO_BIG => 1,
            ERROR_ID_TEXT_VERIFICATION_FAIL => 2,
            ERROR_ID_TOO_MANY_OUTPUTS => 2,
            ERROR_ID_TX_IS_TOO_BIG => 1,
            ERROR_ID_ZERO_QUANTITY => 1,
            _ => return Err(DecoderError::Custom("Invalid SyntaxError")),
        })
    }
}

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::DuplicatedPreviousOutput {
                transaction_hash,
                index,
            } => RlpHelper::new_tagged_list(s, ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT)
                .append(transaction_hash)
                .append(index),
            Error::EmptyInput => RlpHelper::new_tagged_list(s, ERROR_ID_EMPTY_INPUT),
            Error::EmptyOutput => RlpHelper::new_tagged_list(s, ERROR_ID_EMPTY_OUTPUT),
            Error::EmptyShardOwners(shard_id) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_EMPTY_SHARD_OWNERS).append(shard_id)
            }
            Error::InconsistentTransactionInOut => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT)
            }
            Error::InconsistentTransactionInOutWithOrders => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS)
            }
            Error::InsufficientFee {
                minimal,
                got,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INSUFFICIENT_FEE).append(minimal).append(got),
            Error::InvalidAssetType(addr) => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ASSET_TYPE).append(addr),
            Error::InvalidComposedOutputAmount {
                got,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_COMPOSED_OUTPUT_AMOUNT).append(got),
            Error::InvalidDecomposedInputAmount {
                asset_type,
                shard_id,
                got,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_DECOMPOSED_INPUT_AMOUNT)
                .append(asset_type)
                .append(shard_id)
                .append(got),
            Error::InvalidNetworkId(network_id) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_NETWORK_ID).append(network_id)
            }
            Error::InvalidOrderAssetQuantities {
                from,
                to,
                fee,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ORDER_ASSET_QUANTITIES)
                .append(from)
                .append(to)
                .append(fee),
            Error::InvalidOrderAssetTypes => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ORDER_ASSET_TYPES),
            Error::InvalidOrderInOutIndices => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ORDER_IN_OUT_INDICES),
            Error::InvalidOrderLockScriptHash(lock_script_hash) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ORDER_LOCK_SCRIPT_HASH).append(lock_script_hash)
            }
            Error::InvalidOrderParameters(parameters) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ORDER_PARAMETERS).append(parameters)
            }
            Error::InvalidOriginOutputs(order_hash) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ORIGIN_OUTPUTS).append(order_hash)
            }
            Error::InvalidSignature(err) => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_SIGNATURE).append(err),
            Error::MetadataTooBig => RlpHelper::new_tagged_list(s, ERROR_ID_METADATA_TOO_BIG),
            Error::OrderRecipientsAreSame => RlpHelper::new_tagged_list(s, ERROR_ID_ORDER_RECIPIENTS_ARE_SAME),
            Error::TextContentTooBig => RlpHelper::new_tagged_list(s, ERROR_ID_TEXT_CONTENT_TOO_BIG),
            Error::TextVerificationFail(err) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_TEXT_VERIFICATION_FAIL).append(err)
            }
            Error::TooManyOutputs(num) => RlpHelper::new_tagged_list(s, ERROR_ID_TOO_MANY_OUTPUTS).append(num),
            Error::TransactionIsTooBig => RlpHelper::new_tagged_list(s, ERROR_ID_TX_IS_TOO_BIG),
            Error::ZeroQuantity => RlpHelper::new_tagged_list(s, ERROR_ID_ZERO_QUANTITY),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        let error = match tag {
            ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT => Error::DuplicatedPreviousOutput {
                transaction_hash: rlp.val_at(1)?,
                index: rlp.val_at(2)?,
            },
            ERROR_ID_EMPTY_INPUT => Error::EmptyInput,
            ERROR_ID_EMPTY_OUTPUT => Error::EmptyOutput,
            ERROR_ID_EMPTY_SHARD_OWNERS => Error::EmptyShardOwners(rlp.val_at(1)?),
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT => Error::InconsistentTransactionInOut,
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT_WITH_ORDERS => Error::InconsistentTransactionInOutWithOrders,
            ERROR_ID_INSUFFICIENT_FEE => Error::InsufficientFee {
                minimal: rlp.val_at(1)?,
                got: rlp.val_at(2)?,
            },
            ERROR_ID_INVALID_ASSET_TYPE => Error::InvalidAssetType(rlp.val_at(1)?),
            ERROR_ID_INVALID_COMPOSED_OUTPUT_AMOUNT => Error::InvalidComposedOutputAmount {
                got: rlp.val_at(1)?,
            },
            ERROR_ID_INVALID_DECOMPOSED_INPUT_AMOUNT => Error::InvalidDecomposedInputAmount {
                asset_type: rlp.val_at(1)?,
                shard_id: rlp.val_at(2)?,
                got: rlp.val_at(3)?,
            },
            ERROR_ID_INVALID_NETWORK_ID => Error::InvalidNetworkId(rlp.val_at(1)?),
            ERROR_ID_INVALID_ORDER_ASSET_QUANTITIES => Error::InvalidOrderAssetQuantities {
                from: rlp.val_at(1)?,
                to: rlp.val_at(2)?,
                fee: rlp.val_at(3)?,
            },
            ERROR_ID_INVALID_ORDER_ASSET_TYPES => Error::InvalidOrderAssetTypes,
            ERROR_ID_INVALID_ORDER_IN_OUT_INDICES => Error::InvalidOrderInOutIndices,
            ERROR_ID_INVALID_ORDER_LOCK_SCRIPT_HASH => Error::InvalidOrderLockScriptHash(rlp.val_at(1)?),
            ERROR_ID_INVALID_ORDER_PARAMETERS => Error::InvalidOrderParameters(rlp.val_at(1)?),
            ERROR_ID_INVALID_ORIGIN_OUTPUTS => Error::InvalidOriginOutputs(rlp.val_at(1)?),
            ERROR_ID_INVALID_SIGNATURE => Error::InvalidSignature(rlp.val_at(1)?),
            ERROR_ID_METADATA_TOO_BIG => Error::MetadataTooBig,
            ERROR_ID_ORDER_RECIPIENTS_ARE_SAME => Error::OrderRecipientsAreSame,
            ERROR_ID_TEXT_CONTENT_TOO_BIG => Error::TextContentTooBig,
            ERROR_ID_TEXT_VERIFICATION_FAIL => Error::TextVerificationFail(rlp.val_at(1)?),
            ERROR_ID_TOO_MANY_OUTPUTS => Error::TooManyOutputs(rlp.val_at(1)?),
            ERROR_ID_TX_IS_TOO_BIG => Error::TransactionIsTooBig,
            ERROR_ID_ZERO_QUANTITY => Error::ZeroQuantity,
            _ => return Err(DecoderError::Custom("Invalid SyntaxError")),
        };
        RlpHelper::check_size(rlp, tag)?;
        Ok(error)
    }
}


impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            Error::DuplicatedPreviousOutput {
                transaction_hash,
                index,
            } => write!(f, "The previous output of inputs/burns are duplicated: ({}, {})", transaction_hash, index),
            Error::EmptyInput  => write!(f, "The input is empty"),
            Error::EmptyOutput  => writeln!(f, "The output is empty"),
            Error::EmptyShardOwners (shard_id) => write!(f, "Shard({}) must have at least one owner", shard_id),
            Error::InconsistentTransactionInOut  => write!(f, "The sum of the transaction's inputs is different from the sum of the transaction's outputs"),
            Error::InconsistentTransactionInOutWithOrders  => write!(f, "The transaction's input and output do not follow its orders"),
            Error::InsufficientFee {
                minimal,
                got,
            } => write!(f, "Insufficient fee. Min={}, Given={}", minimal, got),
            Error::InvalidAssetType (addr) => write!(f, "Asset type is invalid: {}", addr),
            Error::InvalidComposedOutputAmount {
                got,
            } => write!(f, "The composed output is note valid. The supply must be 1, but {}.", got),
            Error::InvalidDecomposedInputAmount {
                asset_type,
                shard_id,
                got,
            } => write!(f, "The inputs are not valid. The quantity of asset({}, shard #{}) input must be 1, but {}.", asset_type, shard_id, got),
            Error::InvalidNetworkId (network_id) => write!(f, "{} is an invalid network id", network_id),
            Error::InvalidOrderAssetQuantities {
                from,
                to,
                fee,
            } => write!(f, "The asset exchange ratio of the order is invalid: from:to:fee = {}:{}:{}", from, to, fee),
            Error::InvalidOrderAssetTypes => write!(f, "There are same shard asset types in the order"),
            Error::InvalidOrderInOutIndices  => write!(f, "The order on transfer is invalid because its input/output indices are wrong or overlapped with other orders"),
            Error::InvalidOrderLockScriptHash (lock_script_hash) => write!(f, "The lock script hash of the order is different from the output: {}", lock_script_hash),
            Error::InvalidOrderParameters (parameters) => write!(f, "The parameters of the order is different from the output: {:?}", parameters),
            Error::InvalidOriginOutputs (order_hash) => write!(f, "The order({}) is invalid because its origin outputs are wrong", order_hash),
            Error::InvalidSignature (err) => write!(f, "Transaction has invalid signature: {}.", err),
            Error::MetadataTooBig  => write!(f, "Metadata size is too big."),
            Error::OrderRecipientsAreSame  => write!(f, "Both the lock script hash and parameters should not be same between maker and relayer"),
            Error::TextContentTooBig  => write!(f, "The content of the text is too big"),
            Error::TextVerificationFail (err) => write!(f, "Text verification has failed: {}", err),
            Error::TooManyOutputs (num) => write!(f, "The number of outputs is {}. It should be 126 or less.", num),
            Error::TransactionIsTooBig  => write!(f, "Transaction size exceeded the body size limit"),
            Error::ZeroQuantity  => write!(f, "A quantity cannot be 0"),
        }
    }
}
