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

use cjson;
use primitives::U256;

pub struct CuckooParams {
    pub block_reward: U256,
    pub min_score: U256,
    pub max_vertex: usize,
    pub max_edge: usize,
    pub cycle_length: usize,
}

impl From<cjson::spec::CuckooParams> for CuckooParams {
    fn from(p: cjson::spec::CuckooParams) -> Self {
        CuckooParams {
            block_reward: p.block_reward.map_or(U256::from(0), Into::into),
            min_score: p.min_score.map_or(U256::from(0x020000), Into::into),
            max_vertex: p.max_vertex.map_or(1 << 30, Into::into),
            max_edge: p.max_edge.map_or(1 << 29, Into::into),
            cycle_length: p.cycle_length.map_or(42, Into::into),
        }
    }
}
