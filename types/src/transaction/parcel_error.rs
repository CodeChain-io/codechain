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

use ckey::{Address, Error as KeyError, NetworkId};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::transaction::Error as TransactionError;
use crate::util::unexpected::Mismatch;
use crate::ShardId;

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(tag = "type", content = "content")]
/// Errors concerning transaction processing.
pub enum Error {
    /// Transaction is already imported to the queue
    TransactionAlreadyImported,
    /// Transaction is not valid anymore (state already has higher seq)
    Old,
    /// Transction has too low fee
    /// (there is already a transaction with the same sender-seq but higher gas price)
    TooCheapToReplace,
    /// Invalid network ID given.
    InvalidNetworkId(NetworkId),
    /// Max metadata size is exceeded.
    MetadataTooBig,
    /// Transaction was not imported to the queue because limit has been reached.
    LimitReached,
    /// Transaction's fee is below currently set minimal fee requirement.
    InsufficientFee {
        /// Minimal expected fee
        minimal: u64,
        /// Transaction fee
        got: u64,
    },
    /// Sender doesn't have enough funds to pay for this Transaction
    InsufficientBalance {
        address: Address,
        /// Senders balance
        balance: u64,
        /// Transaction cost
        cost: u64,
    },
    /// Returned when transaction seq does not match state seq
    InvalidSeq(Mismatch<u64>),
    InvalidShardId(ShardId),
    ZeroQuantity,
    /// Signature error
    InvalidSignature(String),
    InconsistentShardOutcomes,
    TransactionIsTooBig,
    RegularKeyAlreadyInUse,
    RegularKeyAlreadyInUseAsPlatformAccount,
    InvalidTransferDestination,
    /// Transaction error
    InvalidTransaction(TransactionError),
    InsufficientPermission,
    NewOwnersMustContainSender,
    /// Store/Remove Text error
    TextVerificationFail(String),
    TextNotExist,
    TextContentTooBig,
}

const ERROR_ID_TX_ALREADY_IMPORTED: u8 = 1u8;
const ERROR_ID_OLD: u8 = 3u8;
const ERROR_ID_TOO_CHEAP_TO_REPLACE: u8 = 4u8;
const ERROR_ID_INVALID_NETWORK_ID: u8 = 5u8;
const ERROR_ID_METADATA_TOO_BIG: u8 = 6u8;
const ERROR_ID_LIMIT_REACHED: u8 = 7u8;
const ERROR_ID_INSUFFICIENT_FEE: u8 = 8u8;
const ERROR_ID_INSUFFICIENT_BALANCE: u8 = 9u8;
const ERROR_ID_INVALID_SEQ: u8 = 10u8;
const ERROR_ID_INVALID_SHARD_ID: u8 = 11u8;
const ERROR_ID_INVALID_SIGNATURE: u8 = 14u8;
const ERROR_ID_INCONSISTENT_SHARD_OUTCOMES: u8 = 15u8;
const ERROR_ID_TX_IS_TOO_BIG: u8 = 16u8;
const ERROR_ID_REGULAR_KEY_ALREADY_IN_USE: u8 = 17u8;
const ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM: u8 = 18u8;
const ERROR_ID_INVALID_TRANSFER_DESTINATION: u8 = 19u8;
const ERROR_ID_INVALID_TRANSACTION: u8 = 20u8;
const ERROR_ID_INSUFFICIENT_PERMISSION: u8 = 21u8;
const ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER: u8 = 22u8;
const ERROR_ID_ZERO_QUANTITY: u8 = 23u8;
const ERROR_ID_TEXT_VERIFICATION_FAIL: u8 = 24u8;
const ERROR_ID_TEXT_NOT_EXIST: u8 = 25u8;
const ERROR_ID_TEXT_CONTENT_TOO_BIG: u8 = 26u8;

