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

use bech32::Error as Bech32Error;
use secp256k1::Error as SecpError;

#[derive(Debug, PartialEq)]
pub enum Error {
    InvalidPublic,
    InvalidSecret,
    InvalidMessage,
    InvalidSignature,
    InvalidNetwork,
    InvalidChecksum,
    InvalidPrivate,
    InvalidAddress,
    FailedKeyGeneration,
    Bech32MissingSeparator,
    Bech32InvalidChecksum,
    Bech32InvalidLength,
    Bech32InvalidChar(u8),
    Bech32InvalidData(u8),
    Bech32MixedCase,
    Bech32UnknownHRP,
    Custom(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match *self {
            Error::InvalidPublic => "Invalid Public".into(),
            Error::InvalidSecret => "Invalid Secret".into(),
            Error::InvalidMessage => "Invalid Message".into(),
            Error::InvalidSignature => "Invalid Signature".into(),
            Error::InvalidNetwork => "Invalid Network".into(),
            Error::InvalidChecksum => "Invalid Checksum".into(),
            Error::InvalidPrivate => "Invalid Private".into(),
            Error::InvalidAddress => "Invalid Address".into(),
            Error::FailedKeyGeneration => "Key generation failed".into(),
            Error::Bech32MissingSeparator => "Missing human-readable separator".into(),
            Error::Bech32InvalidChecksum => "Invalid checksum".into(),
            Error::Bech32InvalidLength => "Invalid Length".into(),
            Error::Bech32InvalidChar(_) => "Invalid character".into(),
            Error::Bech32InvalidData(_) => "Invalid data point".into(),
            Error::Bech32MixedCase => "Mixed-case strings not allowed".into(),
            Error::Bech32UnknownHRP => "Unknown human-readable part".into(),
            Error::Custom(ref s) => s.clone(),
        };

        msg.fmt(f)
    }
}

impl Into<String> for Error {
    fn into(self) -> String {
        format!("{}", self)
    }
}

impl From<SecpError> for Error {
    fn from(e: SecpError) -> Self {
        match e {
            SecpError::InvalidPublicKey => Error::InvalidPublic,
            SecpError::InvalidSecretKey => Error::InvalidSecret,
            SecpError::InvalidMessage => Error::InvalidMessage,
            _ => Error::InvalidSignature,
        }
    }
}

impl From<Bech32Error> for Error {
    fn from(e: Bech32Error) -> Self {
        match e {
            Bech32Error::MissingSeparator => Error::Bech32MissingSeparator,
            Bech32Error::InvalidChecksum => Error::Bech32InvalidChecksum,
            Bech32Error::InvalidLength => Error::Bech32InvalidLength,
            Bech32Error::InvalidChar(ch) => Error::Bech32InvalidChar(ch),
            Bech32Error::InvalidData(data) => Error::Bech32InvalidData(data),
            Bech32Error::MixedCase => Error::Bech32MixedCase,
        }
    }
}
