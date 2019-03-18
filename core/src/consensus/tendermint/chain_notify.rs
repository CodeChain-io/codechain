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

use crossbeam_channel as crossbeam;
use primitives::H256;

use super::worker;
use crate::client::ChainNotify;

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
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
        self.inner
            .send(worker::Event::NewBlocks {
                imported,
                enacted,
            })
            .unwrap();
    }
}
