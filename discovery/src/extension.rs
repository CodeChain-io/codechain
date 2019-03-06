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

use std::collections::HashSet;
use std::sync::Arc;

use cnetwork::{Api, IntoSocketAddr, NetworkExtension, NodeId, RoutingTable};
use ctimer::TimerToken;
use parking_lot::RwLock;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rlp::{Decodable, Encodable, UntrustedRlp};
use time::Duration;

use super::message::Message;
use super::node_id::{address_to_hash, KademliaId};
use super::Config;

pub struct Extension {
    config: Config,
    routing_table: Arc<RoutingTable>,
    api: Arc<Api>,
    nodes: RwLock<HashSet<NodeId>>, // FIXME: Find the optimized data structure for it
    use_kademlia: bool,
}

impl Extension {
    pub fn new(routing_table: Arc<RoutingTable>, config: Config, api: Arc<Api>, use_kademlia: bool) -> Self {
        Self {
            config,
            routing_table,
            api,
            nodes: RwLock::new(HashSet::new()),
            use_kademlia,
        }
    }
}

const REFRESH_TOKEN: TimerToken = 0;

impl NetworkExtension for Extension {
    fn name(&self) -> &'static str {
        "discovery"
    }

    fn need_encryption(&self) -> bool {
        false
    }

    fn versions(&self) -> &[u64] {
        const VERSIONS: &[u64] = &[0];
        &VERSIONS
    }

    fn on_initialize(&self) {
        let name = if self.use_kademlia {
            "kademlia"
        } else {
            "unstructured"
        };
        cinfo!(DISCOVERY, "Discovery starts with {} option", name);
        self.api
            .set_timer(REFRESH_TOKEN, Duration::milliseconds(i64::from(self.config.t_refresh)))
            .expect("Refresh msut be registered");
    }

    fn on_node_added(&self, node: &NodeId, _version: u64) {
        let mut nodes = self.nodes.write();
        nodes.insert(*node);
        self.api.send(&node, &Message::Request(self.config.bucket_size).rlp_bytes());
    }

    fn on_node_removed(&self, node: &NodeId) {
        let mut nodes = self.nodes.write();
        nodes.remove(node);
    }

    fn on_message(&self, node: &NodeId, message: &[u8]) {
        let message = match Message::decode(&UntrustedRlp::new(&message)) {
            Ok(message) => message,
            Err(err) => {
                cwarn!(DISCOVERY, "Invalid message from {} : {:?}", node, err);
                return
            }
        };
        match message {
            Message::Request(len) => {
                let addresses = if self.use_kademlia {
                    let datum = address_to_hash(&node.into_addr());
                    let mut addresses = self
                        .routing_table
                        .reachable_addresses(&node.into_addr())
                        .into_iter()
                        .map(|address| KademliaId::new(address, &datum))
                        .collect::<Vec<_>>();

                    addresses.sort_unstable();

                    addresses
                        .into_iter()
                        .map(|kademlia_id| kademlia_id.into())
                        .take(::std::cmp::min(self.config.bucket_size, len) as usize)
                        .collect()
                } else {
                    let mut addresses = self.routing_table.reachable_addresses(&node.into_addr());
                    addresses.shuffle(&mut thread_rng());
                    addresses.into_iter().take(::std::cmp::min(self.config.bucket_size, len) as usize).collect()
                };
                let response = Message::Response(addresses).rlp_bytes();
                self.api.send(&node, &response);
            }
            Message::Response(addresses) => {
                self.routing_table.touch_addresses(addresses);
            }
        }
    }

    fn on_timeout(&self, timer: TimerToken) {
        match timer {
            REFRESH_TOKEN => {
                let nodes = self.nodes.read();

                let request = Message::Request(self.config.bucket_size).rlp_bytes();
                for node in nodes.iter() {
                    self.api.send(&node, &request);
                }
            }
            _ => unreachable!(),
        }
    }
}
