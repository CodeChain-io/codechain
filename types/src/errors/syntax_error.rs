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
use crate::{ShardId, Tracker};
use ckey::NetworkId;
use primitives::H160;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use std::fmt::{Display, Formatter, Result as FormatResult};

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

#[derive(Clone, Copy)]
#[repr(u8)]
enum ErrorID {
    DuplicatedPreviousOutput = 1,
    /// Deprecated
    // EMPTY_INPUT = 2,
    // EMPTY_OUTPUT = 3,
    EmptyShardOwners = 4,
    InconsistentTransactionInOut = 5,
    InsufficientFee = 7,
    InvalidAssetType = 8,
    /// Deprecated
    // INVALID_COMPOSED_OUTPUT_AMOUNT = 9,
    // INVALID_DECOMPOSED_INPUT_AMOUNT = 10,
    InvalidNetworkID = 11,
    InvalidApproval = 21,
    MetadataTooBig = 22,
    TextContentTooBig = 24,
    TooManyOutputs = 26,
    TxIsTooBig = 27,
    ZeroQuantity = 28,
    CannotChangeWCCCAssetScheme = 29,
    DisabledTransaction = 30,
    InvalidSignerOfWRAPCCC = 31,
    InvalidCustomAction = 32,
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
            1u8 => Ok(ErrorID::DuplicatedPreviousOutput),
            4 => Ok(ErrorID::EmptyShardOwners),
            5 => Ok(ErrorID::InconsistentTransactionInOut),
            7 => Ok(ErrorID::InsufficientFee),
            8 => Ok(ErrorID::InvalidAssetType),
            11 => Ok(ErrorID::InvalidNetworkID),
            21 => Ok(ErrorID::InvalidApproval),
            22 => Ok(ErrorID::MetadataTooBig),
            24 => Ok(ErrorID::TextContentTooBig),
            26 => Ok(ErrorID::TooManyOutputs),
            27 => Ok(ErrorID::TxIsTooBig),
            28 => Ok(ErrorID::ZeroQuantity),
            29 => Ok(ErrorID::CannotChangeWCCCAssetScheme),
            30 => Ok(ErrorID::DisabledTransaction),
            31 => Ok(ErrorID::InvalidSignerOfWRAPCCC),
            32 => Ok(ErrorID::InvalidCustomAction),
            _ => Err(DecoderError::Custom("Unexpected ErrorID Value")),
        }
    }
}


struct RlpHelper;
impl TaggedRlp for RlpHelper {
    type Tag = ErrorID;

    fn length_of(tag: ErrorID) -> Result<usize, DecoderError> {
        Ok(match tag {
            ErrorID::DuplicatedPreviousOutput => 3,
            ErrorID::EmptyShardOwners => 2,
            ErrorID::InconsistentTransactionInOut => 1,
            ErrorID::InsufficientFee => 3,
            ErrorID::InvalidAssetType => 2,
            ErrorID::InvalidCustomAction => 2,
            ErrorID::InvalidNetworkID => 2,
            ErrorID::InvalidApproval => 2,
            ErrorID::MetadataTooBig => 1,
            ErrorID::TextContentTooBig => 1,
            ErrorID::TooManyOutputs => 2,
            ErrorID::TxIsTooBig => 1,
            ErrorID::ZeroQuantity => 1,
            ErrorID::CannotChangeWCCCAssetScheme => 1,
            ErrorID::DisabledTransaction => 1,
            ErrorID::InvalidSignerOfWRAPCCC => 1,
        })
    }
}

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::DuplicatedPreviousOutput {
                tracker,
                index,
            } => RlpHelper::new_tagged_list(s, ErrorID::DuplicatedPreviousOutput).append(tracker).append(index),
            Error::EmptyShardOwners(shard_id) => {
                RlpHelper::new_tagged_list(s, ErrorID::EmptyShardOwners).append(shard_id)
            }
            Error::InconsistentTransactionInOut => RlpHelper::new_tagged_list(s, ErrorID::InconsistentTransactionInOut),
            Error::InsufficientFee {
                minimal,
                got,
            } => RlpHelper::new_tagged_list(s, ErrorID::InsufficientFee).append(minimal).append(got),
            Error::InvalidAssetType(addr) => RlpHelper::new_tagged_list(s, ErrorID::InvalidAssetType).append(addr),
            Error::InvalidCustomAction(err) => RlpHelper::new_tagged_list(s, ErrorID::InvalidCustomAction).append(err),
            Error::InvalidNetworkId(network_id) => {
                RlpHelper::new_tagged_list(s, ErrorID::InvalidNetworkID).append(network_id)
            }
            Error::InvalidApproval(err) => RlpHelper::new_tagged_list(s, ErrorID::InvalidApproval).append(err),
            Error::MetadataTooBig => RlpHelper::new_tagged_list(s, ErrorID::MetadataTooBig),
            Error::TextContentTooBig => RlpHelper::new_tagged_list(s, ErrorID::TextContentTooBig),
            Error::TooManyOutputs(num) => RlpHelper::new_tagged_list(s, ErrorID::TooManyOutputs).append(num),
            Error::TransactionIsTooBig => RlpHelper::new_tagged_list(s, ErrorID::TxIsTooBig),
            Error::ZeroQuantity => RlpHelper::new_tagged_list(s, ErrorID::ZeroQuantity),
            Error::CannotChangeWcccAssetScheme => RlpHelper::new_tagged_list(s, ErrorID::CannotChangeWCCCAssetScheme),
            Error::DisabledTransaction => RlpHelper::new_tagged_list(s, ErrorID::DisabledTransaction),
            Error::InvalidSignerOfWrapCCC => RlpHelper::new_tagged_list(s, ErrorID::InvalidSignerOfWRAPCCC),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at(0)?;
        let error = match tag {
            ErrorID::DuplicatedPreviousOutput => Error::DuplicatedPreviousOutput {
                tracker: rlp.val_at(1)?,
                index: rlp.val_at(2)?,
            },
            ErrorID::EmptyShardOwners => Error::EmptyShardOwners(rlp.val_at(1)?),
            ErrorID::InconsistentTransactionInOut => Error::InconsistentTransactionInOut,
            ErrorID::InsufficientFee => Error::InsufficientFee {
                minimal: rlp.val_at(1)?,
                got: rlp.val_at(2)?,
            },
            ErrorID::InvalidAssetType => Error::InvalidAssetType(rlp.val_at(1)?),
            ErrorID::InvalidCustomAction => Error::InvalidCustomAction(rlp.val_at(1)?),
            ErrorID::InvalidNetworkID => Error::InvalidNetworkId(rlp.val_at(1)?),
            ErrorID::InvalidApproval => Error::InvalidApproval(rlp.val_at(1)?),
            ErrorID::MetadataTooBig => Error::MetadataTooBig,
            ErrorID::TextContentTooBig => Error::TextContentTooBig,
            ErrorID::TooManyOutputs => Error::TooManyOutputs(rlp.val_at(1)?),
            ErrorID::TxIsTooBig => Error::TransactionIsTooBig,
            ErrorID::ZeroQuantity => Error::ZeroQuantity,
            ErrorID::CannotChangeWCCCAssetScheme => Error::CannotChangeWcccAssetScheme,
            ErrorID::DisabledTransaction => Error::DisabledTransaction,
            ErrorID::InvalidSignerOfWRAPCCC => Error::InvalidSignerOfWrapCCC,
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
