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
use primitives::H160;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

use super::TaggedRlp;
use crate::{ShardId, Tracker};

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum Error {
    /// There are burn/inputs that shares same previous output
    DuplicatedPreviousOutput {
        tracker: Tracker,
        index: usize,
    },
    EmptyShardOwners(ShardId),
    /// Returned when the sum of the transaction's inputs is different from the sum of outputs.
    InconsistentTransactionInOut,
    /// Transaction's fee is below currently set minimal fee requirement.
    InsufficientFee {
        /// Minimal expected fee
        minimal: u64,
        /// Transaction fee
        got: u64,
    },
    /// AssetType format error
    InvalidAssetType(H160),
    InvalidCustomAction(String),
    /// Invalid network ID given.
    InvalidNetworkId(NetworkId),
    InvalidApproval(String),
    /// Max metadata size is exceeded.
    MetadataTooBig,
    TextContentTooBig,
    TooManyOutputs(usize),
    TransactionIsTooBig,
    /// Returned when the quantity of either input or output is 0.
    ZeroQuantity,
    CannotChangeWcccAssetScheme,
    DisabledTransaction,
    InvalidSignerOfWrapCCC,
}

const ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT: u8 = 1;
/// Deprecated
//const ERROR_ID_EMPTY_INPUT: u8 = 2;
//const ERROR_ID_EMPTY_OUTPUT: u8 = 3;
const ERROR_ID_EMPTY_SHARD_OWNERS: u8 = 4;
const ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT: u8 = 5;
const ERROR_ID_INSUFFICIENT_FEE: u8 = 7;
const ERROR_ID_INVALID_ASSET_TYPE: u8 = 8;
/// Deprecated
//const ERROR_ID_INVALID_COMPOSED_OUTPUT_AMOUNT: u8 = 9;
//const ERROR_ID_INVALID_DECOMPOSED_INPUT_AMOUNT: u8 = 10;
const ERROR_ID_INVALID_NETWORK_ID: u8 = 11;
const ERROR_ID_INVALID_APPROVAL: u8 = 21;
const ERROR_ID_METADATA_TOO_BIG: u8 = 22;
const ERROR_ID_TEXT_CONTENT_TOO_BIG: u8 = 24;
const ERROR_ID_TOO_MANY_OUTPUTS: u8 = 26;
const ERROR_ID_TX_IS_TOO_BIG: u8 = 27;
const ERROR_ID_ZERO_QUANTITY: u8 = 28;
const ERROR_ID_CANNOT_CHANGE_WCCC_ASSET_SCHEME: u8 = 29;
const ERROR_ID_DISABLED_TRANSACTION: u8 = 30;
const ERROR_ID_INVALID_SIGNER_OF_WRAP_CCC: u8 = 31;
const ERROR_ID_INVALID_CUSTOM_ACTION: u8 = 32;

struct RlpHelper;
impl TaggedRlp for RlpHelper {
    type Tag = u8;

