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

#[derive(Clone)]
pub struct Peer {
    total_score: U256,
    best_hash: H256,
}

impl Peer {
    pub fn new(total_score: U256, best_hash: H256) -> Self {
        Self {
            total_score,
            best_hash,
        }
    }

    pub fn update(&mut self, total_score: U256, best_hash: H256) {
        self.total_score = total_score;
        self.best_hash = best_hash;
    }
}
