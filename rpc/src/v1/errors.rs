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

use ccore::Error as CoreError;
use rlp::DecoderError;

use jsonrpc_core::{Error, ErrorCode, Value};

mod codes {
    // NOTE [ToDr] Codes from [-32099, -32000]
    pub const UNSUPPORTED_REQUEST: i64 = -32000;
    pub const NO_WORK: i64 = -32001;
    pub const NO_AUTHOR: i64 = -32002;
    pub const NO_NEW_WORK: i64 = -32003;
    pub const NO_WORK_REQUIRED: i64 = -32004;
    pub const UNKNOWN_ERROR: i64 = -32009;
    pub const TRANSACTION_ERROR: i64 = -32010;
    pub const EXECUTION_ERROR: i64 = -32015;
    pub const EXCEPTION_ERROR: i64 = -32016;
    pub const DATABASE_ERROR: i64 = -32017;
    pub const ACCOUNT_LOCKED: i64 = -32020;
    pub const PASSWORD_INVALID: i64 = -32021;
    pub const ACCOUNT_ERROR: i64 = -32023;
    pub const REQUEST_REJECTED: i64 = -32040;
    pub const REQUEST_REJECTED_LIMIT: i64 = -32041;
    pub const REQUEST_NOT_FOUND: i64 = -32042;
    pub const ENCRYPTION_ERROR: i64 = -32055;
    pub const ENCODING_ERROR: i64 = -32058;
    pub const FETCH_ERROR: i64 = -32060;
    pub const NO_LIGHT_PEERS: i64 = -32065;
    pub const DEPRECATED: i64 = -32070;
}

pub fn transaction<T: Into<CoreError>>(error: T) -> Error {
    let error = error.into();
    if let CoreError::Transaction(e) = error {
        Error {
            code: ErrorCode::ServerError(codes::TRANSACTION_ERROR),
            message: ::ccore::transaction_error_message(&e),
            data: None,
        }
    } else {
        Error {
            code: ErrorCode::ServerError(codes::UNKNOWN_ERROR),
            message: "Unknown error when sending transaction.".into(),
            data: Some(Value::String(format!("{:?}", error))),
        }
    }
}

pub fn rlp(error: DecoderError) -> Error {
    Error {
        code: ErrorCode::InvalidParams,
        message: "Invalid RLP.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}
