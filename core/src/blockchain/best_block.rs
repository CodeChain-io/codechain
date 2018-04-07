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

use cbytes::Bytes;
use ctypes::{H256, U256};

use super::super::types::BlockNumber;

/// Contains information on a best block that is specific to the consensus engine.
///
/// Sometimes referred as 'latest block'.
#[derive(Default)]
pub struct BestBlock {
    /// Best block hash.
    pub hash: H256,
    /// Best block number.
    pub number: BlockNumber,
    /// Best block timestamp.
    pub timestamp: u64,
    /// Best block total score.
    pub total_score: U256,
    /// Best block uncompressed bytes
    pub block: Bytes,
}
