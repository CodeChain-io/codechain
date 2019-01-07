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

use cjson;
use ckey::Public;

#[derive(Debug, PartialEq)]
pub struct SimplePoAParams {
    /// Valid signatories.
    pub validators: Vec<Public>,
    /// base reward for a block.
    pub block_reward: u64,
}

impl From<cjson::scheme::SimplePoAParams> for SimplePoAParams {
    fn from(p: cjson::scheme::SimplePoAParams) -> Self {
        SimplePoAParams {
            validators: p.validators,
            block_reward: p.block_reward.map_or_else(Default::default, Into::into),
        }
    }
}
