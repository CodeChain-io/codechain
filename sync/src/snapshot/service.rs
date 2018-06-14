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

use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::spawn;

use ccore::{BlockChainClient, BlockId, ChainNotify};
use ctypes::H256;

pub struct Service {
    client: Arc<BlockChainClient>,
    /// Snapshot root directory
    root_dir: String,
    /// Snapshot creation period in unit of block numbers
    period: u64,
}

impl Service {
    pub fn new(client: Arc<BlockChainClient>, root_dir: String, period: u64) -> Arc<Self> {
        Arc::new(Self {
            client,
            root_dir,
            period,
        })
    }
}

impl ChainNotify for Service {
    /// fires when chain has new blocks.
    fn new_blocks(
        &self,
        _imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
        let best_number = self.client.chain_info().best_block_number;
        let is_checkpoint = enacted
            .iter()
            .map(|hash| self.client.block_number(BlockId::Hash(*hash)).expect("Enacted block must exist"))
            .any(|number| number % self.period == 0);
        if is_checkpoint && best_number > self.period {
            let root_dir = self.root_dir.clone();
            let period = self.period;
            spawn(move || {
                let target = (best_number / period - 1) * period;
                let path: PathBuf = [root_dir, target.to_string()].iter().collect();
                if let Ok(_) = create_dir_all(path) {
                    // FIXME: implement this
                    // unimplemented!()
                }
            });
        }
    }
}
