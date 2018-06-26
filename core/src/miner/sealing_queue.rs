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
    /// Not yet being sealed by a miner, but if one asks for work, we'd prefer they do this.
    pending: Option<ClosedBlock>,
    /// Currently being sealed by miners.
    in_use: Vec<ClosedBlock>,
    /// The maximum allowable number of items in_use.
    max_size: usize,
}

impl SealingQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            pending: None,
            in_use: Vec::new(),
            max_size,
        }
    }

    /// Return a reference to the item at the top of the queue (or `None` if the queue is empty);
    /// it doesn't constitute noting that the item is used.
    pub fn peek_last_ref(&self) -> Option<&ClosedBlock> {
        self.pending.as_ref().or(self.in_use.last())
    }

    pub fn push(&mut self, b: ClosedBlock) {
        self.pending = Some(b);
    }

    /// Return a reference to the item at the top of the queue (or `None` if the queue is empty);
    /// this constitutes using the item and will remain in the queue for at least another
    /// `max_size` invocations of `push()`.
    pub fn use_last_ref(&mut self) -> Option<&ClosedBlock> {
        if let Some(x) = self.pending.take() {
            self.in_use.push(x);
            if self.in_use.len() > self.max_size {
                self.in_use.remove(0);
            }
        }
        self.in_use.last()
    }

    /// Clears everything; the queue is entirely reset.
    pub fn reset(&mut self) {
        self.pending = None;
        self.in_use.clear();
    }

    pub fn take_used_if<P>(&mut self, predicate: P) -> Option<ClosedBlock>
    where
        P: Fn(&ClosedBlock) -> bool, {
        self.in_use.iter().position(|r| predicate(r)).map(|i| self.in_use.remove(i))
    }
}
