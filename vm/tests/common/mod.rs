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

use ctypes::{BlockNumber, Tracker};
use cvm::ChainTimeInfo;

pub struct TestClient {
    block_age: Option<u64>,
    time_age: Option<u64>,
}

impl TestClient {
    pub fn new(block_age: Option<u64>, time_age: Option<u64>) -> Self {
        TestClient {
            block_age,
            time_age,
        }
    }
}

impl Default for TestClient {
    fn default() -> Self {
        Self::new(Some(0), Some(0))
    }
}

impl ChainTimeInfo for TestClient {
    fn transaction_block_age(&self, _: &Tracker, _parent_block_number: BlockNumber) -> Option<u64> {
        self.block_age
    }

    fn transaction_time_age(&self, _: &Tracker, _parent_timestamp: u64) -> Option<u64> {
        self.time_age
    }
}
