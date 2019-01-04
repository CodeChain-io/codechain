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
use primitives::H256;
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
    InvalidShardRoot(Mismatch<H256>),
    ZeroAmount,
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
const ERROR_ID_INVALID_SHARD_ROOT: u8 = 12u8;
const ERROR_ID_INVALID_SIGNATURE: u8 = 14u8;
const ERROR_ID_INCONSISTENT_SHARD_OUTCOMES: u8 = 15u8;
const ERROR_ID_TX_IS_TOO_BIG: u8 = 16u8;
const ERROR_ID_REGULAR_KEY_ALREADY_IN_USE: u8 = 17u8;
const ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM: u8 = 18u8;
const ERROR_ID_INVALID_TRANSFER_DESTINATION: u8 = 19u8;
const ERROR_ID_INVALID_TRANSACTION: u8 = 20u8;
const ERROR_ID_INSUFFICIENT_PERMISSION: u8 = 21u8;
const ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER: u8 = 22u8;
const ERROR_ID_ZERO_AMOUNT: u8 = 23u8;
const ERROR_ID_TEXT_VERIFICATION_FAIL: u8 = 24u8;
const ERROR_ID_TEXT_NOT_EXIST: u8 = 25u8;
const ERROR_ID_TEXT_CONTENT_TOO_BIG: u8 = 26u8;

