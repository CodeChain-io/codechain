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

use std::fmt;

use ccore::AccountProviderError;
use ccore::Error as CoreError;
use cnetwork::control::Error as NetworkControlError;
use kvdb::Error as KVDBError;
use rlp::DecoderError;

use jsonrpc_core::{Error, ErrorCode, Value};

mod codes {
    pub const NO_AUTHOR: i64 = -32002;
    pub const NO_WORK_REQUIRED: i64 = -32004;
    pub const UNKNOWN_ERROR: i64 = -32009;
    pub const PARCEL_ERROR: i64 = -32010;
    pub const KVDB_ERROR: i64 = -32011;
    pub const NETWORK_DISABLED: i64 = -32014;
    pub const NETWORK_CANNOT_DISCONNECT_NOT_CONNECTED_ERROR: i64 = -32015;
    pub const ACCOUNT_PROVIDER_ERROR: i64 = -32016;
}

pub fn parcel<T: Into<CoreError>>(error: T) -> Error {
    let error = error.into();
    if let CoreError::Parcel(e) = error {
        Error {
            code: ErrorCode::ServerError(codes::PARCEL_ERROR),
            message: format!("{}", e),
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

pub fn account_provider(error: AccountProviderError) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::ACCOUNT_PROVIDER_ERROR),
        message: "AccountProvider error".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn no_author() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::NO_AUTHOR),
        message: "Author not configured. Run Parity with --author to configure.".into(),
        data: None,
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

pub fn network_control(error: NetworkControlError) -> Error {
    match error {
        NetworkControlError::NotConnected => Error {
            code: ErrorCode::ServerError(codes::NETWORK_CANNOT_DISCONNECT_NOT_CONNECTED_ERROR),
            message: "Cannot disconnect not connected node".into(),
            data: None,
        },
    }
}

/// Internal error signifying a logic error in code.
/// Should not be used when function can just fail
/// because of invalid parameters or incomplete node state.
pub fn internal<T: fmt::Debug>(error: &str, data: T) -> Error {
    Error {
        code: ErrorCode::InternalError,
        message: format!("Internal error occurred: {}", error),
        data: Some(Value::String(format!("{:?}", data))),
    }
}
