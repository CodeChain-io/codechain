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

use parking_lot::Mutex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Weak};

use extension::{Api, Extension, NodeToken, TimerToken};

struct TestApi {
    extension: Weak<Extension>,

    connections: Mutex<HashSet<NodeToken>>,
    timers: Mutex<HashMap<TimerToken, (u64, bool)>>,

    messages: Mutex<VecDeque<(NodeToken, Vec<u8>)>>,
}

impl TestApi {
    fn new(extension: Weak<Extension>) -> Arc<Self> {
        Arc::new(Self {
            extension,

            connections: Mutex::new(HashSet::new()),
            timers: Mutex::new(HashMap::new()),

            messages: Mutex::new(VecDeque::new()),
        })
    }

    fn extension(&self) -> Arc<Extension> {
        self.extension.upgrade().expect("Extension must be alive")
    }
}

impl Api for TestApi {
    fn send(&self, node: &NodeToken, message: &Vec<u8>) {
        self.messages.lock().push_back((*node, message.clone()));
    }

    fn connect(&self, node: &NodeToken) {
        self.connections.lock().insert(*node);
        self.extension().on_connection_allowed(node);
    }

    fn set_timer(&self, token: TimerToken, ms: u64) {
        let mut timers = self.timers.lock();
        if timers.contains_key(&token) {
            panic!("Tried to set timer with token #{} twice", token);
        }
        timers.insert(token, (ms, false));
    }

    fn set_timer_once(&self, token: TimerToken, ms: u64) {
        let mut timers = self.timers.lock();
        if timers.contains_key(&token) {
            panic!("Tried to set timer with token #{} twice", token);
        }
        timers.insert(token, (ms, true));
    }

    fn clear_timer(&self, token: TimerToken) {
        let mut timers = self.timers.lock();
        if !timers.contains_key(&token) {
            panic!("Tried to clear unregistered timer of token #{}", token);
        }
        timers.remove(&token);
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

    fn push_message(&self, from: NodeToken, message: &[u8]) {
        self.extension().on_message(&from, &message.to_vec());
    }

    fn pop_message(&self) -> Option<(NodeToken, Vec<u8>)> {
        self.messages.lock().pop_front().clone()
    }

    fn close(&self) {
        self.connections.lock().clear();
        self.extension().on_close();
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
    extensions: HashMap<String, (Arc<Extension>, Arc<TestApi>)>,
}

impl TestClient {
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
        }
    }

    pub fn register_extension(&mut self, extension: Arc<Extension>) {
        let name = extension.name();

        if self.extensions.contains_key(&name) {
            panic!("Duplicated application name : {}", name);
        }
        let api = TestApi::new(Arc::downgrade(&extension));
        extension.on_initialize(api.clone());

        self.extensions.insert(name, (extension, api));
    }

    pub fn get_extension(&self, name: &str) -> Arc<Extension> {
        Arc::clone(&self.extensions[name].0)
    }

    fn get_api(&self, name: &str) -> Arc<TestApi> {
        Arc::clone(&self.extensions[name].1)
    }

    pub fn add_node(&self, token: NodeToken) {
        for name in self.extensions.keys() {
            self.get_api(name).add_node(token);
        }
    }

    pub fn remove_node(&self, token: NodeToken) {
        for name in self.extensions.keys() {
            self.get_api(name).remove_node(token);
        }
    }

    pub fn close(&self) {
        for name in self.extensions.keys() {
            self.get_api(name).close();
        }
    }

    pub fn call_timeout(&self, name: &str, token: TimerToken) {
        self.get_api(name).call_timeout(token);
    }

    pub fn push_message(&self, name: &str, from: NodeToken, message: &[u8]) {
        self.get_api(name).push_message(from, message);
    }

    pub fn pop_message(&self, name: &str) -> Option<(NodeToken, Vec<u8>)> {
        self.get_api(name).pop_message()
    }
}
