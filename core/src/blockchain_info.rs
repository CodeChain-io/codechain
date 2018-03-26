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

use ctypes::{H256, U256};

use super::types::BlockNumber;

/// Information about the blockchain gathered together.
#[derive(Clone, Debug)]
pub struct BlockChainInfo {
    /// Blockchain score.
    pub total_score: U256,
    /// Block queue score.
    pub pending_total_score: U256,
    /// Genesis block hash.
    pub genesis_hash: H256,
    /// Best blockchain block hash.
    pub best_block_hash: H256,
    /// Best blockchain block number.
    pub best_block_number: BlockNumber,
    /// Best blockchain block timestamp.
    pub best_block_timestamp: u64,
}
