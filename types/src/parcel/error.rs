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

use ckey::{Address, Error as KeyError};
use primitives::{H256, U256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::transaction::Error as TransactionError;
use super::super::util::unexpected::Mismatch;
use super::super::ShardId;

#[derive(Debug, PartialEq, Clone, Serialize)]
/// Errors concerning parcel processing.
pub enum Error {
    /// Parcel is already imported to the queue
    ParcelAlreadyImported,
    /// Transaction is already imported in blockchain
    TransactionAlreadyImported,
    /// Parcel is not valid anymore (state already has higher nonce)
    Old,
    /// Parcel has too low fee
    /// (there is already a parcel with the same sender-nonce but higher gas price)
    TooCheapToReplace,
    /// Invalid network ID given.
    InvalidNetworkId,
    /// Max metadata size is exceeded.
    MetadataTooBig,
    /// Parcel was not imported to the queue because limit has been reached.
    LimitReached,
    /// Parcel's fee is below currently set minimal fee requirement.
    InsufficientFee {
        /// Minimal expected fee
        minimal: U256,
        /// Parcel fee
        got: U256,
    },
    /// Sender doesn't have enough funds to pay for this Parcel
    InsufficientBalance {
        address: Address,
        /// Senders balance
        balance: U256,
        /// Parcel cost
        cost: U256,
    },
    /// Returned when parcel nonce does not match state nonce.
    InvalidNonce {
        /// Nonce expected.
        expected: U256,
        /// Nonce found.
        got: U256,
    },
    InvalidShardId(ShardId),
    InvalidShardRoot(Mismatch<H256>),
    /// Signature error
    InvalidSignature(String),
    InconsistentShardOutcomes,
    ParcelsTooBig,
    RegularKeyAlreadyInUse,
    RegularKeyAlreadyInUseAsMaster,
    InvalidTransferDestination,
    /// Transaction error
    InvalidTransaction(TransactionError),
    InsufficientPermission,
    NewOwnersMustContainSender,
}

const ERROR_ID_PARCEL_ALREADY_IMPORTED: u8 = 1u8;
const ERROR_ID_TRANSACTION_ALREADY_IMPORTED: u8 = 2u8;
const ERROR_ID_OLD: u8 = 3u8;
const ERROR_ID_TOO_CHEAP_TO_REPLACE: u8 = 4u8;
const ERROR_ID_INVALID_NETWORK_ID: u8 = 5u8;
const ERROR_ID_METADATA_TOO_BIG: u8 = 6u8;
const ERROR_ID_LIMIT_REACHED: u8 = 7u8;
const ERROR_ID_INSUFFICIENT_FEE: u8 = 8u8;
const ERROR_ID_INSUFFICIENT_BALANCE: u8 = 9u8;
const ERROR_ID_INVALID_NONCE: u8 = 10u8;
const ERROR_ID_INVALID_SHARD_ID: u8 = 11u8;
const ERROR_ID_INVALID_SHARD_ROOT: u8 = 12u8;
const ERROR_ID_INVALID_SIGNATURE: u8 = 14u8;
const ERROR_ID_INCONSISTENT_SHARD_OUTCOMES: u8 = 15u8;
const ERROR_ID_PARCELS_TOO_BIG: u8 = 16u8;
const ERROR_ID_REGULAR_KEY_ALREADY_IN_USE: u8 = 17u8;
const ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_MASTER: u8 = 18u8;
const ERROR_ID_INVALID_TRANSFER_DESTINATION: u8 = 19u8;
const ERROR_ID_INVALID_TRANSACTION: u8 = 20u8;
const ERROR_ID_INSUFFICIENT_PERMISSION: u8 = 21u8;
const ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER: u8 = 22u8;

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::ParcelAlreadyImported => s.begin_list(1).append(&ERROR_ID_PARCEL_ALREADY_IMPORTED),
            Error::TransactionAlreadyImported => s.begin_list(1).append(&ERROR_ID_TRANSACTION_ALREADY_IMPORTED),
            Error::Old => s.begin_list(1).append(&ERROR_ID_OLD),
            Error::TooCheapToReplace => s.begin_list(1).append(&ERROR_ID_TOO_CHEAP_TO_REPLACE),
            Error::InvalidNetworkId => s.begin_list(1).append(&ERROR_ID_INVALID_NETWORK_ID),
            Error::MetadataTooBig => s.begin_list(1).append(&ERROR_ID_METADATA_TOO_BIG),
            Error::LimitReached => s.begin_list(1).append(&ERROR_ID_LIMIT_REACHED),
            Error::InsufficientFee {
                minimal,
                got,
            } => s.begin_list(3).append(&ERROR_ID_INSUFFICIENT_FEE).append(minimal).append(got),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => s.begin_list(4).append(&ERROR_ID_INSUFFICIENT_BALANCE).append(address).append(balance).append(cost),
            Error::InvalidNonce {
                expected,
                got,
            } => s.begin_list(3).append(&ERROR_ID_INVALID_NONCE).append(expected).append(got),
            Error::InvalidShardId(shard_id) => s.begin_list(2).append(&ERROR_ID_INVALID_SHARD_ID).append(shard_id),
            Error::InvalidShardRoot(mismatch) => s.begin_list(2).append(&ERROR_ID_INVALID_SHARD_ROOT).append(mismatch),
            Error::InvalidSignature(err) => s.begin_list(2).append(&ERROR_ID_INVALID_SIGNATURE).append(err),
            Error::InconsistentShardOutcomes => s.begin_list(1).append(&ERROR_ID_INCONSISTENT_SHARD_OUTCOMES),
            Error::ParcelsTooBig => s.begin_list(1).append(&ERROR_ID_PARCELS_TOO_BIG),
            Error::RegularKeyAlreadyInUse => s.begin_list(1).append(&ERROR_ID_REGULAR_KEY_ALREADY_IN_USE),
            Error::RegularKeyAlreadyInUseAsMaster => {
                s.begin_list(1).append(&ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_MASTER)
            }
            Error::InvalidTransferDestination => s.begin_list(1).append(&ERROR_ID_INVALID_TRANSFER_DESTINATION),
            Error::InvalidTransaction(err) => s.begin_list(2).append(&ERROR_ID_INVALID_TRANSACTION).append(err),
            Error::InsufficientPermission => s.begin_list(1).append(&ERROR_ID_INSUFFICIENT_PERMISSION),
            Error::NewOwnersMustContainSender => s.begin_list(1).append(&ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        Ok(match tag {
            ERROR_ID_PARCEL_ALREADY_IMPORTED => Error::ParcelAlreadyImported,
            ERROR_ID_TRANSACTION_ALREADY_IMPORTED => Error::TransactionAlreadyImported,
            ERROR_ID_OLD => Error::Old,
            ERROR_ID_TOO_CHEAP_TO_REPLACE => Error::TooCheapToReplace,
            ERROR_ID_INVALID_NETWORK_ID => Error::InvalidNetworkId,
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
            ERROR_ID_INVALID_NONCE => Error::InvalidNonce {
                expected: rlp.val_at(1)?,
                got: rlp.val_at(2)?,
            },
            ERROR_ID_INVALID_SHARD_ID => Error::InvalidShardId(rlp.val_at(1)?),
            ERROR_ID_INVALID_SHARD_ROOT => Error::InvalidShardRoot(rlp.val_at(1)?),
            ERROR_ID_INVALID_SIGNATURE => Error::InvalidSignature(rlp.val_at(1)?),
            ERROR_ID_INCONSISTENT_SHARD_OUTCOMES => Error::InconsistentShardOutcomes,
            ERROR_ID_PARCELS_TOO_BIG => Error::ParcelsTooBig,
            ERROR_ID_REGULAR_KEY_ALREADY_IN_USE => Error::RegularKeyAlreadyInUse,
            ERROR_ID_REGULAR_KEY_ALREADY_IN_USE_AS_MASTER => Error::RegularKeyAlreadyInUseAsMaster,
            ERROR_ID_INVALID_TRANSFER_DESTINATION => Error::InvalidTransferDestination,
            ERROR_ID_INVALID_TRANSACTION => Error::InvalidTransaction(rlp.val_at(1)?),
            ERROR_ID_INSUFFICIENT_PERMISSION => Error::InsufficientPermission,
            ERROR_ID_NEW_OWNERS_MUST_CONTAIN_SENDER => Error::NewOwnersMustContainSender,
            _ => return Err(DecoderError::Custom("Invalid parcel error")),
        })
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        let msg: String = match self {
            Error::ParcelAlreadyImported => "The parcel is already imported".into(),
            Error::TransactionAlreadyImported => "The transaction is already imported".into(),
            Error::Old => "No longer valid".into(),
            Error::TooCheapToReplace => "Fee too low to replace".into(),
            Error::InvalidNetworkId => "This network ID is not allowed on this chain".into(),
            Error::MetadataTooBig => "Metadata size is too big.".into(),
            Error::LimitReached => "Parcel limit reached".into(),
            Error::InsufficientFee {
                minimal,
                got,
            } => format!("Insufficient fee. Min={}, Given={}", minimal, got),
            Error::InsufficientBalance {
                address,
                balance,
                cost,
            } => format!("{} has only {:?} but it must be larger than {:?}", address, balance, cost),
            Error::InvalidNonce {
                expected,
                got,
            } => format!("Invalid parcel nonce: expected {}, found {}", expected, got),
            Error::InvalidShardId(shard_id) => format!("{} is an invalid shard id", shard_id),
            Error::InvalidShardRoot(mismatch) => format!("Invalid shard root {}", mismatch),
            Error::InvalidSignature(err) => format!("Parcel has invalid signature: {}.", err),
            Error::InconsistentShardOutcomes => "Shard outcomes are inconsistent".to_string(),
            Error::ParcelsTooBig => "Parcel size exceeded the body size limit".to_string(),
            Error::RegularKeyAlreadyInUse => "The regular key is already registered to another account".to_string(),
            Error::RegularKeyAlreadyInUseAsMaster => "The regular key is already used as a master account".to_string(),
            Error::InvalidTransferDestination => "Transfer receiver is not valid account".to_string(),
            Error::InvalidTransaction(err) => format!("Parcel has an invalid transaction: {}", err).to_string(),
            Error::InsufficientPermission => "Sender doesn't have a permission".to_string(),
            Error::NewOwnersMustContainSender => "New owners must contain the sender".to_string(),
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
