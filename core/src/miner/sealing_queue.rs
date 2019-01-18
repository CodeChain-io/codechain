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

use crate::block::ClosedBlock;

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
        self.pending.as_ref().or_else(|| self.in_use.last())
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

#[cfg(test)]
mod tests {
    use ckey::Address;

    use super::SealingQueue;
    use crate::block::{ClosedBlock, OpenBlock};
    use crate::scheme::Scheme;
    use crate::tests::helpers::get_temp_state_db;

    const QUEUE_SIZE: usize = 2;

    fn create_closed_block(address: Address) -> ClosedBlock {
        let scheme = Scheme::new_test();
        let genesis_header = scheme.genesis_header();
        let db = scheme.ensure_genesis_state(get_temp_state_db()).unwrap();
        let b = OpenBlock::try_new(&*scheme.engine, db, &genesis_header, address, vec![], false).unwrap();
        let parent_transactions_root = *genesis_header.transactions_root();
        let parent_invoices_root = *genesis_header.invoices_root();
        b.close(parent_transactions_root, parent_invoices_root).unwrap()
    }

    #[test]
    fn fail_to_find_when_pushed() {
        let mut q = SealingQueue::new(QUEUE_SIZE);
        let b = create_closed_block(Address::default());
        let h = b.hash();

        q.push(b);

        assert!(q.take_used_if(|b| b.hash() == h).is_none());
    }

    #[test]
    fn find_when_pushed_and_used() {
        let mut q = SealingQueue::new(QUEUE_SIZE);
        let b = create_closed_block(Address::default());
        let h = b.hash();

        q.push(b);
        q.use_last_ref();

        assert!(q.take_used_if(|b| b.hash() == h).is_some());
    }

    #[test]
    fn find_when_others_used() {
        let mut q = SealingQueue::new(QUEUE_SIZE);
        let b1 = create_closed_block(Address::from(1));
        let b2 = create_closed_block(Address::from(2));
        let h1 = b1.hash();

        q.push(b1);
        q.use_last_ref();
        q.push(b2);
        q.use_last_ref();

        assert!(q.take_used_if(|b| b.hash() == h1).is_some());
    }

    #[test]
    fn fail_to_find_when_too_many_used() {
        let mut q = SealingQueue::new(1);
        let b1 = create_closed_block(Address::from(1));
        let b2 = create_closed_block(Address::from(2));
        let h1 = b1.hash();

        q.push(b1);
        q.use_last_ref();
        q.push(b2);
        q.use_last_ref();

        assert!(q.take_used_if(|b| b.hash() == h1).is_none());
    }

    #[test]
    fn fail_to_find_when_not_used_and_then_pushed() {
        let mut q = SealingQueue::new(QUEUE_SIZE);
        let b1 = create_closed_block(Address::from(1));
        let b2 = create_closed_block(Address::from(2));
        let h1 = b1.hash();

        q.push(b1);
        q.push(b2);
        q.use_last_ref();

        assert!(q.take_used_if(|b| b.hash() == h1).is_none());
    }

    #[test]
    fn peek_correctly_after_push() {
        let mut q = SealingQueue::new(QUEUE_SIZE);
        let b1 = create_closed_block(Address::from(1));
        let b2 = create_closed_block(Address::from(2));
        let h1 = b1.hash();
        let h2 = b2.hash();

        q.push(b1);
        assert_eq!(q.peek_last_ref().unwrap().hash(), h1);

        q.push(b2);
        assert_eq!(q.peek_last_ref().unwrap().hash(), h2);
    }

    #[test]
    fn inspect_correctly() {
        let mut q = SealingQueue::new(QUEUE_SIZE);
        let b1 = create_closed_block(Address::from(1));
        let b2 = create_closed_block(Address::from(2));
        let h1 = b1.hash();
        let h2 = b2.hash();

        q.push(b1);
        assert_eq!(q.use_last_ref().unwrap().hash(), h1);
        assert_eq!(q.peek_last_ref().unwrap().hash(), h1);

        q.push(b2);
        assert_eq!(q.use_last_ref().unwrap().hash(), h2);
        assert_eq!(q.peek_last_ref().unwrap().hash(), h2);
    }

    #[test]
    fn fail_to_find_when_not_used_peeked_and_then_pushed() {
        let mut q = SealingQueue::new(QUEUE_SIZE);
        let b1 = create_closed_block(Address::from(1));
        let b2 = create_closed_block(Address::from(2));
        let h = b1.hash();

        q.push(b1);
        q.peek_last_ref();
        q.push(b2);
        q.use_last_ref();

        assert!(q.take_used_if(|b| b.hash() == h).is_none());
    }
}
