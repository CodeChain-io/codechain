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

use parking_lot::RwLock;

use super::{ALPHA, K, NodeId, T_REFRESH};
use super::kademlia::Kademlia;
use super::super::Address;
use super::super::discovery::{Api as DiscoveryApi};


pub struct Extension {
    kademlia: RwLock<Kademlia>,
}

impl Extension {
    pub fn new(localhost: NodeId, alpha: Option<u8>, k: Option<u8>, t_refresh: Option<u32>) -> Self {
        let alpha = alpha.unwrap_or(ALPHA);
        let k = k.unwrap_or(K);
        let t_refresh = t_refresh.unwrap_or(T_REFRESH);
        let kademlia = RwLock::new(Kademlia::new(localhost, alpha, k, t_refresh));
        Self {
            kademlia,
        }
    }
}

impl DiscoveryApi for Extension {
    fn get(&self, max: usize) -> Vec<Address> {
        debug_assert!(max <= ::std::u8::MAX as usize);

        let kademlia = self.kademlia.read();
        kademlia.get_closest_addresses(max)
    }

    fn add(&self, address: Address) {
        let mut kademlia = self.kademlia.write();
        kademlia.add(address);
    }

    fn remove(&self, address: &Address) {
        let mut kademlia = self.kademlia.write();
        kademlia.remove(&address);
    }
}

