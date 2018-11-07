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

// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.
//
// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use std::collections::VecDeque;
use std::env;

use lru_cache::LruCache;
use primitives::H256;

use super::super::CacheableItem;
use super::block_changes::BlockChanges;

/// Shared canonical state cache.
pub struct GlobalCache<Item: CacheableItem> {
    /// `None` indicates that item is known to be missing.
    // When changing the type of the values here, be sure to update `mem_used` and
    // `new`.
    cache: LruCache<Item::Address, Option<Item>>,
    /// Information on the modifications in recently committed blocks; specifically which addresses
    /// changed in which block. Ordered by block number in descending order.
    modifications: VecDeque<BlockChanges<Item>>,
}

const STATE_CACHE_BLOCKS: usize = 12;

impl<Item: CacheableItem> GlobalCache<Item> {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(capacity),
            modifications: VecDeque::new(),
        }
    }

    pub fn keep_size(&mut self) {
        if self.modifications.len() >= STATE_CACHE_BLOCKS {
            self.modifications.pop_back();
        }
    }

    pub fn is_allowed(&self, addr: &Item::Address, parent_hash: &Option<H256>) -> bool {
        let mut parent = match parent_hash {
            None => {
                ctrace!(STATE_DB, "Cache lookup skipped for {:?}: no parent hash", addr);
                return false
            }
            Some(parent) => parent,
        };

        if self.modifications.is_empty() {
            return true
        }
        // Ignore all accounts modified in later blocks
        // Modifications contains block ordered by the number
        // We search for our parent in that list first and then for
        // all its parent until we hit the canonical block,
        // checking against all the intermediate modifications.
        for m in self.modifications.iter() {
            if m.hash() == parent {
                if m.is_canon() {
                    return true
                }
                parent = m.parent();
            }
            if m.contains(addr) {
                ctrace!(STATE_DB, "Cache lookup skipped for {:?}: modified in a later block", addr);
                return false
            }
        }

        return false
    }

    // Save modified addresses. These are ordered by the block number.
    pub fn save(&mut self, block_changes: BlockChanges<Item>) {
        let insert_at = {
            let number = block_changes.number();
            self.modifications.iter().enumerate().find(|&(_, m)| m.is_before(number)).map(|(i, _)| i)
        };
        ctrace!(STATE_DB, "inserting modifications at {:?}", insert_at);
        if let Some(insert_at) = insert_at {
            self.modifications.insert(insert_at, block_changes);
        } else {
            self.modifications.push_back(block_changes);
        }
    }

    pub fn insert(&mut self, addr: Item::Address, item: Option<Item>) {
        self.cache.insert(addr, item);
    }

    pub fn get_mut(&mut self, addr: &Item::Address) -> Option<&mut Option<Item>> {
        if !is_global_cache_enabled() {
            return None
        }
        self.cache.get_mut(addr)
    }

    pub fn enact(&mut self, block: &H256) -> bool {
        self.update(block, true)
    }

    pub fn retract(&mut self, block: &H256) -> bool {
        self.update(block, false)
    }

    // return true if there is an update
    fn update(&mut self, block: &H256, is_enact: bool) -> bool {
        let target = self.modifications.iter_mut().find(|m| m.hash() == block);
        if let Some(m) = target {
            if is_enact {
                ctrace!(STATE_DB, "Reverting enacted block {:?}", block);
            } else {
                ctrace!(STATE_DB, "Retracting block {:?}", block);
            }
            m.set_canon(is_enact);
            for a in m.modified_addresses() {
                if is_enact {
                    ctrace!(STATE_DB, "Reverting enacted address {:?}", a);
                } else {
                    ctrace!(STATE_DB, "Retracted address {:?}", a);
                }
                self.cache.remove(&a);
            }
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.modifications.clear();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }
}

fn is_global_cache_enabled() -> bool {
    lazy_static! {
        static ref CACHE_ENABLED: bool = {
            const DEFAULT: bool = true;

            let enabled = env::var_os("ENABLE_GLOBAL_CACHE")
                .and_then(|enable| enable.to_str().map(|enable| enable != "0"))
                .unwrap_or(DEFAULT);
            enabled
        };
    }
    *CACHE_ENABLED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Default, RlpEncodableWrapper, RlpDecodableWrapper)]
    struct Item(usize);
    impl CacheableItem for Item {
        type Address = [u8; 1];
        fn is_null(&self) -> bool {
            self.0 == 0
        }
    }

    #[test]
    fn modifications_are_saved_in_reverse_order() {
        let mut cache = GlobalCache::<Item>::new(10);
        cache.save(BlockChanges::new(5, Default::default(), Default::default(), Default::default(), true));
        cache.save(BlockChanges::new(7, Default::default(), Default::default(), Default::default(), true));
        cache.save(BlockChanges::new(6, Default::default(), Default::default(), Default::default(), true));
        cache.save(BlockChanges::new(11, Default::default(), Default::default(), Default::default(), true));
        cache.save(BlockChanges::new(9, Default::default(), Default::default(), Default::default(), true));
        cache.save(BlockChanges::new(1, Default::default(), Default::default(), Default::default(), true));
        cache.save(BlockChanges::new(3, Default::default(), Default::default(), Default::default(), true));

        let mut iter = cache.modifications.into_iter();
        assert_eq!(Some(11), iter.next().map(|b| *b.number()));
        assert_eq!(Some(9), iter.next().map(|b| *b.number()));
        assert_eq!(Some(7), iter.next().map(|b| *b.number()));
        assert_eq!(Some(6), iter.next().map(|b| *b.number()));
        assert_eq!(Some(5), iter.next().map(|b| *b.number()));
        assert_eq!(Some(3), iter.next().map(|b| *b.number()));
        assert_eq!(Some(1), iter.next().map(|b| *b.number()));
    }

    #[test]
    fn the_oldest_modifications_are_removed() {
        let mut cache = GlobalCache::<Item>::new(10);
        for i in 0..(STATE_CACHE_BLOCKS + 1) {
            cache.save(BlockChanges::new(i as u64, Default::default(), Default::default(), Default::default(), true));
        }

        assert_eq!(STATE_CACHE_BLOCKS + 1, cache.modifications.len());
        cache.keep_size();
        assert_eq!(STATE_CACHE_BLOCKS, cache.modifications.len());

        let mut iter = cache.modifications.into_iter().rev();
        for i in 1..(STATE_CACHE_BLOCKS + 1) {
            assert_eq!(Some(i as u64), iter.next().map(|b| *b.number()));
        }
    }
}
