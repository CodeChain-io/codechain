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
use kvdb::Error as KVDBError;
use rlp::DecoderError;

use jsonrpc_core::{Error, ErrorCode, Value};

mod codes {
    pub const UNKNOWN_ERROR: i64 = -32009;
    pub const TRANSACTION_ERROR: i64 = -32010;
    pub const KVDB_ERROR: i64 = -32011;
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

pub fn kvdb(error: KVDBError) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::KVDB_ERROR),
        message: "KVDB error.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn rlp(error: DecoderError) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::UNKNOWN_ERROR),
        message: "Invalid RLP.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}
