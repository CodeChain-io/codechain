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

use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Deref;
use std::sync::{Arc, Weak};

use parking_lot::Mutex;
use rlp::Encodable;
use time::Duration;

use super::super::extension::{Api, Error, Extension, NodeToken, Result, TimerToken};

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum Call {
    Send(NodeToken, Vec<u8>),
    Connect(NodeToken),
    SetTimer {
        token: TimerToken,
        duration: Duration,
    },
    SetTimerOnce {
        token: TimerToken,
        duration: Duration,
    },
    ClearTimer(TimerToken),
    SendLocalMessage(Vec<u8>),
}

struct TestApi {
    extension: Weak<Extension>,

    connection_requests: Mutex<HashSet<NodeToken>>,
    connections: Mutex<HashSet<NodeToken>>,
    timers: Mutex<HashMap<TimerToken, (Duration, bool)>>,

    calls: Mutex<VecDeque<Call>>,
}

impl TestApi {
    fn new(extension: Weak<Extension>) -> Arc<Self> {
        Arc::new(Self {
            extension,

            connection_requests: Mutex::new(HashSet::new()),
            connections: Mutex::new(HashSet::new()),
            timers: Mutex::new(HashMap::new()),

            calls: Mutex::new(VecDeque::new()),
        })
    }

    fn extension(&self) -> Arc<Extension> {
        self.extension.upgrade().expect("Extension must be alive")
    }
}

impl Api for TestApi {
    fn send(&self, node: &NodeToken, message: &Vec<u8>) {
        self.calls.lock().push_back(Call::Send(*node, message.clone()));
    }

    fn connect(&self, node: &NodeToken) {
        self.connection_requests.lock().insert(*node);
        self.calls.lock().push_back(Call::Connect(*node));
    }

    fn set_timer(&self, token: TimerToken, duration: Duration) -> Result<()> {
        let mut timers = self.timers.lock();
        if timers.contains_key(&token) {
            panic!("Tried to set timer with token #{} twice", token);
        }
        timers.insert(token, (duration, false));
        self.calls.lock().push_back(Call::SetTimer {
            token,
            duration,
        });
        Ok(())
    }

    fn set_timer_once(&self, token: TimerToken, duration: Duration) -> Result<()> {
        let mut timers = self.timers.lock();
        if timers.contains_key(&token) {
            panic!("Tried to set timer with token #{} twice", token);
        }
        timers.insert(token, (duration, true));
        self.calls.lock().push_back(Call::SetTimerOnce {
            token,
            duration,
        });
        Ok(())
    }

    fn clear_timer(&self, token: TimerToken) -> Result<()> {
        let mut timers = self.timers.lock();
        if !timers.contains_key(&token) {
            panic!("Tried to clear unregistered timer of token #{}", token);
        }
        timers.remove(&token);
        self.calls.lock().push_back(Call::ClearTimer(token));
        Ok(())
    }

    fn send_local_message(&self, message: &Encodable) {
        let message = message.rlp_bytes().into_vec();
        self.calls.lock().push_back(Call::SendLocalMessage(message));
    }
}

impl TestApi {
    fn add_node(&self, token: NodeToken) {
        self.extension().on_node_added(&token);
    }

    fn remove_node(&self, token: NodeToken) {
        if !self.connections.lock().remove(&token) {
            panic!("Tried to remove unregistered node #{}", token);
        }
        self.extension().on_node_removed(&token);
    }

    fn connected(&self, token: NodeToken) {
        let mut connections = self.connections.lock();
        if connections.contains(&token) {
            panic!("Duplicated connection detected for node #{}", token);
        }
        connections.insert(token);
        self.extension().on_connected(&token);
    }

    fn allow_connection(&self, token: NodeToken) {
        let mut connection_requests = self.connection_requests.lock();
        let mut connections = self.connections.lock();

        if connection_requests.contains(&token) && !connections.contains(&token) {
            connection_requests.remove(&token);
            connections.insert(token);
        } else {
            panic!("Invalid connection allowance to node #{}", token);
        }
        self.extension().on_connection_allowed(&token);
    }

    fn deny_connection(&self, token: NodeToken, error: Error) {
        let mut connection_requests = self.connection_requests.lock();

        if connection_requests.contains(&token) {
            connection_requests.remove(&token);
        } else {
            panic!("Invalid connection denial to node #{}", token);
        }
        self.extension().on_connection_denied(&token, error);
    }

    fn send_message(&self, from: NodeToken, message: &[u8]) {
        if !self.connections.lock().contains(&from) {
            panic!("Tried to inject message from unconnected node #{}", from);
        }
        self.extension().on_message(&from, &message.to_vec());
    }

    fn close(&self) {
        self.connections.lock().clear();
    }

    fn call_timeout(&self, token: TimerToken) {
        let extension = self.extension();
        let mut timers = self.timers.lock();
        if let Some(&(_, oneshot)) = timers.get(&token) {
            if oneshot {
                timers.remove(&token);
            }
            extension.on_timeout(token);
        } else {
            panic!("Timer with token #{} is not registered for extension \"{}\"", token, extension.name());
        }
    }
}

pub struct TestClient {
    nodes: HashSet<NodeToken>,
    extensions: HashMap<String, (Arc<Extension>, Arc<TestApi>)>,
}

impl TestClient {
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            extensions: HashMap::new(),
        }
    }

    pub fn register_extension(&mut self, extension: Arc<Extension>) {
        let name = extension.name();

        if self.extensions.contains_key(&name) {
            panic!("Duplicated extension name : {}", name);
        }
        let api = TestApi::new(Arc::downgrade(&extension));
        extension.on_initialize(api.clone());

        self.extensions.insert(name, (extension, api));
    }

    pub fn get_extension<'a>(&'a self, name: &str) -> &'a Extension {
        self.extensions[name].0.deref()
    }

    fn get_api<'a>(&'a self, name: &str) -> &'a TestApi {
        &self.extensions[name].1
    }

    pub fn add_node(&mut self, token: NodeToken) {
        if self.nodes.contains(&token) {
            panic!("Duplicated node #{} detected", token);
        }
        for name in self.extensions.keys() {
            self.get_api(name).add_node(token);
        }
    }

    pub fn remove_node(&self, token: NodeToken) {
        if !self.nodes.contains(&token) {
            panic!("Tried to remove non existent node #{}", token);
        }
        for name in self.extensions.keys() {
            self.get_api(name).remove_node(token);
        }
    }

    pub fn connected(&self, name: &str, token: NodeToken) {
        self.get_api(name).connected(token);
    }

    pub fn allow_connection(&self, name: &str, token: NodeToken) {
        self.get_api(name).allow_connection(token);
    }

    pub fn deny_connection(&self, name: &str, token: NodeToken, error: Error) {
        self.get_api(name).deny_connection(token, error);
    }

    pub fn send_message(&self, name: &str, from: NodeToken, message: &[u8]) {
        self.get_api(name).send_message(from, message);
    }

    pub fn close(&self) {
        for name in self.extensions.keys() {
            self.get_api(name).close();
        }
    }

    pub fn call_timeout(&self, name: &str, token: TimerToken) {
        self.get_api(name).call_timeout(token);
    }

    pub fn pop_call(&self, name: &str) -> Option<Call> {
        self.get_api(name).calls.lock().pop_front()
    }
}
