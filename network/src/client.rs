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

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use cio::IoChannel;
use parking_lot::RwLock;

use super::{Api, Error as ExtensionError, Extension, NodeId};
use super::connection::HandlerMessage as ConnectionMessage;

struct ClientApi {
    extension: Weak<Extension>,
    channel: IoChannel<ConnectionMessage>,
}

impl Api for ClientApi {
    fn send(&self, id: &NodeId, message: &Vec<u8>) {
        if let Some(extension) = self.extension.upgrade() {
            let need_encryption = extension.need_encryption();
            let extension_name = extension.name();
            let node_id = *id;
            if let Err(err) = self.channel.send(ConnectionMessage::SendExtensionMessage { node_id, extension_name, need_encryption, data: message.clone() }) {
                info!("Cannot send extension message to {:?} : {:?}", id, err);
            } else {
                info!("Request send extension message to {:?}", id);
            }
        } else {
            info!("The extension already dropped");
        }
    }

    fn connect(&self, id: &NodeId) {
        if let Some(extension) = self.extension.upgrade() {
            let _need_encryption = extension.need_encryption();
            info!("connect_async to {:?}", id);
        } else {
            info!("The extension already dropped");
        }
    }

    fn set_timer(&self, _timer_id: usize, _ms: u64) {
        unimplemented!();
    }

    fn set_timer_once(&self, _timer_id: usize, _ms: u64) {
        unimplemented!();
    }

    fn clear_timer(&self, _timer_id: usize, _ms: u64) {
        unimplemented!();
    }
}

pub struct Client {
    extensions: RwLock<HashMap<String, Weak<Extension>>>,
}

macro_rules! define_broadcast_method {
    ($method_name: ident) => {
        pub fn $method_name (&self) {
            let extensions = self.extensions.read();
            for (ref name, ref extension) in extensions.iter() {
                if let Some(ref extension) = extension.upgrade() {
                    extension.$method_name();
                } else {
                    info!("Extension {} already dropped before {}", name, stringify!($method_name));
                }
            }
        }
    };
    ($method_name: ident; $($var: ident, $t: ty);*) => {
        pub fn $method_name (&self, $($var: $t), *) {
            let extensions = self.extensions.read();
            for (ref name, ref extension) in extensions.iter() {
                if let Some(ref extension) = extension.upgrade() {
                    extension.$method_name($($var),*);
                } else {
                    info!("Extension {} already dropped before {}", name, stringify!($method_name));
                }
            }
        }
    };
}

macro_rules! define_method {
    ($method_name: ident; $($var: ident, $t: ty);*) => {
        pub fn $method_name (&self, name: &String, $($var: $t), *) {
            let extensions = self.extensions.read();
            if let Some(ref extension) = extensions.get(name) {
                if let Some(ref extension) = extension.upgrade() {
                    extension.$method_name($($var),*);
                } else {
                    info!("Extension {} already dropped before {}", name, stringify!($method_name));
                }
            } else {
                info!("{} doesn't exist.", name);
            }
        }
    };
}

impl Client {
    pub fn register_extension(&self, extension: Arc<Extension>, channel: IoChannel<ConnectionMessage>) -> Arc<Api> {
        let name = extension.name();
        let mut extensions = self.extensions.write();
        if let Some(_) = extensions.insert(name, Arc::downgrade(&extension)) {
            let name = extension.name();
            panic!("Duplicated application name : {}", name);
        }

        let api = Arc::new(ClientApi {
            extension: Arc::downgrade(&extension),
            channel,
        }) as Arc<Api>;
        extension.on_initialize(Arc::clone(&api));
        api
    }

    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            extensions: RwLock::new(HashMap::new()),
        })
    }

    define_broadcast_method!(on_node_added; id, &NodeId);
    define_broadcast_method!(on_node_removed; id, &NodeId);

    define_method!(on_connected; id, &NodeId);
    define_method!(on_connection_allowed; id, &NodeId);
    define_method!(on_connection_denied; id, &NodeId; error, ExtensionError);

    define_method!(on_message; id, &NodeId; data, &Vec<u8>);

    define_broadcast_method!(on_close);

    define_method!(on_timer_set_allowed; timer_id, usize);
    define_method!(on_timer_set_denied; timer_id, usize; error, ExtensionError);
    define_method!(on_timeout; timer_id, usize);
}

