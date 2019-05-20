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

use ctypes::BlockNumber;
use primitives::H256;

/// Uniquely identifies block.
#[derive(Debug, PartialEq, Copy, Clone, Hash, Eq)]
pub enum BlockId {
    /// Block's blake256.
    /// Querying by hash is always faster.
    Hash(H256),
    /// Block number within canon blockchain.
    Number(BlockNumber),
    /// Earliest block (genesis).
    Earliest,
    /// Latest mined block.
    Latest,
    /// Parent of latest mined block.
    ParentOfLatest,
}

impl From<H256> for BlockId {
    fn from(hash: H256) -> Self {
        BlockId::Hash(hash)
    }
}

impl From<BlockNumber> for BlockId {
    fn from(number: BlockNumber) -> Self {
        BlockId::Number(number)
    }
}
/// Uniquely identifies transaction.
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum TransactionId {
    /// Transaction's blake256.
    Hash(H256),
    /// Block id and transaction index within this block.
    /// Querying by block position is always faster.
    Location(BlockId, usize),
}

impl From<H256> for TransactionId {
    fn from(hash: H256) -> Self {
        TransactionId::Hash(hash)
    }
}
