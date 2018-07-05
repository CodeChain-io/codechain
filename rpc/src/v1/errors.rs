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
    pub const NO_WORK_REQUIRED: i64 = -32004;
    pub const UNKNOWN_ERROR: i64 = -32009;
    pub const PARCEL_ERROR: i64 = -32010;
    pub const KVDB_ERROR: i64 = -32011;
    pub const NETWORK_DISABLED: i64 = -32014;
}

pub fn parcel<T: Into<CoreError>>(error: T) -> Error {
    let error = error.into();
    if let CoreError::Parcel(e) = error {
        Error {
            code: ErrorCode::ServerError(codes::PARCEL_ERROR),
            message: ::ccore::parcel_error_message(&e),
            data: None,
        }
    } else {
        Error {
            code: ErrorCode::ServerError(codes::UNKNOWN_ERROR),
            message: "Unknown error when sending parcel.".into(),
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

pub fn no_work_required() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::NO_WORK_REQUIRED),
        message: "External work is only required for Proof of Work engines.".into(),
        data: None,
    }
}

pub fn network_disabled() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::NETWORK_DISABLED),
        message: "Network is diabled.".into(),
        data: None,
    }
}