impl Drop for Client {
    fn drop(&mut self) {
        self.on_close()
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;
    use std::sync::Arc;
    use std::vec::Vec;

    use cio::IoService;
    use parking_lot::Mutex;

    use super::{Api, Client, Extension, ExtensionError, NodeId};

    struct TestApi {
    }

    impl Api for TestApi {
        fn send(&self, id: &usize, message: &Vec<u8>) {
            unimplemented!()
        }

        fn connect(&self, id: &usize) {
            unimplemented!()
        }

        fn set_timer(&self, timer_id: usize, ms: u64) {
            unimplemented!()
        }

        fn set_timer_once(&self, timer_id: usize, ms: u64) {
            unimplemented!()
        }

        fn clear_timer(&self, timer_id: usize, ms: u64) {
            unimplemented!()
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    enum Callback {
        Initialize,
        NodeAdded,
        NodeRemoved,
        Connected,
        ConnectionAllowed,
        ConnectionDenied,
        Message,
        Close,
        TimerSetAllowed,
        TimerSetDenied,
        Timeout,
    }

    struct TestExtension {
        name: String,
        callbacks: Mutex<Vec<Callback>>,
    }

    impl TestExtension {
        fn new(name: String) -> Self {
            Self {
                name,
                callbacks: Mutex::new(vec![]),
            }
        }
    }

    impl Extension for TestExtension {
        fn name(&self) -> String {
            self.name.clone()
        }

        fn need_encryption(&self) -> bool {
            false
        }

        fn on_initialize(&self, _api: Arc<Api>) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Initialize);
        }

        fn on_node_added(&self, _id: &NodeId) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::NodeAdded);
        }

        fn on_node_removed(&self, _id: &NodeId) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::NodeRemoved);
        }

        fn on_connected(&self, _id: &NodeId) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Connected);
        }

        fn on_connection_allowed(&self, _id: &NodeId) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Connected);
        }

        fn on_connection_denied(&self, _id: &NodeId, _error: ExtensionError) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::ConnectionDenied);
        }

        fn on_message(&self, _id: &NodeId, _message: &Vec<u8>) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Message);
        }

        fn on_close(&self) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Close);
        }

        fn on_timer_set_allowed(&self, _timer_id: usize) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::TimerSetAllowed);
        }

        fn on_timer_set_denied(&self, _timer_id: usize, _error: ExtensionError) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::TimerSetDenied);
        }

        fn on_timeout(&self, _timer_id: usize) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Timeout);
        }
    }

    #[test]
    fn broadcast_node_added() {
        let service = IoService::start().unwrap();

        let client = Client::new();

        let e1 = Arc::new(TestExtension::new("e1".to_string()));
        let _ = client.register_extension(Arc::clone(&e1) as Arc<Extension>, service.channel());
        let e2 = Arc::new(TestExtension::new("e2".to_string()));
        let _ = client.register_extension(Arc::clone(&e2) as Arc<Extension>, service.channel());

        client.on_node_added(&1);

        {
            let callbacks = e1.callbacks.lock();
            assert_eq!(callbacks.deref(), &vec![Callback::Initialize, Callback::NodeAdded]);
        }

        {
            let callbacks = e2.callbacks.lock();
            assert_eq!(callbacks.deref(), &vec![Callback::Initialize, Callback::NodeAdded]);
        }
    }

    #[test]
    fn message_only_to_target() {
        let service = IoService::start().unwrap();

        let client = Client::new();

        let e1 = Arc::new(TestExtension::new("e1".to_string()));
        let _ = client.register_extension(Arc::clone(&e1) as Arc<Extension>, service.channel());
        let e2 = Arc::new(TestExtension::new("e2".to_string()));
        let _ = client.register_extension(Arc::clone(&e2) as Arc<Extension>, service.channel());

        client.on_message(&"e1".to_string(), &1, &vec![]);
        {
            let callbacks = e1.callbacks.lock();
            assert_eq!(callbacks.deref(), &vec![Callback::Initialize, Callback::Message]);
            let callbacks = e2.callbacks.lock();
            assert_eq!(callbacks.deref(), &vec![Callback::Initialize]);
        }

        client.on_message(&"e2".to_string(), &1, &vec![]);
        {
            let callbacks = e1.callbacks.lock();
            assert_eq!(callbacks.deref(), &vec![Callback::Initialize, Callback::Message]);
            let callbacks = e2.callbacks.lock();
            assert_eq!(callbacks.deref(), &vec![Callback::Initialize, Callback::Message]);
        }

        client.on_message(&"e2".to_string(), &5, &vec![]);
        client.on_message(&"e2".to_string(), &1, &vec![]);
        {
            let callbacks = e1.callbacks.lock();
            assert_eq!(callbacks.deref(), &vec![Callback::Initialize, Callback::Message]);
            let callbacks = e2.callbacks.lock();
            assert_eq!(callbacks.deref(), &vec![Callback::Initialize, Callback::Message, Callback::Message, Callback::Message]);
        }
    }
}
