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

use ccore::{self, MinerService};
use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Miner;
use super::super::types::{Bytes, Work};

pub struct MinerClient {
    miner: Arc<ccore::Miner>,
}

impl MinerClient {
    pub fn new(miner: &Arc<ccore::Miner>) -> Self {
        Self {
            miner: miner.clone(),
        }
    }
}

impl Miner for MinerClient {
    fn get_work(&self) -> Result<Work> {
        if !self.miner.can_produce_work_package() {
            cwarn!(MINER, "Cannot give work package - engine seals internally.");
            return Err(errors::no_work_required())
        }
        unimplemented!();
    }

    fn submit_work(&self, _nonce: Bytes, _pow_hash: Bytes) -> Result<bool> {
        if !self.miner.can_produce_work_package() {
            cwarn!(MINER, "Cannot give work package - engine seals internally.");
            return Err(errors::no_work_required())
        }
        unimplemented!();
    }
}
