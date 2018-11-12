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
use time::Duration;

use super::super::extension::{Api, Extension, Result, TimerToken};
use super::super::NodeId;

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum Call {
    Send(NodeId, Vec<u8>),
    Negotiate(NodeId),
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

    connections: Mutex<HashSet<NodeId>>,
    timers: Mutex<HashMap<TimerToken, (Duration, bool)>>,

    calls: Mutex<VecDeque<Call>>,
}

impl TestApi {
    fn new(extension: Weak<Extension>) -> Arc<Self> {
        Arc::new(Self {
            extension,

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
    fn send(&self, node: &NodeId, message: &[u8]) {
        self.calls.lock().push_back(Call::Send(*node, message.to_vec()));
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
        if timers.contains_key(&token) {
            timers.remove(&token);
        }
        self.calls.lock().push_back(Call::ClearTimer(token));
        Ok(())
    }
}

impl TestApi {
    fn remove_node(&self, node: NodeId) {
        if !self.connections.lock().remove(&node) {
            panic!("Tried to remove unregistered node #{}", node);
        }
        self.extension().on_node_removed(&node);
    }

    fn add_node(&self, node: NodeId) {
        let mut connections = self.connections.lock();
        if connections.contains(&node) {
            panic!("Duplicated connection detected for node #{}", node);
        }
        connections.insert(node);
        let version = *self.extension().versions().iter().max().unwrap();
        self.extension().on_node_added(&node, version);
    }

    fn send_message(&self, from: NodeId, message: &[u8]) {
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
    nodes: HashSet<NodeId>,
    extensions: HashMap<&'static str, (Arc<Extension>, Arc<TestApi>)>,
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

    pub fn get_extension(&self, name: &str) -> &Extension {
        self.extensions[name].0.deref()
    }

    fn get_api(&self, name: &str) -> &TestApi {
        &self.extensions[name].1
    }

    pub fn remove_node(&self, node: NodeId) {
        if !self.nodes.contains(&node) {
            panic!("Tried to remove non existent node #{}", node);
        }
        for name in self.extensions.keys() {
            self.get_api(name).remove_node(node);
        }
    }

    pub fn add_node(&self, name: &str, node: NodeId) {
        self.get_api(name).add_node(node);
    }

    pub fn send_message(&self, name: &str, from: NodeId, message: &[u8]) {
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
