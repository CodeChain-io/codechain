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
use crate::transaction::Timelock;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use std::fmt::{Display, Formatter, Result as FormatResult};

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum Error {
    /// Transaction was not imported to the queue because limit has been reached.
    LimitReached,
    /// Transaction is not valid anymore (state already has higher seq)
    Old,
    Timelocked {
        timelock: Timelock,
        remaining_time: u64,
    },
    /// Transction has too low fee
    /// (there is already a transaction with the same sender-seq but higher gas price)
    TooCheapToReplace,
    /// Transaction is already imported to the queue
    TransactionAlreadyImported,
    TransferExpired {
        expiration: u64,
        timestamp: u64,
    },
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum ErrorID {
    LimitReached = 2,
    Old = 3,
    Timelocked = 5,
    TooCheapToReplace = 6,
    TxAlreadyImported = 7,
    TransferExpired = 8,
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
            2u8 => Ok(ErrorID::LimitReached),
            3 => Ok(ErrorID::Old),
            5 => Ok(ErrorID::Timelocked),
            6 => Ok(ErrorID::TooCheapToReplace),
            7 => Ok(ErrorID::TxAlreadyImported),
            8 => Ok(ErrorID::TransferExpired),
            _ => Err(DecoderError::Custom("Unexpected ErrorID Value")),
        }
    }
}

struct RlpHelper;
impl TaggedRlp for RlpHelper {
    type Tag = ErrorID;

    fn length_of(tag: ErrorID) -> Result<usize, DecoderError> {
        Ok(match tag {
            ErrorID::LimitReached => 1,
            ErrorID::Old => 1,
            ErrorID::Timelocked => 3,
            ErrorID::TooCheapToReplace => 1,
            ErrorID::TxAlreadyImported => 1,
            ErrorID::TransferExpired => 3,
        })
    }
}

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::LimitReached => RlpHelper::new_tagged_list(s, ErrorID::LimitReached),
            Error::Old => RlpHelper::new_tagged_list(s, ErrorID::Old),
            Error::Timelocked {
                timelock,
                remaining_time,
            } => RlpHelper::new_tagged_list(s, ErrorID::Timelocked).append(timelock).append(remaining_time),
            Error::TooCheapToReplace => RlpHelper::new_tagged_list(s, ErrorID::TooCheapToReplace),
            Error::TransactionAlreadyImported => RlpHelper::new_tagged_list(s, ErrorID::TxAlreadyImported),
            Error::TransferExpired {
                expiration,
                timestamp,
            } => RlpHelper::new_tagged_list(s, ErrorID::TransferExpired).append(expiration).append(timestamp),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at(0)?;
        let error = match tag {
            ErrorID::LimitReached => Error::LimitReached,
            ErrorID::Old => Error::Old,
            ErrorID::Timelocked => Error::Timelocked {
                timelock: rlp.val_at(1)?,
                remaining_time: rlp.val_at(2)?,
            },
            ErrorID::TooCheapToReplace => Error::TooCheapToReplace,
            ErrorID::TxAlreadyImported => Error::TransactionAlreadyImported,
            ErrorID::TransferExpired => Error::TransferExpired {
                expiration: rlp.val_at(1)?,
                timestamp: rlp.val_at(2)?,
            },
        };
        RlpHelper::check_size(rlp, tag)?;
        Ok(error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            Error::LimitReached => write!(f, "Transaction limit reached"),
            Error::Old => write!(f, "No longer valid"),
            Error::Timelocked {
                timelock,
                remaining_time,
            } => write!(
                f,
                "The transaction cannot be executed because of the timelock({:?}). The remaining time is {}",
                timelock, remaining_time
            ),
            Error::TooCheapToReplace => write!(f, "Fee too low to replace"),
            Error::TransactionAlreadyImported => write!(f, "The transaction is already imported"),
            Error::TransferExpired {
                expiration,
                timestamp,
            } => write!(
                f,
                "The TransferAsset transaction is expired. Expiration: {}, Block timestamp: {}",
                expiration, timestamp
            ),
        }
    }
}