fn list_length_for(tag: u8) -> Result<usize, DecoderError> {
    Ok(match tag {
        ERROR_ID_TX_ALREADY_IMPORTED => 1,
        ERROR_ID_OLD => 1,
        ERROR_ID_TOO_CHEAP_TO_REPLACE => 1,
        ERROR_ID_INVALID_NETWORK_ID => 2,
        ERROR_ID_METADATA_TOO_BIG => 1,
        ERROR_ID_LIMIT_REACHED => 1,
        ERROR_ID_INSUFFICIENT_FEE => 3,
        ERROR_ID_INSUFFICIENT_BALANCE => 4,
        ERROR_ID_INVALID_SEQ => 2,
        ERROR_ID_INVALID_SHARD_ID => 2,
        ERROR_ID_ZERO_QUANTITY => 1,
        ERROR_ID_INVALID_SIGNATURE => 2,
        ERROR_ID_INCONSISTENT_SHARD_OUTCOMES => 1,
        ERROR_ID_TX_IS_TOO_BIG => 1,
        ERROR_ID_REGULAR_KEY_ALREADY_IN_USE => 1,
        ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM => 1,
        ERROR_ID_INVALID_TRANSFER_DESTINATION => 1,
        ERROR_ID_INVALID_TRANSACTION => 2,
        ERROR_ID_INSUFFICIENT_PERMISSION => 1,
        ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER => 1,
        ERROR_ID_TEXT_VERIFICATION_FAIL => 2,
        ERROR_ID_TEXT_NOT_EXIST => 1,
        ERROR_ID_TEXT_CONTENT_TOO_BIG => 1,
        _ => return Err(DecoderError::Custom("Invalid parcel error")),
    })
}

fn tag_with(s: &mut RlpStream, tag: u8) -> &mut RlpStream {
    s.begin_list(list_length_for(tag).unwrap()).append(&tag)
}

