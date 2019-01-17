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

use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::TaggedRlp;
use crate::transaction::Timelock;
use crate::util::unexpected::Mismatch;

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum Error {
    /// Returned when transaction seq does not match state seq
    InvalidSeq(Mismatch<u64>),
    /// Transaction was not imported to the queue because limit has been reached.
    LimitReached,
    /// Transaction is not valid anymore (state already has higher seq)
    Old,
    OrderExpired {
        expiration: u64,
        timestamp: u64,
    },
    Timelocked {
        timelock: Timelock,
        remaining_time: u64,
    },
    /// Transction has too low fee
    /// (there is already a transaction with the same sender-seq but higher gas price)
    TooCheapToReplace,
    /// Transaction is already imported to the queue
    TransactionAlreadyImported,
}

const ERROR_ID_INVALID_SEQ: u8 = 10u8;
const ERROR_ID_LIMIT_REACHED: u8 = 7u8;
const ERROR_ID_OLD: u8 = 3u8;
const ERROR_ID_ORDER_EXPIRED: u8 = 35u8;
const ERROR_ID_TIMELOCKED: u8 = 26u8;
const ERROR_ID_TOO_CHEAP_TO_REPLACE: u8 = 4u8;
const ERROR_ID_TX_ALREADY_IMPORTED: u8 = 1u8;

struct RlpHelper;
impl TaggedRlp for RlpHelper {
    type Tag = u8;

    fn length_of(tag: u8) -> Result<usize, DecoderError> {
        Ok(match tag {
            ERROR_ID_INVALID_SEQ => 2,
            ERROR_ID_LIMIT_REACHED => 1,
            ERROR_ID_OLD => 1,
            ERROR_ID_ORDER_EXPIRED => 3,
            ERROR_ID_TIMELOCKED => 3,
            ERROR_ID_TOO_CHEAP_TO_REPLACE => 1,
            ERROR_ID_TX_ALREADY_IMPORTED => 1,
            _ => return Err(DecoderError::Custom("Invalid HistoryError")),
        })
    }
}

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::InvalidSeq(mismatch) => RlpHelper::new_tagged_list(s, ERROR_ID_INVALID_SEQ).append(mismatch),
            Error::LimitReached => RlpHelper::new_tagged_list(s, ERROR_ID_LIMIT_REACHED),
            Error::Old => RlpHelper::new_tagged_list(s, ERROR_ID_OLD),
            Error::OrderExpired {
                expiration,
                timestamp,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_ORDER_EXPIRED).append(expiration).append(timestamp),
            Error::Timelocked {
                timelock,
                remaining_time,
            } => RlpHelper::new_tagged_list(s, ERROR_ID_TIMELOCKED).append(timelock).append(remaining_time),
            Error::TooCheapToReplace => RlpHelper::new_tagged_list(s, ERROR_ID_TOO_CHEAP_TO_REPLACE),
            Error::TransactionAlreadyImported => RlpHelper::new_tagged_list(s, ERROR_ID_TX_ALREADY_IMPORTED),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        let error = match tag {
            ERROR_ID_INVALID_SEQ => Error::InvalidSeq(rlp.val_at(1)?),
            ERROR_ID_LIMIT_REACHED => Error::LimitReached,
            ERROR_ID_OLD => Error::Old,
            ERROR_ID_ORDER_EXPIRED => Error::OrderExpired {
                expiration: rlp.val_at(1)?,
                timestamp: rlp.val_at(2)?,
            },
            ERROR_ID_TIMELOCKED => Error::Timelocked {
                timelock: rlp.val_at(1)?,
                remaining_time: rlp.val_at(2)?,
            },
            ERROR_ID_TOO_CHEAP_TO_REPLACE => Error::TooCheapToReplace,
            ERROR_ID_TX_ALREADY_IMPORTED => Error::TransactionAlreadyImported,
            _ => return Err(DecoderError::Custom("Invalid HistoryError")),
        };
        RlpHelper::check_size(rlp, tag)?;
        Ok(error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            Error::InvalidSeq(mismatch) => write!(f, "Invalid transaction seq {}", mismatch),
            Error::LimitReached => write!(f, "Transaction limit reached"),
            Error::Old => write!(f, "No longer valid"),
            Error::OrderExpired {
                expiration,
                timestamp,
            } => write!(f, "The order is expired. Expiration: {}, Block timestamp: {}", expiration, timestamp),
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
        }
    }
}
