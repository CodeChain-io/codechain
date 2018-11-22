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

use std::sync::Arc;

use ccore::{EngineInfo, MinerService};
use ckey::PlatformAddress;

use jsonrpc_core::Result;

use super::super::traits::Engine;

pub struct EngineClient<C, M>
where
    C: EngineInfo,
    M: MinerService, {
    client: Arc<C>,
    miner: Arc<M>,
}

impl<C, M> EngineClient<C, M>
where
    C: EngineInfo,
    M: MinerService,
{
    pub fn new(client: Arc<C>, miner: Arc<M>) -> Self {
        Self {
            client,
            miner,
        }
    }
}

impl<C, M> Engine for EngineClient<C, M>
where
    C: EngineInfo + 'static,
    M: MinerService + 'static,
{
    fn get_block_reward(&self, block_number: u64) -> Result<u64> {
        Ok(self.client.block_reward(block_number))
    }

    fn get_coinbase(&self) -> Result<Option<PlatformAddress>> {
        let author = self.miner.authoring_params().author;
        if author.is_zero() {
            Ok(None)
        } else {
            let network_id = self.client.common_params().network_id;
            Ok(Some(PlatformAddress::new_v1(network_id, author)))
        }
    }
}