impl Error {
    fn item_count(&self) -> usize {
        match self {
            Error::TransactionAlreadyImported => 1,
            Error::Old => 1,
            Error::TooCheapToReplace => 1,
            Error::InvalidNetworkId(_) => 2,
            Error::MetadataTooBig => 1,
            Error::LimitReached => 1,
            Error::InsufficientFee {
                ..
            } => 3,
            Error::InsufficientBalance {
                ..
            } => 4,
            Error::InvalidSeq(_) => 2,
            Error::InvalidShardId(_) => 2,
            Error::InvalidShardRoot(_) => 2,
            Error::ZeroAmount => 1,
            Error::InvalidSignature(_) => 2,
            Error::InconsistentShardOutcomes => 1,
            Error::TransactionIsTooBig => 1,
            Error::RegularKeyAlreadyInUse => 1,
            Error::RegularKeyAlreadyInUseAsPlatformAccount => 1,
            Error::InvalidTransferDestination => 1,
            Error::InvalidTransaction(_) => 2,
            Error::InsufficientPermission => 1,
            Error::NewOwnersMustContainSender => 1,
            Error::TextVerificationFail(_) => 2,
            Error::TextNotExist => 1,
            Error::TextContentTooBig => 1,
        }
    }
}
impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.item_count());
        match self {
            Error::TransactionAlreadyImported => s.append(&ERROR_ID_TX_ALREADY_IMPORTED),
            Error::Old => s.append(&ERROR_ID_OLD),
            Error::TooCheapToReplace => s.append(&ERROR_ID_TOO_CHEAP_TO_REPLACE),
            Error::InvalidNetworkId(network_id) => s.append(&ERROR_ID_INVALID_NETWORK_ID).append(network_id),
            Error::MetadataTooBig => s.append(&ERROR_ID_METADATA_TOO_BIG),
            Error::LimitReached => s.append(&ERROR_ID_LIMIT_REACHED),
            Error::InsufficientFee {
                minimal,
                got,
            } => s.append(&ERROR_ID_INSUFFICIENT_FEE).append(minimal).append(got),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => s.append(&ERROR_ID_INSUFFICIENT_BALANCE).append(address).append(balance).append(cost),
            Error::InvalidSeq(mismatch) => s.append(&ERROR_ID_INVALID_SEQ).append(mismatch),
            Error::InvalidShardId(shard_id) => s.append(&ERROR_ID_INVALID_SHARD_ID).append(shard_id),
            Error::InvalidShardRoot(mismatch) => s.append(&ERROR_ID_INVALID_SHARD_ROOT).append(mismatch),
            Error::ZeroAmount => s.append(&ERROR_ID_ZERO_AMOUNT),
            Error::InvalidSignature(err) => s.append(&ERROR_ID_INVALID_SIGNATURE).append(err),
            Error::InconsistentShardOutcomes => s.append(&ERROR_ID_INCONSISTENT_SHARD_OUTCOMES),
            Error::TransactionIsTooBig => s.append(&ERROR_ID_TX_IS_TOO_BIG),
            Error::RegularKeyAlreadyInUse => s.append(&ERROR_ID_REGULAR_KEY_ALREADY_IN_USE),
            Error::RegularKeyAlreadyInUseAsPlatformAccount => {
                s.append(&ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_PLATFORM)
            }
            Error::InvalidTransferDestination => s.append(&ERROR_ID_INVALID_TRANSFER_DESTINATION),
            Error::InvalidTransaction(err) => s.append(&ERROR_ID_INVALID_TRANSACTION).append(err),
            Error::InsufficientPermission => s.append(&ERROR_ID_INSUFFICIENT_PERMISSION),
            Error::NewOwnersMustContainSender => s.append(&ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER),
            Error::TextVerificationFail(err) => s.append(&ERROR_ID_TEXT_VERIFICATION_FAIL).append(err),
            Error::TextNotExist => s.append(&ERROR_ID_TEXT_NOT_EXIST),
            Error::TextContentTooBig => s.append(&ERROR_ID_TEXT_CONTENT_TOO_BIG),
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
                cost: rlp.val_at(2)?,
            },
            ERROR_ID_INVALID_SEQ => Error::InvalidSeq(rlp.val_at(1)?),
            ERROR_ID_INVALID_SHARD_ID => Error::InvalidShardId(rlp.val_at(1)?),
            ERROR_ID_INVALID_SHARD_ROOT => Error::InvalidShardRoot(rlp.val_at(1)?),
            ERROR_ID_ZERO_AMOUNT => Error::ZeroAmount,
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
        if rlp.item_count()? != error.item_count() {
            return Err(DecoderError::RlpInvalidLength)
        }
        Ok(error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        let msg: String = match self {
            Error::TransactionAlreadyImported => "The transaction is already imported".into(),
            Error::Old => "No longer valid".into(),
            Error::TooCheapToReplace => "Fee too low to replace".into(),
            Error::InvalidNetworkId(network_id) => format!("{} is an invalid network id", network_id),
            Error::MetadataTooBig => "Metadata size is too big.".into(),
            Error::LimitReached => "Transaction limit reached".into(),
            Error::InsufficientFee {
                minimal,
                got,
            } => format!("Insufficient fee. Min={}, Given={}", minimal, got),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => format!("{} has only {:?} but it must be larger than {:?}", address, balance, cost),
            Error::InvalidSeq(mismatch) => format!("Invalid transaction seq {}", mismatch),
            Error::InvalidShardId(shard_id) => format!("{} is an invalid shard id", shard_id),
            Error::InvalidShardRoot(mismatch) => format!("Invalid shard root {}", mismatch),
            Error::ZeroAmount => "An amount cannot be 0".to_string(),
            Error::InvalidSignature(err) => format!("Transaction has invalid signature: {}.", err),
            Error::InconsistentShardOutcomes => "Shard outcomes are inconsistent".to_string(),
            Error::TransactionIsTooBig => "Transaction size exceeded the body size limit".to_string(),
            Error::RegularKeyAlreadyInUse => "The regular key is already registered to another account".to_string(),
            Error::RegularKeyAlreadyInUseAsPlatformAccount => {
                "The regular key is already used as a platform account".to_string()
            }
            Error::InvalidTransferDestination => "Transfer receiver is not valid account".to_string(),
            Error::InvalidTransaction(err) => format!("Transaction has an invalid transaction: {}", err),
            Error::InsufficientPermission => "Sender doesn't have a permission".to_string(),
            Error::NewOwnersMustContainSender => "New owners must contain the sender".to_string(),
            Error::TextVerificationFail(err) => format!("Text verification has failed: {}", err),
            Error::TextNotExist => "The text does not exist".to_string(),
            Error::TextContentTooBig => "The content of the text is too big".into(),
        };

        f.write_fmt(format_args!("Parcel error ({})", msg))
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
