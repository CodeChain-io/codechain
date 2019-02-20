// Copyright 2019 Kodebox, Inc.
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

mod established;
mod incoming;
mod message;
mod outgoing;

use std::fmt;
use std::io;
use std::result;

use ccrypto::aes::SymmetricCipherError;
use rlp::DecoderError;

pub use self::established::EstablishedConnection;
pub use self::incoming::IncomingConnection;
pub use self::message::{IncomingMessage, OutgoingMessage};
pub use self::outgoing::OutgoingConnection;

use super::super::stream::Error as StreamError;
use super::stream::Error as P2pStreamError;


#[derive(Debug)]
pub enum Error {
    SymmetricCipher(SymmetricCipherError),
    IoError(io::Error),
    Decoder(DecoderError),
    InvalidSign,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::SymmetricCipher(err) => write!(f, "{:?}", err),
            Error::Decoder(err) => err.fmt(f),
            Error::IoError(err) => err.fmt(f),
            Error::InvalidSign => write!(f, "Invalid signature"),
        }
    }
}

impl From<DecoderError> for Error {
    fn from(err: DecoderError) -> Self {
        Error::Decoder(err)
    }
}

impl From<StreamError> for Error {
    fn from(err: StreamError) -> Self {
        match err {
            StreamError::IoError(err) => Error::IoError(err),
            StreamError::DecoderError(err) => Error::Decoder(err),
        }
    }
}

impl From<P2pStreamError> for Error {
    fn from(err: P2pStreamError) -> Self {
        match err {
            P2pStreamError::IoError(err) => Error::IoError(err),
            P2pStreamError::DecoderError(err) => Error::Decoder(err),
            P2pStreamError::InvalidSign => Error::InvalidSign,
        }
    }
}

impl From<SymmetricCipherError> for Error {
    fn from(err: SymmetricCipherError) -> Self {
        Error::SymmetricCipher(err)
    }
}

pub type Result<T> = result::Result<T, Error>;
