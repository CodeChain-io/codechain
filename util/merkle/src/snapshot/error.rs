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

use std::io::Error as IoError;

use primitives::H256;
use rlp::DecoderError as RlpDecoderError;

use crate::TrieError;

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
    RlpDecoderError(RlpDecoderError),
    TrieError(TrieError),
    ChunkError(ChunkError),
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::IoError(err)
    }
}

impl From<RlpDecoderError> for Error {
    fn from(err: RlpDecoderError) -> Self {
        Error::RlpDecoderError(err)
    }
}

impl From<TrieError> for Error {
    fn from(err: TrieError) -> Self {
        Error::TrieError(err)
    }
}

impl From<ChunkError> for Error {
    fn from(err: ChunkError) -> Self {
        Error::ChunkError(err)
    }
}

#[derive(Debug)]
pub enum ChunkError {
    TooBig,
    InvalidHeight,
    ChunkRootMismatch {
        expected: H256,
        actual: H256,
    },
    InvalidContent,
}
