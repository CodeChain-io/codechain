// Copyright 2018-2019 Kodebox, Inc.
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

use ckey::Error as KeyError;
use cmerkle::TrieError;
use ctypes::errors::{HistoryError, RuntimeError, SyntaxError};

#[derive(Debug, PartialEq)]
pub enum Error {
    Key(KeyError),
    Trie(TrieError),
    History(HistoryError),
    Runtime(RuntimeError),
    Syntax(SyntaxError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Key(err) => err.fmt(f),
            Error::Trie(err) => err.fmt(f),
            Error::History(err) => err.fmt(f),
            Error::Runtime(err) => err.fmt(f),
            Error::Syntax(err) => err.fmt(f),
        }
    }
}

impl From<KeyError> for Error {
    fn from(err: KeyError) -> Self {
        Error::Key(err)
    }
}

impl From<TrieError> for Error {
    fn from(err: TrieError) -> Self {
        Error::Trie(err)
    }
}

impl From<HistoryError> for Error {
    fn from(err: HistoryError) -> Self {
        Error::History(err)
    }
}

impl From<RuntimeError> for Error {
    fn from(err: RuntimeError) -> Self {
        Error::Runtime(err)
    }
}

impl From<SyntaxError> for Error {
    fn from(err: SyntaxError) -> Self {
        Error::Syntax(err)
    }
}
