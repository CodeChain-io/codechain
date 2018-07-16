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
use ckey::{Address, Error as KeyError};
use ctypes::parcel::Error as ParcelError;
use ctypes::transaction::Error as TransactionError;
use primitives::{H256, U256};
use trie::TrieError;
use unexpected::{Mismatch, OutOfBounds};
use util_error::UtilError;

use super::account_provider::SignError as AccountsError;
use super::client::Error as ClientError;
use super::consensus::EngineError;
use super::types::BlockNumber;

#[derive(Debug, Clone, Copy, PartialEq)]
/// Import to the block queue result
pub enum ImportError {
    /// Already in the block chain.
    AlreadyInChain,
    /// Already in the block queue.
    AlreadyQueued,
    /// Already marked as bad from a previous import (could mean parent is bad).
    KnownBad,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            ImportError::AlreadyInChain => "block already in chain",
            ImportError::AlreadyQueued => "block already in the block queue",
            ImportError::KnownBad => "block known to be bad",
        };

        f.write_fmt(format_args!("Block import error ({})", msg))
    }
}

/// Error dedicated to import block function
#[derive(Debug)]
pub enum BlockImportError {
    /// Import error
    Import(ImportError),
    /// Block error
    Block(BlockError),
    /// Other error
    Other(String),
}

