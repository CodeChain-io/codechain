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

use cio::IoError;
use ckeys::{Error as KeyError};
use util_error::UtilError;
use unexpected::Mismatch;

use super::consensus::EngineError;
use super::transaction::TransactionError;

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
/// Errors concerning block processing.
pub enum BlockError {
    /// Seal is incorrect format.
    InvalidSealArity(Mismatch<usize>),
    /// Some low-level aspect of the seal is incorrect.
    InvalidSeal,
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::BlockError::*;

        let msg: String = match *self {
            InvalidSealArity(ref mis) => format!("Block seal in incorrect format: {}", mis),
            InvalidSeal => "Block has invalid seal.".into(),
        };

        f.write_fmt(format_args!("Block error ({})", msg))
    }
}


#[derive(Debug)]
/// General error type which should be capable of representing all errors in codechain
pub enum Error {
    /// Error concerning a utility.
    Util(UtilError),
    /// Error concerning block processing.
    Block(BlockError),
    /// Error concerning transaction processing.
    Transaction(TransactionError),
    /// Io crate error.
    Io(IoError),
    /// Consensus vote error.
    Engine(EngineError),
    /// Key error.
    Key(KeyError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Util(ref err) => err.fmt(f),
            Error::Io(ref err) => err.fmt(f),
            Error::Block(ref err) => err.fmt(f),
            Error::Transaction(ref err) => err.fmt(f),
            Error::Engine(ref err) => err.fmt(f),
            Error::Key(ref err) => err.fmt(f),
        }
    }
}

impl From<TransactionError> for Error {
    fn from(err: TransactionError) -> Error {
        Error::Transaction(err)
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Error {
        Error::Io(err)
    }
}

impl From<BlockError> for Error {
    fn from(err: BlockError) -> Error {
        Error::Block(err)
    }
}

impl From<EngineError> for Error {
    fn from(err: EngineError) -> Error {
        Error::Engine(err)
    }
}

impl From<KeyError> for Error {
    fn from(err: KeyError) -> Error {
        Error::Key(err)
    }
}

impl From<::rlp::DecoderError> for Error {
    fn from(err: ::rlp::DecoderError) -> Error {
        Error::Util(UtilError::from(err))
    }
}

impl From<UtilError> for Error {
    fn from(err: UtilError) -> Error {
        Error::Util(err)
    }
}
