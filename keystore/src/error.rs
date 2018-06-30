// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use ccrypto;
use ccrypto::Error as CCryptoError;
use ckey::Error as CKeysError;
use std::fmt;
use std::io::Error as IoError;

/// Account-related errors.
#[derive(Debug)]
pub enum Error {
    /// IO error
    Io(IoError),
    /// Invalid Password
    InvalidPassword,
    /// Account's secret is invalid.
    InvalidSecret,
    /// Invalid Account.
    InvalidAccount,
    /// Invalid Message.
    InvalidMessage,
    /// Invalid Key File
    InvalidKeyFile(String),
    /// Account creation failed.
    CreationFailed,
    /// `ckeys` error
    CKeys(CKeysError),
    /// `CCrypto` error
    CCrypto(CCryptoError),
    /// Custom error
    Custom(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let s = match *self {
            Error::Io(ref err) => err.to_string(),
            Error::InvalidPassword => "Invalid password".into(),
            Error::InvalidSecret => "Invalid secret".into(),
            Error::InvalidAccount => "Invalid account".into(),
            Error::InvalidMessage => "Invalid message".into(),
            Error::InvalidKeyFile(ref reason) => format!("Invalid key file: {}", reason),
            Error::CreationFailed => "Account creation failed".into(),
            Error::CKeys(ref err) => err.to_string(),
            Error::CCrypto(ref err) => err.to_string(),
            Error::Custom(ref s) => s.clone(),
        };

        write!(f, "{}", s)
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::Io(err)
    }
}

impl From<CKeysError> for Error {
    fn from(err: CKeysError) -> Self {
        Error::CKeys(err)
    }
}

impl From<CCryptoError> for Error {
    fn from(err: CCryptoError) -> Self {
        Error::CCrypto(err)
    }
}

impl From<ccrypto::error::ScryptError> for Error {
    fn from(err: ccrypto::error::ScryptError) -> Self {
        Error::CCrypto(err.into())
    }
}

impl From<ccrypto::error::SymmError> for Error {
    fn from(err: ccrypto::error::SymmError) -> Self {
        Error::CCrypto(err.into())
    }
}
