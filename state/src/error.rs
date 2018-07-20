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

use ctypes::parcel::Error as ParcelError;
use ctypes::transaction::Error as TransactionError;
use trie::TrieError;

#[derive(Debug)]
pub enum Error {
    Trie(TrieError),
    Parcel(ParcelError),
    Transaction(TransactionError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Parcel(err) => err.fmt(f),
            Error::Trie(err) => err.fmt(f),
            Error::Transaction(err) => err.fmt(f),
        }
    }
}

impl From<TrieError> for Error {
    fn from(err: TrieError) -> Self {
        Error::Trie(err)
    }
}

impl From<ParcelError> for Error {
    fn from(err: ParcelError) -> Error {
        Error::Parcel(err)
    }
}

impl From<TransactionError> for Error {
    fn from(err: TransactionError) -> Self {
        Error::Transaction(err)
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
