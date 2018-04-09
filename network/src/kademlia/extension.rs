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

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cio::TimerToken;
use parking_lot::{Mutex, RwLock};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};

use super::super::SocketAddr;
use super::super::connection::AddressConverter;
use super::super::discovery::Api as DiscoveryApi;
use super::super::extension::NodeId as ExtensionNodeId;
use super::super::{Api, Extension as NetworkExtension};
use super::config::Config;
use super::command::Command;
use super::event::Event;
use super::kademlia::Kademlia;
use super::message::Message;


pub struct Extension {
    kademlia: RwLock<Kademlia>,
    events: Mutex<VecDeque<Event>>,
    event_fired: AtomicBool,
    api: Mutex<Option<Arc<Api>>>,
    converter: RwLock<Arc<AddressConverter>>,
    active_nodes: RwLock<HashSet<ExtensionNodeId>>,
}

struct DummyConverter;
impl DummyConverter {
    fn new() -> Arc<Self> {
        Arc::new(Self {})
    }
}

impl AddressConverter for DummyConverter {
    fn node_id_to_address(&self, _node_id: &ExtensionNodeId) -> Option<SocketAddr> {
        None
    }

    fn address_to_node_id(&self, _address: &SocketAddr) -> Option<usize> {
        None
    }
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
            converter: RwLock::new(DummyConverter::new()),
            active_nodes: RwLock::new(HashSet::new()),
        }
    }

    fn on_receive(&self, id: &ExtensionNodeId, message: &Vec<u8>) -> ::std::result::Result<(), DecoderError> {
        if let Some(sender) = self.get_address(&id) {
            let rlp = UntrustedRlp::new(&message.as_slice());
            let message: Message = Decodable::decode(&rlp)?;
            let event = Event::Message {
                message,
                sender,
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
                api.set_timer_once(CONSUME_EVENT_TOKEN, 0);
            }
        }
    }

    fn get_address(&self, id: &ExtensionNodeId) -> Option<SocketAddr> {
        let converter = self.converter.read();
        converter.node_id_to_address(id)
    }

    fn get_node_id(&self, address: &SocketAddr) -> Option<ExtensionNodeId> {
        let converter = self.converter.read();
        converter.address_to_node_id(&address)
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
            if let Some(id) = self.get_node_id(&target) {
                api.send(&id, &message.rlp_bytes().to_vec())
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

    fn add(&self, address: SocketAddr) {
        let event = {
            let mut kademlia = self.kademlia.write();
            let command = kademlia.find_node_command(address);
            Event::Command(command)
        };
        self.push_event(event);
    }

    fn remove(&self, address: &SocketAddr) {
        let mut kademlia = self.kademlia.write();
        kademlia.remove(&address);
    }

    fn set_address_converter(&self, converter: Arc<AddressConverter>) {
        *self.converter.write() = converter;
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
        let api_clone = Arc::clone(&api);
        *self.api.lock() = Some(api);
        let t_refresh = {
            let kademlia = self.kademlia.read();
            kademlia.t_refresh as u64
        };
        api_clone.set_timer(REFRESH_TOKEN, t_refresh);
    }

    fn on_node_added(&self, id: &ExtensionNodeId) {
        if let Some(address) = self.get_address(&id) {
            self.add(address);
            let mut active_nodes = self.active_nodes.write();
            active_nodes.insert(*id);
        }
    }

    fn on_node_removed(&self, id: &ExtensionNodeId) {
        if let Some(address) = self.get_address(&id) {
            self.remove(&address);

            let mut active_nodes = self.active_nodes.write();
            active_nodes.remove(&id);
        }
    }

    fn on_message(&self, id: &ExtensionNodeId, message: &Vec<u8>) {
        if let Err(err) = self.on_receive(id, message) {
            warn!("Invalid message from {} : {:?}", id, err);
        }
    }

    fn on_timeout(&self, token: TimerToken) {
        match token {
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
