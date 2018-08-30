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

use std::collections::hash_set::Iter;
use std::collections::HashSet;

use ctypes::BlockNumber;
use primitives::H256;

use super::super::CacheableItem;

#[derive(Debug)]
/// Accumulates a list of cacheable item changed in a block.
pub struct BlockChanges<Item: CacheableItem> {
    /// Block number.
    number: BlockNumber,
    /// Block hash.
    hash: H256,
    /// Parent block hash.
    parent: H256,
    /// A set of modified addresses.
    modified_addresses: HashSet<Item::Address>,
    /// Block is part of the canonical chain.
    is_canon: bool,
}

impl<Item: CacheableItem> BlockChanges<Item> {
    pub fn new(
        number: BlockNumber,
        hash: H256,
        parent: H256,
        modified_addresses: HashSet<Item::Address>,
        is_canon: bool,
    ) -> Self {
        Self {
            number,
            hash,
            parent,
            modified_addresses,
            is_canon,
        }
    }

    pub fn is_before(&self, number: &BlockNumber) -> bool {
        &self.number < number
    }

    pub fn hash(&self) -> &H256 {
        &self.hash
    }

    pub fn parent(&self) -> &H256 {
        &self.parent
    }

    pub fn modified_addresses(&self) -> Iter<Item::Address> {
        self.modified_addresses.iter()
    }
    pub fn contains(&self, address: &Item::Address) -> bool {
        self.modified_addresses.contains(address)
    }

    pub fn set_canon(&mut self, is_canon: bool) {
        self.is_canon = is_canon;
    }
    pub fn is_canon(&self) -> bool {
        self.is_canon
    }
}
