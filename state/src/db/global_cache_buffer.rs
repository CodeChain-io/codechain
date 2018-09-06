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

use std::collections::HashSet;

use ctypes::BlockNumber;
use primitives::H256;

use super::super::CacheableItem;
use super::block_changes::BlockChanges;
use super::global_cache::GlobalCache;

pub struct GlobalCacheBuffer<Item: CacheableItem> {
    queue: Vec<QueuedItem<Item>>,
}

impl<Item: CacheableItem> GlobalCacheBuffer<Item> {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
        }
    }

    pub fn push(&mut self, addr: Item::Address, item: Option<Item>, modified: bool) {
        self.queue.push(QueuedItem::new(addr, item, modified));
    }

    pub fn sync_cache(
        &mut self,
        cache: &mut GlobalCache<Item>,
        number: BlockNumber,
        hash: H256,
        parent: H256,
        is_best: bool,
    ) {
        cache.keep_size();
        let mut modified_addresses = HashSet::new();
        ctrace!(STATE_DB, "committing {} cache entries", self.queue.len());
        for local_item in self.queue.drain(..) {
            let is_modified = local_item.is_modified();
            let (address, item) = local_item.drain();
            if is_modified {
                modified_addresses.insert(address.clone());
            }
            if is_best {
                if let Some(Some(existing)) = cache.get_mut(&address) {
                    if let Some(new) = item {
                        if is_modified {
                            *existing = new;
                        }
                        continue
                    }
                }
                cache.insert(address, item);
            }
        }

        // Save modified addresses. These are ordered by the block number.
        let block_changes = BlockChanges::new(number, hash, parent, modified_addresses, is_best);
        cache.save(block_changes);
    }
}

/// Buffered cache item.
struct QueuedItem<Item: CacheableItem> {
    address: Item::Address,
    /// Item or `None` if item does not exist.
    item: Option<Item>,
    /// Indicates that the item was modified before being added to the cache.
    modified: bool,
}

impl<Item: CacheableItem> QueuedItem<Item> {
    pub fn new(address: Item::Address, item: Option<Item>, modified: bool) -> Self {
        Self {
            address,
            item,
            modified,
        }
    }

    pub fn drain(self) -> (Item::Address, Option<Item>) {
        (self.address, self.item)
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }
}
