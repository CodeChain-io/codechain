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

use std::fmt::{Display, Error as FmtError, Formatter};

use kvdb;
use util_error::UtilError;

/// Client configuration errors.
#[derive(Debug)]
pub enum Error {
    /// Database error
    Database(kvdb::Error),
    /// Util error
    Util(UtilError),
}

impl From<UtilError> for Error {
    fn from(err: UtilError) -> Self {
        Error::Util(err)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match self {
            Error::Util(err) => write!(f, "{}", err),
            Error::Database(s) => write!(f, "Database error: {}", s),
        }
    }
}
