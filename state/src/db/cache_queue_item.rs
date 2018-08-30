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

use super::super::CacheableItem;

/// Buffered cache item.
pub struct CacheQueueItem<Item: CacheableItem> {
    address: Item::Address,
    /// Item or `None` if item does not exist.
    item: Option<Item>,
    /// Indicates that the item was modified before being added to the cache.
    modified: bool,
}

impl<Item: CacheableItem> CacheQueueItem<Item> {
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
