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

use slab::Slab;

pub type Key = usize;

pub struct LimitedTable<Item> {
    slab: Slab<Item>,
    begin: Key,
    limit: usize,
}

impl<Item> LimitedTable<Item> {
    pub fn new(begin: Key, limit: usize) -> Self {
        Self {
            slab: Slab::with_capacity(limit),
            begin,
            limit,
        }
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn len(&self) -> usize {
        self.slab.len()
    }

    pub fn capacity(&self) -> usize {
        let capacity = self.slab.capacity();
        debug_assert_eq!(self.limit(), capacity);
        capacity
    }

    pub fn is_full(&self) -> bool {
        self.limit() == self.len()
    }

    pub fn insert(&mut self, item: Item) -> Option<Key> {
        if self.is_full() {
            return None
        }
        Some(self.begin + self.slab.insert(item))
    }

    pub fn remove(&mut self, key: Key) -> Option<Item> {
        if key < self.begin {
            return None
        }
        let key = key - self.begin;
        if !self.slab.contains(key) {
            return None
        }
        Some(self.slab.remove(key))
    }

    pub fn contains(&self, key: Key) -> bool {
        self.slab.contains(key - self.begin)
    }

    pub fn get(&self, key: Key) -> Option<&Item> {
        self.slab.get(key - self.begin)
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, key: Key) -> Option<&mut Item> {
        self.slab.get_mut(key - self.begin)
    }
}

#[cfg(test)]
mod tests {
    use super::Key;
    use super::LimitedTable;
    struct TestItem;

    #[test]
    fn limit() {
        let begin = 11;
        let limit = 54;
        let table: LimitedTable<TestItem> = LimitedTable::new(begin, limit);
        assert_eq!(limit, table.limit());
    }

    #[test]
    fn empty_table_len_is_zero() {
        let begin = 11;
        let limit = 54;
        let table: LimitedTable<TestItem> = LimitedTable::new(begin, limit);
        assert_eq!(0, table.len());
    }

    #[test]
    fn len_of_inserted_table() {
        let begin: Key = 11;
        let limit: usize = 54;
        let mut table: LimitedTable<TestItem> = LimitedTable::new(begin, limit);
        assert_eq!(0, table.len());
        let t1 = table.insert(TestItem);
        assert!(t1.is_some());
        assert_eq!(1, table.len());
        let t2 = table.insert(TestItem);
        assert!(t2.is_some());
        assert_eq!(2, table.len());
        let t3 = table.insert(TestItem);
        assert!(t3.is_some());
        assert_eq!(3, table.len());
        let t4 = table.insert(TestItem);
        assert!(t4.is_some());
        assert_eq!(4, table.len());
    }

    #[test]
    fn get_returns_none_when_the_item_is_removed() {
        let begin: Key = 11;
        let limit: usize = 54;
        let mut table: LimitedTable<TestItem> = LimitedTable::new(begin, limit);
        let t1 = table.insert(TestItem);
        assert!(t1.is_some());
        let t2 = table.insert(TestItem);
        assert!(t2.is_some());
        let t3 = table.insert(TestItem);
        assert!(t3.is_some());
        let t4 = table.insert(TestItem);
        assert!(t4.is_some());
        assert!(table.get(t4.unwrap()).is_some());
        let _ = table.remove(t4.unwrap());
        assert!(table.get(t4.unwrap()).is_none());
    }

    #[test]
    fn insert_returns_none_when_the_table_is_full() {
        let begin: Key = 11;
        let limit: usize = 20;
        let mut table: LimitedTable<TestItem> = LimitedTable::new(begin, limit);
        for _ in begin..(begin + limit) {
            let t = table.insert(TestItem);
            assert!(table.get(t.unwrap()).is_some());
        }
        let t = table.insert(TestItem);
        assert!(t.is_none());
    }

    #[test]
    fn contains_returns_true_when_item_is_inserted() {
        let begin: Key = 11;
        let limit: usize = 54;
        let mut table: LimitedTable<TestItem> = LimitedTable::new(begin, limit);
        let t1 = table.insert(TestItem);
        assert_ne!(None, t1);
        assert!(table.contains(t1.unwrap()));
    }
}
