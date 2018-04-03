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

use std::sync::Arc;

use parking_lot::RwLock;
use rlp::{Decodable, DecoderError, UntrustedRlp};

use super::{ALPHA, Config, K, T_REFRESH};
use super::kademlia::Kademlia;
use super::super::Address;
use super::super::connection::AddressConverter;
use super::super::discovery::Api as DiscoveryApi;
use super::super::extension::NodeId as ExtensionNodeId;


pub struct Extension {
    kademlia: RwLock<Kademlia>,
    converter: RwLock<Arc<AddressConverter>>,
}

struct DummyConverter;
impl DummyConverter {
    fn new() -> Arc<Self> {
        Arc::new(Self {})
    }
}

impl AddressConverter for DummyConverter {
    fn node_id_to_address(&self, _node_id: &ExtensionNodeId) -> Option<Address> {
        None
    }

    fn address_to_node_id(&self, address: &Address) -> Option<usize> {
        None
    }
}

impl Extension {
    pub fn new(config: Config) -> Self {
        let kademlia = RwLock::new(Kademlia::new(config.node_id, config.alpha, config.k, config.t_refresh));
        Self {
            kademlia,
            converter: RwLock::new(DummyConverter::new()),
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

    fn set_address_converter(&self, converter: Arc<AddressConverter>) {
        *self.converter.write() = converter;
    }
}