fn check_tag_size(rlp: &UntrustedRlp, tag: u8) -> Result<(), DecoderError> {
    if rlp.item_count()? != list_length_for(tag)? {
        return Err(DecoderError::RlpInvalidLength)
    }
    Ok(())
}

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::TransactionAlreadyImported => tag_with(s, ERROR_ID_TX_ALREADY_IMPORTED),
            Error::Old => tag_with(s, ERROR_ID_OLD),
            Error::TooCheapToReplace => tag_with(s, ERROR_ID_TOO_CHEAP_TO_REPLACE),
            Error::InvalidNetworkId(network_id) => tag_with(s, ERROR_ID_INVALID_NETWORK_ID).append(network_id),
            Error::MetadataTooBig => tag_with(s, ERROR_ID_METADATA_TOO_BIG),
            Error::LimitReached => tag_with(s, ERROR_ID_LIMIT_REACHED),
            Error::InsufficientFee {
                minimal,
                got,
            } => tag_with(s, ERROR_ID_INSUFFICIENT_FEE).append(minimal).append(got),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => tag_with(s, ERROR_ID_INSUFFICIENT_BALANCE).append(address).append(balance).append(cost),
            Error::InvalidSeq(mismatch) => tag_with(s, ERROR_ID_INVALID_SEQ).append(mismatch),
            Error::InvalidShardId(shard_id) => tag_with(s, ERROR_ID_INVALID_SHARD_ID).append(shard_id),
            Error::ZeroQuantity => tag_with(s, ERROR_ID_ZERO_QUANTITY),
            Error::InvalidSignature(err) => tag_with(s, ERROR_ID_INVALID_SIGNATURE).append(err),
            Error::InconsistentShardOutcomes => tag_with(s, ERROR_ID_INCONSISTENT_SHARD_OUTCOMES),
            Error::TransactionIsTooBig => tag_with(s, ERROR_ID_TX_IS_TOO_BIG),
            Error::RegularKeyAlreadyInUse => tag_with(s, ERROR_ID_REGULAR_KEY_ALREADY_IN_USE),
            Error::RegularKeyAlreadyInUseAsPlatformAccount => {
                tag_with(s, ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM)
            }
            Error::InvalidTransferDestination => tag_with(s, ERROR_ID_INVALID_TRANSFER_DESTINATION),
            Error::InvalidTransaction(err) => tag_with(s, ERROR_ID_INVALID_TRANSACTION).append(err),
            Error::InsufficientPermission => tag_with(s, ERROR_ID_INSUFFICIENT_PERMISSION),
            Error::NewOwnersMustContainSender => tag_with(s, ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER),
            Error::TextVerificationFail(err) => tag_with(s, ERROR_ID_TEXT_VERIFICATION_FAIL).append(err),
            Error::TextNotExist => tag_with(s, ERROR_ID_TEXT_NOT_EXIST),
            Error::TextContentTooBig => tag_with(s, ERROR_ID_TEXT_CONTENT_TOO_BIG),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        let error = match tag {
            ERROR_ID_TX_ALREADY_IMPORTED => Error::TransactionAlreadyImported,
            ERROR_ID_OLD => Error::Old,
            ERROR_ID_TOO_CHEAP_TO_REPLACE => Error::TooCheapToReplace,
            ERROR_ID_INVALID_NETWORK_ID => Error::InvalidNetworkId(rlp.val_at(1)?),
            ERROR_ID_METADATA_TOO_BIG => Error::MetadataTooBig,
            ERROR_ID_LIMIT_REACHED => Error::LimitReached,
            ERROR_ID_INSUFFICIENT_FEE => Error::InsufficientFee {
                minimal: rlp.val_at(1)?,
                got: rlp.val_at(2)?,
            },
            ERROR_ID_INSUFFICIENT_BALANCE => Error::InsufficientBalance {
                address: rlp.val_at(1)?,
                balance: rlp.val_at(2)?,
                cost: rlp.val_at(3)?,
            },
            ERROR_ID_INVALID_SEQ => Error::InvalidSeq(rlp.val_at(1)?),
            ERROR_ID_INVALID_SHARD_ID => Error::InvalidShardId(rlp.val_at(1)?),
            ERROR_ID_ZERO_QUANTITY => Error::ZeroQuantity,
            ERROR_ID_INVALID_SIGNATURE => Error::InvalidSignature(rlp.val_at(1)?),
            ERROR_ID_INCONSISTENT_SHARD_OUTCOMES => Error::InconsistentShardOutcomes,
            ERROR_ID_TX_IS_TOO_BIG => Error::TransactionIsTooBig,
            ERROR_ID_REGULAR_KEY_ALREADY_IN_USE => Error::RegularKeyAlreadyInUse,
            ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM => Error::RegularKeyAlreadyInUseAsPlatformAccount,
            ERROR_ID_INVALID_TRANSFER_DESTINATION => Error::InvalidTransferDestination,
            ERROR_ID_INVALID_TRANSACTION => Error::InvalidTransaction(rlp.val_at(1)?),
            ERROR_ID_INSUFFICIENT_PERMISSION => Error::InsufficientPermission,
            ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER => Error::NewOwnersMustContainSender,
            ERROR_ID_TEXT_VERIFICATION_FAIL => Error::TextVerificationFail(rlp.val_at(1)?),
            ERROR_ID_TEXT_NOT_EXIST => Error::TextNotExist,
            ERROR_ID_TEXT_CONTENT_TOO_BIG => Error::TextContentTooBig,
            _ => return Err(DecoderError::Custom("Invalid parcel error")),
        };
        check_tag_size(rlp, tag)?;
        Ok(error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            Error::TransactionAlreadyImported => write!(f, "The transaction is already imported"),
            Error::Old => write!(f, "No longer valid"),
            Error::TooCheapToReplace => write!(f, "Fee too low to replace"),
            Error::InvalidNetworkId(network_id) => write!(f, "{} is an invalid network id", network_id),
            Error::MetadataTooBig => write!(f, "Metadata size is too big."),
            Error::LimitReached => write!(f, "Transaction limit reached"),
            Error::InsufficientFee {
                minimal,
                got,
            } => write!(f, "Insufficient fee. Min={}, Given={}", minimal, got),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => write!(f, "{} has only {:?} but it must be larger than {:?}", address, balance, cost),
            Error::InvalidSeq(mismatch) => write!(f, "Invalid transaction seq {}", mismatch),
            Error::InvalidShardId(shard_id) => write!(f, "{} is an invalid shard id", shard_id),
            Error::ZeroQuantity => write!(f, "A quantity cannot be 0"),
            Error::InvalidSignature(err) => write!(f, "Transaction has invalid signature: {}.", err),
            Error::InconsistentShardOutcomes => write!(f, "Shard outcomes are inconsistent"),
            Error::TransactionIsTooBig => write!(f, "Transaction size exceeded the body size limit"),
            Error::RegularKeyAlreadyInUse => write!(f, "The regular key is already registered to another account"),
            Error::RegularKeyAlreadyInUseAsPlatformAccount => {
                write!(f, "The regular key is already used as a platform account")
            }
            Error::InvalidTransferDestination => write!(f, "Transfer receiver is not valid account"),
            Error::InvalidTransaction(err) => write!(f, "Transaction has an invalid transaction: {}", err),
            Error::InsufficientPermission => write!(f, "Sender doesn't have a permission"),
            Error::NewOwnersMustContainSender => write!(f, "New owners must contain the sender"),
            Error::TextVerificationFail(err) => write!(f, "Text verification has failed: {}", err),
            Error::TextNotExist => write!(f, "The text does not exist"),
            Error::TextContentTooBig => write!(f, "The content of the text is too big"),
        }
    }
}

impl From<KeyError> for Error {
    fn from(err: KeyError) -> Self {
        Error::InvalidSignature(format!("{}", err))
    }
}

impl From<TransactionError> for Error {
    fn from(err: TransactionError) -> Self {
        Error::InvalidTransaction(err)
    }
}
