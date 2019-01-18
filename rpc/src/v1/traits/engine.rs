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

use cjson::bytes::{Bytes, WithoutPrefix};
use ckey::PlatformAddress;

use jsonrpc_core::Result;

build_rpc_trait! {
    pub trait Engine {
        /// Gets the reward of the given block number
        # [rpc(name = "engine_getBlockReward")]
        fn get_block_reward(&self, u64) -> Result<u64>;

        /// Gets coinbase's account id
        # [rpc(name = "engine_getCoinbase")]
        fn get_coinbase(&self) -> Result<Option<PlatformAddress>>;

        /// Gets the recommended minimum confirmations
        # [rpc(name = "engine_getRecommendedConfirmation")]
        fn get_recommended_confirmation(&self) -> Result<u32>;

        /// Gets custom action data for given custom action handler id and rlp encoded key.
        # [rpc(name = "engine_getCustomActionData")]
        fn get_custom_action_data(&self, u64, Bytes, Option<u64>) -> Result<Option<WithoutPrefix<Bytes>>>;
    }
}
