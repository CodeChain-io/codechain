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

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cnetwork::{Api, DiscoveryApi, IntoSocketAddr, NetworkExtension, NodeId, RoutingTable, SocketAddr, TimerToken};
use parking_lot::{Mutex, RwLock};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};
use time::Duration;

use super::command::Command;
use super::config::Config;
use super::event::Event;
use super::kademlia::Kademlia;
use super::message::Message;


pub struct Extension {
    kademlias: RwLock<HashMap<NodeId, Kademlia>>,
    config: Config,
    events: Mutex<VecDeque<Event>>,
    event_fired: AtomicBool,

    routing_table: RwLock<Option<Arc<RoutingTable>>>,
    api: Mutex<Option<Arc<Api>>>,
}

const CONSUME_EVENT_TOKEN: TimerToken = 0;
const REFRESH_TOKEN: TimerToken = 1;

impl Extension {
    pub fn new(config: Config) -> Self {
        Self {
            kademlias: RwLock::new(HashMap::new()),
            config,
            events: Mutex::new(VecDeque::new()),
            event_fired: AtomicBool::new(false),

            routing_table: RwLock::new(None),
            api: Mutex::new(None),
        }
    }

    fn on_receive(&self, node: &NodeId, message: &[u8]) -> ::std::result::Result<(), DecoderError> {
        let sender = node.into_addr();
        let rlp = UntrustedRlp::new(message);
        let message: Message = Decodable::decode(&rlp)?;
        let event = Event::Message {
            message,
            sender,
        };
        self.push_event(event);
        Ok(())
    }

    fn push_event(&self, event: Event) {
        let already_fired = {
            let mut events = self.events.lock();
            events.push_back(event);
            self.event_fired.swap(true, Ordering::SeqCst)
        };
        if !already_fired {
            let api = self.api.lock();
            if let Some(api) = &*api {
                api.set_timer_once(CONSUME_EVENT_TOKEN, Duration::milliseconds(0))
                    .expect("Consume event must be registered");
            }
        }
    }

    fn consume_events(&self) {
        loop {
            let event = {
                let mut events = self.events.lock();
                let event = events.pop_front();
                if event.is_none() {
                    let _ = self.event_fired.swap(false, Ordering::SeqCst);
                    break
                }
                event.expect("Already check none")
            };

            let commands = {
                match event {
                    Event::Message {
                        message,
                        sender,
                    } => {
                        let mut kademlias = self.kademlias.write();
                        kademlias.values_mut().flat_map(|kademlia| kademlia.handle_message(&message, &sender)).collect()
                    }
                    Event::Command(ref command) => self.handle_command(command),
                }
            };

            for command in commands {
                self.push_event(Event::Command(command));
            }
        }
    }

    fn handle_command(&self, command: &Command) -> Vec<Command> {
        match command {
            Command::Verify => {
                let mut kademlias = self.kademlias.write();
                kademlias.values_mut().flat_map(|kademlia| kademlia.handle_verify_command()).collect()
            }
            Command::Refresh => {
                let mut kademlias = self.kademlias.write();
                kademlias.values_mut().flat_map(|kademlia| kademlia.handle_refresh_command()).collect()
            }
            Command::Send {
                message,
                target,
            } => {
                self.handle_send_command(&message, &target);
                vec![]
            }
        }
    }

    fn handle_send_command(&self, message: &Message, target: &SocketAddr) {
        let api = self.api.lock();
        if let Some(api) = &*api {
            let node = target.into();
            api.send(&node, &message.rlp_bytes().to_vec())
        }
    }
}

impl DiscoveryApi for Extension {
    fn set_routing_table(&self, routing_table: Arc<RoutingTable>) {
        *self.routing_table.write() = Some(routing_table);
    }
}

impl NetworkExtension for Extension {
    fn name(&self) -> String {
        "kademlia".to_string()
    }

    fn need_encryption(&self) -> bool {
        false
    }

    fn versions(&self) -> Vec<u64> {
        vec![0]
    }

    fn on_initialize(&self, api: Arc<Api>) {
        let mut api_guard = self.api.lock();
        let t_refresh = Duration::milliseconds(self.config.t_refresh as i64);
        api.set_timer(REFRESH_TOKEN, t_refresh).expect("Refresh must be registered");
        *api_guard = Some(Arc::clone(&api));
    }

    fn on_node_added(&self, node: &NodeId, _version: u64) {
        let mut kademlias = self.kademlias.write();
        let routing_table = self.routing_table.read();

        routing_table.as_ref().map(|routing_table| {
            match routing_table.local_node_id(node) {
                Some(local_node_id) => {
                    if !kademlias.contains_key(&local_node_id) {
                        let t = kademlias.insert(
                            local_node_id.clone(),
                            Kademlia::new(local_node_id, self.config.k, self.config.t_refresh),
                        );
                        debug_assert!(t.is_none());
                    }
                }
                None => {
                    cwarn!(DISCOVERY, "Cannot find routing table");
                    return
                }
            };
            let address = node.into_addr();
            for kademlia in kademlias.values_mut() {
                let event = {
                    let command = kademlia.find_node_command(address.clone());
                    Event::Command(command)
                };
                self.push_event(event);
            }
        });
    }

    fn on_node_removed(&self, node: &NodeId) {
        let mut kademlias = self.kademlias.write();
        let address = node.into_addr();
        kademlias
            .values_mut()
            .map(|ref mut kademlia| {
                kademlia.remove(&address);
            })
            .collect::<Vec<_>>();
    }

    fn on_message(&self, node: &NodeId, message: &[u8]) {
        if let Err(err) = self.on_receive(node, message) {
            cwarn!(DISCOVERY, "Invalid message from {} : {:?}", node, err);
        }
    }

    fn on_timeout(&self, timer: TimerToken) {
        match timer {
            CONSUME_EVENT_TOKEN => {
                self.consume_events();
            }
            REFRESH_TOKEN => {
                let command = Command::Refresh;
                let event = Event::Command(command);
                self.push_event(event);
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use cnetwork::{IntoSocketAddr, NodeId as NetworkNodeId, TestNetworkCall, TestNetworkClient};

    use super::*;

    lazy_static! {
        static ref NODES: [NetworkNodeId; 1] = [SocketAddr::v4(127, 0, 0, 1, 3480).into(),];
    }

    fn dummy_routing_table() -> Arc<RoutingTable> {
        let routing_table = RoutingTable::new();
        let node_id = NODES[0].clone();
        routing_table.add_candidate(node_id.into_addr().clone());
        routing_table.add_node(&node_id.into_addr(), node_id.clone());
        routing_table
    }

    #[test]
    fn test_add_node() {
        let config = Config::new(None, None);
        let default_refresh = config.t_refresh;
        let extension = Arc::new(Extension::new(config));

        let mut client = TestNetworkClient::new();
        client.register_extension(extension.clone());
        extension.set_routing_table(dummy_routing_table());

        let command = client.pop_call(&extension.name());
        assert_eq!(
            Some(TestNetworkCall::SetTimer {
                token: 1,
                duration: Duration::milliseconds(default_refresh as i64),
            }),
            command
        );

        let command = client.pop_call(&extension.name());
        assert_eq!(None, command);

        client.add_node(&extension.name(), NODES[0].clone());

        let command = client.pop_call(&extension.name());
        assert_eq!(
            Some(TestNetworkCall::SetTimerOnce {
                token: 0,
                duration: Duration::milliseconds(0),
            }),
            command
        );

        let command = client.pop_call(&extension.name());
        assert_eq!(None, command);

        let command = client.pop_call(&extension.name());
        assert_eq!(None, command);
    }
}
