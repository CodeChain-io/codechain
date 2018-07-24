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
use unexpected::Mismatch;

use super::super::ShardId;

#[derive(Debug, PartialEq, Clone)]
/// Errors concerning parcel processing.
pub enum Error {
    /// Parcel is already imported to the queue
    AlreadyImported,
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
    /// Not enough permissions given by permission contract.
    NotAllowed,
    /// Signature error
    InvalidSignature(String),
    InconsistentShardOutcomes,
    ParcelsTooBig,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        let msg: String = match self {
            Error::AlreadyImported => "Already imported".into(),
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
            Error::NotAllowed => "Sender does not have permissions to execute this type of transaction".into(),
            Error::InvalidSignature(err) => format!("Parcel has invalid signature: {}.", err),
            Error::InconsistentShardOutcomes => "Shard outcomes are inconsistent".to_string(),
            Error::ParcelsTooBig => "Parcel size exceeded the body size limit".to_string(),
        };

        f.write_fmt(format_args!("Parcel error ({})", msg))
    }
}

impl From<KeyError> for Error {
    fn from(err: KeyError) -> Self {
        Error::InvalidSignature(format!("{}", err))
    }
}
