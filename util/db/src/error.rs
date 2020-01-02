// Copyright 2019 Kodebox, Inc.
// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of CodeChain.
//
// This is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this.  If not, see <http://www.gnu.org/licenses/>.

use primitives::H256;
use std::{fmt, io};

#[derive(Debug)]
/// Error in database subsystem.
pub enum DatabaseError {
    Io(io::Error),
    /// An entry was removed more times than inserted.
    NegativelyReferencedHash(H256),
    /// A committed value was inserted more than once.
    AlreadyExists(H256),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DatabaseError::NegativelyReferencedHash(hash) => {
                write!(f, "Entry {} removed from database more times than it was added.", hash)
            }
            DatabaseError::AlreadyExists(hash) => write!(f, "Committed key already exists in database: {}", hash),
            DatabaseError::Io(ref err) => err.fmt(f),
        }
    }
}

impl From<io::Error> for DatabaseError {
    fn from(err: io::Error) -> Self {
        DatabaseError::Io(err)
    }
}
