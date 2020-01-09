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

use super::worker;
use crate::client::ChainNotify;
use crossbeam_channel as crossbeam;
use ctypes::BlockHash;

pub struct TendermintChainNotify {
    inner: crossbeam::Sender<worker::Event>,
}

impl TendermintChainNotify {
    pub fn new(inner: crossbeam::Sender<worker::Event>) -> Self {
        Self {
            inner,
        }
    }
}

impl ChainNotify for TendermintChainNotify {
    /// fires when chain has new blocks.
    fn new_blocks(
        &self,
        imported: Vec<BlockHash>,
        _invalid: Vec<BlockHash>,
        enacted: Vec<BlockHash>,
        _retracted: Vec<BlockHash>,
        _sealed: Vec<BlockHash>,
    ) {
        self.inner
            .send(worker::Event::NewBlocks {
                imported,
                enacted,
            })
            .unwrap();
    }
}
