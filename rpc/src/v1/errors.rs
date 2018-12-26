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
use ckey::Error as KeyError;
use ckeystore::Error as KeystoreError;
use cnetwork::control::Error as NetworkControlError;
use cstate::StateError;
use ctypes::parcel::Error as ParcelError;
use kvdb::Error as KVDBError;
use rlp::DecoderError;

use jsonrpc_core::{Error, ErrorCode, Value};

mod codes {
    pub const NO_AUTHOR: i64 = -32002;
    pub const NO_WORK_REQUIRED: i64 = -32004;
    pub const RLP_ERROR: i64 = -32009;
    pub const CORE_ERROR: i64 = -32010;
    pub const KVDB_ERROR: i64 = -32011;
    pub const PARCEL_ERROR: i64 = -32012;
    pub const NETWORK_DISABLED: i64 = -32014;
    pub const NETWORK_CANNOT_DISCONNECT_NOT_CONNECTED_ERROR: i64 = -32015;
    pub const ACCOUNT_PROVIDER_ERROR: i64 = -32016;
    pub const VERIFICATION_FAILED: i64 = -32030;
    pub const ALREADY_IMPORTED: i64 = -32031;
    pub const NOT_ENOUGH_BALANCE: i64 = -32032;
    pub const TOO_LOW_FEE: i64 = -32033;
    pub const TOO_CHEAP_TO_REPLACE: i64 = -32034;
    pub const INVALID_SEQ: i64 = -32035;
    pub const INVALID_NETWORK_ID: i64 = -32036;
    pub const KEYSTORE_ERROR: i64 = -32040;
    pub const KEY_ERROR: i64 = -32041;
    pub const ALREADY_EXISTS: i64 = -32042;
    pub const WRONG_PASSWORD: i64 = -32043;
    pub const NO_SUCH_ACCOUNT: i64 = -32044;
    pub const NOT_UNLOCKED: i64 = -32045;
    pub const TRANSFER_ONLY_IN_EXECUTE_VM: i64 = -32046;
    pub const ASSET_TRANSACTION_ONLY_IN_EXECUTE_TRANSACITON: i64 = -32047;
    pub const UNKNOWN_ERROR: i64 = -32099;
}

pub fn core<T: Into<CoreError>>(error: T) -> Error {
    let error = error.into();
    Error {
        code: ErrorCode::ServerError(codes::CORE_ERROR),
        message: format!("{}", error),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn parcel_state<T: Into<StateError>>(error: T) -> Error {
    let error = error.into();
    if let StateError::Parcel(e) = error {
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

pub fn parcel_core<T: Into<CoreError>>(error: T) -> Error {
    let error = error.into();
    let unknown_error = Error {
        code: ErrorCode::ServerError(codes::UNKNOWN_ERROR),
        message: "Unknown error when sending parcel.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    };
    match error {
        CoreError::Key(error) => match error {
            KeyError::InvalidSignature => Error {
                code: ErrorCode::ServerError(codes::VERIFICATION_FAILED),
                message: "Verification Failed".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            KeyError::InvalidNetworkId(_) => Error {
                code: ErrorCode::ServerError(codes::INVALID_NETWORK_ID),
                message: "Invalid NetworkId".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            _ => unknown_error,
        },
        CoreError::State(StateError::Parcel(error)) => match error {
            ParcelError::InvalidSignature(_) => Error {
                code: ErrorCode::ServerError(codes::VERIFICATION_FAILED),
                message: "Verification Failed".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            ParcelError::InvalidNetworkId(_) => Error {
                code: ErrorCode::ServerError(codes::INVALID_NETWORK_ID),
                message: "Invalid NetworkId".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            ParcelError::ParcelAlreadyImported => Error {
                code: ErrorCode::ServerError(codes::ALREADY_IMPORTED),
                message: "Already Imported".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            ParcelError::InsufficientBalance {
                ..
            } => Error {
                code: ErrorCode::ServerError(codes::NOT_ENOUGH_BALANCE),
                message: "Not Enough Balance".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            ParcelError::InsufficientFee {
                ..
            } => Error {
                code: ErrorCode::ServerError(codes::TOO_LOW_FEE),
                message: "Too Low Fee".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            ParcelError::TooCheapToReplace => Error {
                code: ErrorCode::ServerError(codes::TOO_CHEAP_TO_REPLACE),
                message: "Too Cheap to Replace".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            ParcelError::Old {
                ..
            } => Error {
                code: ErrorCode::ServerError(codes::INVALID_SEQ),
                message: "Invalid Seq".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            _ => unknown_error,
        },
        _ => unknown_error,
    }
}

pub fn kvdb(error: &KVDBError) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::KVDB_ERROR),
        message: "KVDB error.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn rlp(error: &DecoderError) -> Error {
    Error {
        code: ErrorCode::ServerError(codes::RLP_ERROR),
        message: "Invalid RLP.".into(),
        data: Some(Value::String(format!("{:?}", error))),
    }
}

pub fn account_provider(error: AccountProviderError) -> Error {
    match error {
        AccountProviderError::KeystoreError(error) => match error {
            KeystoreError::InvalidAccount => Error {
                code: ErrorCode::ServerError(codes::NO_SUCH_ACCOUNT),
                message: "No Such Account".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            KeystoreError::InvalidPassword => Error {
                code: ErrorCode::ServerError(codes::WRONG_PASSWORD),
                message: "Wrong Password".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            KeystoreError::AlreadyExists => Error {
                code: ErrorCode::ServerError(codes::ALREADY_EXISTS),
                message: "Already Exists".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
            _ => Error {
                code: ErrorCode::ServerError(codes::KEYSTORE_ERROR),
                message: "Keystore Error".into(),
                data: Some(Value::String(format!("{:?}", error))),
            },
        },
        AccountProviderError::KeyError(_) => Error {
            code: ErrorCode::ServerError(codes::KEY_ERROR),
            message: "Key Error".into(),
            data: Some(Value::String(format!("{:?}", error))),
        },
        AccountProviderError::NotUnlocked => Error {
            code: ErrorCode::ServerError(codes::NOT_UNLOCKED),
            message: "Not Unlocked".into(),
            data: Some(Value::String(format!("{:?}", error))),
        },
        _ => Error {
            code: ErrorCode::ServerError(codes::ACCOUNT_PROVIDER_ERROR),
            message: "AccountProvider Error".into(),
            data: Some(Value::String(format!("{:?}", error))),
        },
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

pub fn network_control(error: &NetworkControlError) -> Error {
    match error {
        NetworkControlError::NotConnected => Error {
            code: ErrorCode::ServerError(codes::NETWORK_CANNOT_DISCONNECT_NOT_CONNECTED_ERROR),
            message: "Cannot disconnect not connected node".into(),
            data: None,
        },
        NetworkControlError::Disabled => Error {
            code: ErrorCode::ServerError(codes::NETWORK_DISABLED),
            message: "Network is diabled.".into(),
            data: None,
        },
    }
}

pub fn transfer_only() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::TRANSFER_ONLY_IN_EXECUTE_VM),
        message: "chain_executeVM() only accepts AssetTransfer transactions.".into(),
        data: None,
    }
}

pub fn asset_transaction_only() -> Error {
    Error {
        code: ErrorCode::ServerError(codes::ASSET_TRANSACTION_ONLY_IN_EXECUTE_TRANSACITON),
        message: "chain_executeTransaction() only accepts asset transactions.".into(),
        data: None,
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
