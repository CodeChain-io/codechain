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

use std::fmt::{Display, Formatter, Result as FormatResult};
use std::io::{Error as FileError, ErrorKind};

use kvdb::Error as DBError;
use primitives::H256;

#[derive(Debug)]
pub enum Error {
    NodeNotFound(H256),
    DBError(DBError),
    FileError(ErrorKind),
}

impl From<DBError> for Error {
    fn from(error: DBError) -> Self {
        Error::DBError(error)
    }
}

impl From<FileError> for Error {
    fn from(error: FileError) -> Self {
        Error::FileError(error.kind())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            Error::NodeNotFound(key) => write!(f, "State node not found: {:x}", key),
            Error::DBError(error) => write!(f, "DB Error: {:?}", error),
            Error::FileError(kind) => write!(f, "File system error: {:?}", kind),
        }
    }
}