    fn length_of(tag: u8) -> Result<usize, DecoderError> {
        Ok(match tag {
            ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT => 3,
            ERROR_ID_EMPTY_SHARD_OWNERS => 2,
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT => 1,
            ERROR_ID_INSUFFICIENT_FEE => 3,
            ERROR_ID_INVALID_ASSET_TYPE => 2,
            ERROR_ID_INVALID_CUSTOM_ACTION => 2,
            ERROR_ID_INVALID_NETWORK_ID => 2,
            ERROR_ID_INVALID_APPROVAL => 2,
            ERROR_ID_METADATA_TOO_BIG => 1,
            ERROR_ID_TEXT_CONTENT_TOO_BIG => 1,
            ERROR_ID_TOO_MANY_OUTPUTS => 2,
            ERROR_ID_TX_IS_TOO_BIG => 1,
            ERROR_ID_ZERO_QUANTITY => 1,
            ERROR_ID_CANNOT_CHANGE_WCCC_ASSET_SCHEME => 1,
            ERROR_ID_DISABLED_TRANSACTION => 1,
            ERROR_ID_INVALID_SIGNER_OF_WRAP_CCC => 1,
            _ => return Err(DecoderError::Custom("Invalid SyntaxError")),
        })
    }
}

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::DuplicatedPreviousOutput {
                tracker,
                index,
            } => RlpHelper::new_tagged_list(s, ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT).append(tracker).append(index),
            Error::EmptyShardOwners(shard_id) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_EMPTY_SHARD_OWNERS).append(shard_id)
            }
            Error::InconsistentTransactionInOut => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT)
            }
            Error::InsufficientFee {
                minimal,
                got,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_INSUFFICIENT_FEE).append(minimal).append(got),
            Error::InvalidAssetType(addr) => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_ASSET_TYPE).append(addr),
            Error::InvalidCustomAction(err) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_CUSTOM_ACTION).append(err)
            }
            Error::InvalidNetworkId(network_id) => {
                RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_NETWORK_ID).append(network_id)
            }
            Error::InvalidApproval(err) => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_APPROVAL).append(err),
            Error::MetadataTooBig => RlpHelper::new_tagged_list(s, ERROR_ID_METADATA_TOO_BIG),
            Error::TextContentTooBig => RlpHelper::new_tagged_list(s, ERROR_ID_TEXT_CONTENT_TOO_BIG),
            Error::TooManyOutputs(num) => RlpHelper::new_tagged_list(s, ERROR_ID_TOO_MANY_OUTPUTS).append(num),
            Error::TransactionIsTooBig => RlpHelper::new_tagged_list(s, ERROR_ID_TX_IS_TOO_BIG),
            Error::ZeroQuantity => RlpHelper::new_tagged_list(s, ERROR_ID_ZERO_QUANTITY),
            Error::CannotChangeWcccAssetScheme => {
                RlpHelper::new_tagged_list(s, ERROR_ID_CANNOT_CHANGE_WCCC_ASSET_SCHEME)
            }
            Error::DisabledTransaction => RlpHelper::new_tagged_list(s, ERROR_ID_DISABLED_TRANSACTION),
            Error::InvalidSignerOfWrapCCC => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_SIGNER_OF_WRAP_CCC),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        let error = match tag {
            ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT => Error::DuplicatedPreviousOutput {
                tracker: rlp.val_at(1)?,
                index: rlp.val_at(2)?,
            },
            ERROR_ID_EMPTY_SHARD_OWNERS => Error::EmptyShardOwners(rlp.val_at(1)?),
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT => Error::InconsistentTransactionInOut,
            ERROR_ID_INSUFFICIENT_FEE => Error::InsufficientFee {
                minimal: rlp.val_at(1)?,
                got: rlp.val_at(2)?,
            },
            ERROR_ID_INVALID_ASSET_TYPE => Error::InvalidAssetType(rlp.val_at(1)?),
            ERROR_ID_INVALID_CUSTOM_ACTION => Error::InvalidCustomAction(rlp.val_at(1)?),
            ERROR_ID_INVALID_NETWORK_ID => Error::InvalidNetworkId(rlp.val_at(1)?),
            ERROR_ID_INVALID_APPROVAL => Error::InvalidApproval(rlp.val_at(1)?),
            ERROR_ID_METADATA_TOO_BIG => Error::MetadataTooBig,
            ERROR_ID_TEXT_CONTENT_TOO_BIG => Error::TextContentTooBig,
            ERROR_ID_TOO_MANY_OUTPUTS => Error::TooManyOutputs(rlp.val_at(1)?),
            ERROR_ID_TX_IS_TOO_BIG => Error::TransactionIsTooBig,
            ERROR_ID_ZERO_QUANTITY => Error::ZeroQuantity,
            ERROR_ID_CANNOT_CHANGE_WCCC_ASSET_SCHEME => Error::CannotChangeWcccAssetScheme,
            ERROR_ID_DISABLED_TRANSACTION => Error::DisabledTransaction,
            ERROR_ID_INVALID_SIGNER_OF_WRAP_CCC => Error::InvalidSignerOfWrapCCC,
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
                tracker,
                index,
            } => write!(f, "The previous output of inputs/burns are duplicated: ({}, {})", tracker, index),
            Error::EmptyShardOwners(shard_id) => write!(f, "Shard({}) must have at least one owner", shard_id),
            Error::InconsistentTransactionInOut => {
                write!(f, "The sum of the transaction's inputs is different from the sum of the transaction's outputs")
            }
            Error::InsufficientFee {
                minimal,
                got,
            } => write!(f, "Insufficient fee. Min={}, Given={}", minimal, got),
            Error::InvalidAssetType(addr) => write!(f, "Asset type is invalid: {}", addr),
            Error::InvalidCustomAction(err) => write!(f, "Invalid custom action: {}", err),
            Error::InvalidNetworkId(network_id) => write!(f, "{} is an invalid network id", network_id),
            Error::InvalidApproval(err) => write!(f, "Transaction has an invalid approval :{}", err),
            Error::MetadataTooBig => write!(f, "Metadata size is too big."),
            Error::TextContentTooBig => write!(f, "The content of the text is too big"),
            Error::TooManyOutputs(num) => write!(f, "The number of outputs is {}. It should be 126 or less.", num),
            Error::TransactionIsTooBig => write!(f, "Transaction size exceeded the body size limit"),
            Error::ZeroQuantity => write!(f, "A quantity cannot be 0"),
            Error::CannotChangeWcccAssetScheme => write!(f, "Cannot change the asset scheme of WCCC"),
            Error::DisabledTransaction => write!(f, "Used the disabled transaction"),
            Error::InvalidSignerOfWrapCCC => write!(f, "The signer of WrapCCC must be matched"),
        }
    }
}
