// Copyright 2019 Kodebox, Inc.
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

use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use std::collections::BinaryHeap;

pub struct OrderedHeap<T> {
    heap: BinaryHeap<OrderedHeapEntry<T>>,
    seq: usize,
}

impl<T: Ord> OrderedHeap<T> {
    pub fn new() -> OrderedHeap<T> {
        OrderedHeap {
            heap: BinaryHeap::new(),
            seq: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        self.heap.push(OrderedHeapEntry {
            seq: self.seq,
            value,
        });
        self.seq += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        self.heap.pop().map(|x| x.value)
    }
}

#[derive(Debug, Clone)]
struct OrderedHeapEntry<T> {
    seq: usize,
    value: T,
}

impl<T: Ord> Ord for OrderedHeapEntry<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value).then(self.seq.cmp(&other.seq).reverse())
    }
}

impl<T> PartialOrd for OrderedHeapEntry<T>
where
    Self: Ord,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<T> PartialEq for OrderedHeapEntry<T>
where
    Self: Ord,
{
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T> Eq for OrderedHeapEntry<T> where Self: Ord {}
