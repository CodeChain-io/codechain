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

use cnetwork::{Api, DiscoveryApi, NetworkExtension, NodeToken, SocketAddr, TimerToken};
use parking_lot::{Mutex, RwLock};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};
use time::Duration;

use super::command::Command;
use super::config::Config;
use super::event::Event;
use super::kademlia::Kademlia;
use super::message::Message;


pub struct Extension {
    kademlia: RwLock<Kademlia>,
    events: Mutex<VecDeque<Event>>,
    event_fired: AtomicBool,
    api: Mutex<Option<Arc<Api>>>,

    addr_to_node: RwLock<HashMap<SocketAddr, NodeToken>>,
    node_to_addr: RwLock<HashMap<NodeToken, SocketAddr>>,
}

const CONSUME_EVENT_TOKEN: TimerToken = 0;
const REFRESH_TOKEN: TimerToken = 1;

impl Extension {
    pub fn new(config: Config) -> Self {
        let kademlia = RwLock::new(Kademlia::new(config.node_id, config.alpha, config.k, config.t_refresh));
        Self {
            kademlia,
            events: Mutex::new(VecDeque::new()),
            event_fired: AtomicBool::new(false),
            api: Mutex::new(None),

            addr_to_node: RwLock::new(HashMap::new()),
            node_to_addr: RwLock::new(HashMap::new()),
        }
    }

    fn on_receive(&self, node: &NodeToken, message: &[u8]) -> ::std::result::Result<(), DecoderError> {
        if let Some(sender) = self.get_address(&node) {
            let rlp = UntrustedRlp::new(message);
            let message: Message = Decodable::decode(&rlp)?;
            let event = Event::Message {
                message,
                sender: sender.clone(),
            };
            self.push_event(event)
        }
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
            if let &Some(ref api) = &*api {
                api.set_timer_once(CONSUME_EVENT_TOKEN, Duration::milliseconds(0))
                    .expect("Consume event must be registered");
            }
        }
    }

    fn get_address(&self, node: &NodeToken) -> Option<SocketAddr> {
        self.node_to_addr.read().get(node).map(Clone::clone)
    }

    fn get_node_token(&self, address: &SocketAddr) -> Option<NodeToken> {
        self.addr_to_node.read().get(address).map(Clone::clone)
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

            let command = {
                match event {
                    Event::Message {
                        ref message,
                        ref sender,
                    } => {
                        let mut kademlia = self.kademlia.write();
                        kademlia.handle_message(message, sender)
                    }
                    Event::Command(ref command) => self.handle_command(command),
                }
            };

            if let Some(command) = command {
                self.push_event(Event::Command(command));
            }
        }
    }

    fn handle_command(&self, command: &Command) -> Option<Command> {
        match command {
            &Command::Verify => {
                let mut kademlia = self.kademlia.write();
                kademlia.handle_verify_command()
            }
            &Command::Refresh => {
                let mut kademlia = self.kademlia.write();
                kademlia.handle_refresh_command()
            }
            &Command::Send {
                ref message,
                ref target,
            } => {
                self.handle_send_command(&message, &target);
                None
            }
        }
    }

    fn handle_send_command(&self, message: &Message, target: &SocketAddr) {
        let api = self.api.lock();
        if let &Some(ref api) = &*api {
            if let Some(node) = self.get_node_token(&target) {
                api.send(&node, &message.rlp_bytes().to_vec())
            }
        }
    }
}

impl DiscoveryApi for Extension {
    fn get(&self, max: usize) -> Vec<SocketAddr> {
        debug_assert!(max <= ::std::u8::MAX as usize);

        let kademlia = self.kademlia.read();
        kademlia.get_closest_addresses(max)
    }

    fn add_connection(&self, node: NodeToken, address: SocketAddr) {
        let mut addr_to_node = self.addr_to_node.write();
        let mut node_to_addr = self.node_to_addr.write();

        addr_to_node.insert(address.clone(), node.clone());
        node_to_addr.insert(node.clone(), address.clone());
    }

    fn remove_connection(&self, node: &NodeToken) {
        let mut addr_to_node = self.addr_to_node.write();
        let mut node_to_addr = self.node_to_addr.write();

        if let Some(addr) = node_to_addr.remove(node) {
            addr_to_node.remove(&addr);
        }
    }
}

impl NetworkExtension for Extension {
    fn name(&self) -> String {
        "kademlia".to_string()
    }

    fn need_encryption(&self) -> bool {
        false
    }

    fn on_initialize(&self, api: Arc<Api>) {
        let kademlia = self.kademlia.read();
        {
            let mut api_guard = self.api.lock();
            *api_guard = Some(Arc::clone(&api));
        }
        let t_refresh = Duration::milliseconds(kademlia.t_refresh as i64);
        api.set_timer(REFRESH_TOKEN, t_refresh).expect("Refresh must be registered");
    }

    fn on_node_added(&self, node: &NodeToken) {
        let mut kademlia = self.kademlia.write();
        let node_to_addr = self.node_to_addr.read();

        if let Some(address) = node_to_addr.get(node).map(Clone::clone) {
            let event = {
                let command = kademlia.find_node_command(address);
                Event::Command(command)
            };
            self.push_event(event);
        }
    }

    fn on_node_removed(&self, node: &NodeToken) {
        let mut kademlia = self.kademlia.write();

        let address = self.node_to_addr.write().remove(node);
        if let Some(address) = address {
            self.addr_to_node.write().remove(&address);
            kademlia.remove(&address);
        }
    }

    fn on_message(&self, node: &NodeToken, message: &[u8]) {
        if let Err(err) = self.on_receive(node, message) {
            warn!(target: "discovery", "Invalid message from {} : {:?}", node, err);
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
    use std::sync::Arc;

    use cnetwork::{NetworkExtension, SocketAddr, TestNetworkCall, TestNetworkClient};
    use time::Duration;

    use super::{Config, DiscoveryApi, Extension, NodeToken};

    #[derive(Clone)]
    struct Node {
        token: NodeToken,
        address: SocketAddr,
    }

    lazy_static! {
        static ref NODES: [Node; 1] = [Node {
            token: 1,
            address: SocketAddr::v4(127, 0, 0, 1, 3481),
        }];
    }

    #[test]
    fn test_add_node() {
        let config = Config::new(None, None, None, None);
        let default_refresh = config.t_refresh;
        let extension = Arc::new(Extension::new(config));


        let mut client = TestNetworkClient::new();
        client.register_extension(extension.clone());

        let command = client.pop_call(&extension.name());
        assert_eq!(
            Some(TestNetworkCall::SetTimer {
                token: 1,
                duration: Duration::milliseconds(default_refresh as i64),
            }),
            command
        );

        extension.add_connection(NODES[0].token.clone(), NODES[0].address.clone());

        let command = client.pop_call(&extension.name());
        assert_eq!(None, command);

        client.add_node(NODES[0].token);

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