impl From<Error> for BlockImportError {
    fn from(e: Error) -> Self {
        match e {
            Error::Block(block_error) => BlockImportError::Block(block_error),
            Error::Import(import_error) => BlockImportError::Import(import_error),
            _ => BlockImportError::Other(format!("other block import error: {:?}", e)),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
/// Errors concerning block processing.
pub enum BlockError {
    /// Extra data is of an invalid length.
    ExtraDataOutOfBounds(OutOfBounds<usize>),
    /// Seal is incorrect format.
    InvalidSealArity(Mismatch<usize>),
    /// State root header field is invalid.
    InvalidStateRoot(Mismatch<H256>),
    /// Parcels root header field is invalid.
    InvalidParcelsRoot(Mismatch<H256>),
    /// Score is out of range; this can be used as an looser error prior to getting a definitive
    /// value for score. This error needs only provide bounds of which it is out.
    ScoreOutOfBounds(OutOfBounds<U256>),
    /// Score header field is invalid; this is a strong error used after getting a definitive
    /// value for difficulty (which is provided).
    InvalidScore(Mismatch<U256>),
    /// Proof-of-work aspect of seal is invalid.
    InvalidProofOfWork,
    /// Score of proof-of-work is out of bound.
    PowOutOfBounds(OutOfBounds<U256>),
    /// Some low-level aspect of the seal is incorrect.
    InvalidSeal,
    /// Invoices trie root header field is invalid.
    InvalidInvoicesRoot(Mismatch<H256>),
    /// Timestamp header field is invalid.
    InvalidTimestamp(OutOfBounds<u64>),
    /// Timestamp header field is too far in future.
    TemporarilyInvalid(OutOfBounds<u64>),
    /// Parent hash field of header is invalid; this is an invalid error indicating a logic flaw in the codebase.
    /// TODO: remove and favour an assert!/panic!.
    InvalidParentHash(Mismatch<H256>),
    /// Number field of header is invalid.
    InvalidNumber(Mismatch<BlockNumber>),
    /// Block number isn't sensible.
    RidiculousNumber(OutOfBounds<BlockNumber>),
    /// Too many parcels from a particular address.
    TooManyParcels(Address),
    /// Parent given is unknown.
    UnknownParent(H256),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpecError {
    InvalidCommonParams,
    InvalidState,
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::SpecError::*;
        let msg: String = match self {
            InvalidCommonParams => "Common params are not matched with gensis block".into(),
            InvalidState => "Genesis state is not same with spec".into(),
        };
        f.write_fmt(format_args!("Spec file error ({})", msg))
    }
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::BlockError::*;

        let msg: String = match self {
            ExtraDataOutOfBounds(oob) => format!("Extra block data too long. {}", oob),
            InvalidSealArity(mis) => format!("Block seal in incorrect format: {}", mis),
            InvalidStateRoot(mis) => format!("Invalid state root in header: {}", mis),
            InvalidParcelsRoot(mis) => format!("Invalid parcels root in header: {}", mis),
            ScoreOutOfBounds(oob) => format!("Invalid block score: {}", oob),
            InvalidScore(oob) => format!("Invalid block score: {}", oob),
            InvalidProofOfWork => "Invalid proof of work.".into(),
            PowOutOfBounds(oob) => format!("Invalid proof of work: {}", oob),
            InvalidSeal => "Block has invalid seal.".into(),
            InvalidInvoicesRoot(mis) => format!("Invalid invoices trie root in header: {}", mis),
            InvalidTimestamp(oob) => format!("Invalid timestamp in header: {}", oob),
            TemporarilyInvalid(oob) => format!("Future timestamp in header: {}", oob),
            InvalidParentHash(mis) => format!("Invalid parent hash: {}", mis),
            InvalidNumber(mis) => format!("Invalid number in header: {}", mis),
            RidiculousNumber(oob) => format!("Implausible block number. {}", oob),
            UnknownParent(hash) => format!("Unknown parent: {}", hash),
            TooManyParcels(address) => format!("Too many parcels from: {}", address),
        };

        f.write_fmt(format_args!("Block error ({})", msg))
    }
}

#[derive(Debug)]
/// General error type which should be capable of representing all errors in codechain
pub enum Error {
    /// Client configuration error.
    Client(ClientError),
    /// Error concerning a utility.
    Util(UtilError),
    /// Error concerning block processing.
    Block(BlockError),
    /// Error concerning parcel processing.
    Parcel(ParcelError),
    /// Error concerning block import.
    Import(ImportError),
    /// Io crate error.
    Io(IoError),
    /// Consensus vote error.
    Engine(EngineError),
    /// Key error.
    Key(KeyError),
    /// TrieDB-related error.
    Trie(TrieError),
    /// PoW hash is invalid or out of date.
    PowHashInvalid,
    /// The value of the nonce or mishash is invalid.
    PowInvalid,
    Spec(SpecError),
    /// Account Provider error.
    AccountProvider(AccountsError),
    Transaction(TransactionError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Client(err) => err.fmt(f),
            Error::Util(err) => err.fmt(f),
            Error::Io(err) => err.fmt(f),
            Error::Block(err) => err.fmt(f),
            Error::Parcel(err) => err.fmt(f),
            Error::Import(err) => err.fmt(f),
            Error::Engine(err) => err.fmt(f),
            Error::Key(err) => err.fmt(f),
            Error::Trie(err) => err.fmt(f),
            Error::PowHashInvalid => f.write_str("Invalid or out of date PoW hash."),
            Error::PowInvalid => f.write_str("Invalid nonce or mishash"),
            Error::Spec(err) => err.fmt(f),
            Error::AccountProvider(err) => err.fmt(f),
            Error::Transaction(err) => err.fmt(f),
        }
    }
}

impl From<ClientError> for Error {
    fn from(err: ClientError) -> Error {
        Error::Client(err)
    }
}

impl From<ParcelError> for Error {
    fn from(err: ParcelError) -> Error {
        Error::Parcel(err)
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

impl From<SpecError> for Error {
    fn from(err: SpecError) -> Error {
        Error::Spec(err)
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

impl From<ImportError> for Error {
    fn from(err: ImportError) -> Error {
        Error::Import(err)
    }
}

impl From<BlockImportError> for Error {
    fn from(err: BlockImportError) -> Error {
        match err {
            BlockImportError::Block(e) => Error::Block(e),
            BlockImportError::Import(e) => Error::Import(e),
            BlockImportError::Other(s) => Error::Util(UtilError::from(s)),
        }
    }
}

impl From<TrieError> for Error {
    fn from(err: TrieError) -> Self {
        Error::Trie(err)
    }
}

impl<E> From<Box<E>> for Error
where
    Error: From<E>,
{
    fn from(err: Box<E>) -> Self {
        Error::from(*err)
    }
}

impl From<AccountsError> for Error {
    fn from(err: AccountsError) -> Error {
        Error::AccountProvider(err)
    }
}

impl From<TransactionError> for Error {
    fn from(err: TransactionError) -> Self {
        Error::Transaction(err)
    }
}
