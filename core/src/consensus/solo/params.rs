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

/// Params for a null engine.
#[derive(Clone, Default)]
pub struct SoloParams {
    /// base reward for a block.
    pub block_reward: u64,
    pub enable_hit_handler: bool,
}

impl From<cjson::scheme::SoloParams> for SoloParams {
    fn from(p: cjson::scheme::SoloParams) -> Self {
        SoloParams {
            block_reward: p.block_reward.map_or_else(Default::default, Into::into),
            enable_hit_handler: p.action_handlers.hit.is_some(),
        }
    }
}
