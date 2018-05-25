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

use super::super::block::ClosedBlock;

pub struct SealingQueue {
    backing: Vec<ClosedBlock>,
}

impl SealingQueue {
    pub fn new() -> Self {
        Self {
            backing: Vec::new(),
        }
    }

    pub fn push(&mut self, b: ClosedBlock) {
        self.backing.push(b)
    }

    pub fn take_if<P>(&mut self, predicate: P) -> Option<ClosedBlock>
    where
        P: Fn(&ClosedBlock) -> bool, {
        self.backing.iter().position(|r| predicate(r)).map(|i| self.backing.remove(i))
    }
}
