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
use rlp::Encodable;
use time::Duration;

use super::p2p::Message as P2pMessage;
use super::timer::Message as TimerMessage;
use super::{Api, NetworkExtension, NetworkExtensionError, NetworkExtensionResult, NodeToken, TimerToken};

struct ClientApi {
    extension: Weak<NetworkExtension>,
    p2p_channel: IoChannel<P2pMessage>,
    timer_channel: IoChannel<TimerMessage>,
}

impl Api for ClientApi {
    fn send(&self, id: &NodeToken, message: &Vec<u8>) {
        if let Some(extension) = self.extension.upgrade() {
            let need_encryption = extension.need_encryption();
            let extension_name = extension.name();
            let node_id = *id;
            if let Err(err) = self.p2p_channel.send(P2pMessage::SendExtensionMessage {
                node_id,
                extension_name,
                need_encryption,
                data: message.clone(),
            }) {
                warn!(target: "netapi", "Cannot send extension message to {:?} : {:?}", id, err);
            } else {
                trace!(target: "netapi", "Request send extension message to {:?}", id);
            }
        } else {
            debug!(target: "netapi", "The extension already dropped");
        }
    }

    fn negotiate(&self, id: &NodeToken) {
        if let Some(extension) = self.extension.upgrade() {
            let extension_name = extension.name();
            let version = 0;
            let node_id = *id;
            if let Err(err) = self.p2p_channel.send(P2pMessage::RequestNegotiation {
                node_id,
                extension_name,
                version,
            }) {
                warn!(target: "netapi", "Cannot request negotiation to {:?} : {:?}", id, err);
            } else {
                trace!(target: "netapi", "Request negotiation to {:?}", id);
            }
        } else {
            debug!(target: "netapi", "The extension already dropped");
        }
    }

    fn set_timer(&self, timer_id: usize, duration: Duration) -> NetworkExtensionResult<()> {
        if let Some(extension) = self.extension.upgrade() {
            let extension_name = extension.name();
            Ok(self.timer_channel.send_sync(TimerMessage::SetTimer {
                extension_name,
                timer_id,
                duration,
            })?)
        } else {
            Err(NetworkExtensionError::ExtensionDropped)
        }
    }

    fn set_timer_once(&self, timer_id: usize, duration: Duration) -> NetworkExtensionResult<()> {
        if let Some(extension) = self.extension.upgrade() {
            let extension_name = extension.name();
            Ok(self.timer_channel.send_sync(TimerMessage::SetTimerOnce {
                extension_name,
                timer_id,
                duration,
            })?)
        } else {
            Err(NetworkExtensionError::ExtensionDropped)
        }
    }

    fn clear_timer(&self, timer_id: usize) -> NetworkExtensionResult<()> {
        if let Some(extension) = self.extension.upgrade() {
            let extension_name = extension.name();
            Ok(self.timer_channel.send_sync(TimerMessage::ClearTimer {
                extension_name,
                timer_id,
            })?)
        } else {
            Err(NetworkExtensionError::ExtensionDropped)
        }
    }

    fn send_local_message(&self, message: &Encodable) {
        if let Some(extension) = self.extension.upgrade() {
            let extension_name = extension.name();
            let message = message.rlp_bytes().into_vec();
            if let Err(err) = self.timer_channel.send(TimerMessage::LocalMessage {
                extension_name,
                message,
            }) {
                warn!(target: "netapi", "Cannot send local message: {:?}", err);
            }
        } else {
            debug!(target: "netapi", "The extension already dropped");
        }
    }
}

pub struct Client {
    extensions: RwLock<HashMap<String, Arc<NetworkExtension>>>,
    p2p_channel: IoChannel<P2pMessage>,
    timer_channel: IoChannel<TimerMessage>,
}

macro_rules! define_broadcast_method {
    ($method_name: ident) => {
        pub fn $method_name (&self) {
            let extensions = self.extensions.read();
            for (_, ref extension) in extensions.iter() {
                extension.$method_name();
            }
        }
    };
    ($method_name: ident; $($var: ident, $t: ty);*) => {
        pub fn $method_name (&self, $($var: $t), *) {
            let extensions = self.extensions.read();
            for (_, ref extension) in extensions.iter() {
                extension.$method_name($($var),*);
            }
        }
    };
}

macro_rules! define_method {
    ($method_name: ident; $($var: ident, $t: ty);*) => {
        pub fn $method_name (&self, name: &String, $($var: $t), *) {
            let extensions = self.extensions.read();
            if let Some(ref extension) = extensions.get(name) {
                extension.$method_name($($var),*);
            } else {
                debug!(target: "netapi", "{} doesn't exist.", name);
            }
        }
    };
}

impl Client {
    pub fn register_extension(&self, extension: Arc<NetworkExtension>) {
        let name = extension.name();
        let mut extensions = self.extensions.write();
        if let Some(_) = extensions.insert(name, Arc::clone(&extension)) {
            let name = extension.name();
            panic!("Duplicated extension name : {}", name);
        }
    }

    pub fn initialize_extension(&self, extension_name: &String) {
        let extension = {
            let mut extensions = self.extensions.read();
            extensions.get(extension_name).map(Arc::clone)
        };
        if let Some(extension) = extension {
            let p2p_channel = self.p2p_channel.clone();
            let timer_channel = self.timer_channel.clone();
            let api: Arc<Api> = Arc::new(ClientApi {
                extension: Arc::downgrade(&extension),
                p2p_channel,
                timer_channel,
            });
            extension.on_initialize(api);
        }
    }

    pub fn new(p2p_channel: IoChannel<P2pMessage>, timer_channel: IoChannel<TimerMessage>) -> Arc<Self> {
        Arc::new(Self {
            extensions: RwLock::new(HashMap::new()),
            p2p_channel,
            timer_channel,
        })
    }

    define_broadcast_method!(on_node_added; id, &NodeToken);
    define_broadcast_method!(on_node_removed; id, &NodeToken);

    define_method!(on_negotiated; id, &NodeToken);
    define_method!(on_negotiation_allowed; id, &NodeToken);
    define_method!(on_negotiation_denied; id, &NodeToken; error, NetworkExtensionError);

    define_method!(on_message; id, &NodeToken; data, &[u8]);

    define_method!(on_timeout; timer_id, TimerToken);

    define_method!(on_local_message; message, &[u8]);
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;
    use std::sync::Arc;
    use std::vec::Vec;

    use cio::IoService;
    use parking_lot::Mutex;
    use rlp::Encodable;
    use time::Duration;

    use super::{Api, Client, NetworkExtension, NetworkExtensionError, NetworkExtensionResult, NodeToken};

    #[allow(dead_code)]
    struct TestApi;

    impl Api for TestApi {
        fn send(&self, _id: &usize, _message: &Vec<u8>) {
            unimplemented!()
        }

        fn negotiate(&self, _id: &usize) {
            unimplemented!()
        }

        fn set_timer(&self, _timer_id: usize, _duration: Duration) -> NetworkExtensionResult<()> {
            unimplemented!()
        }

        fn set_timer_once(&self, _timer_id: usize, _duration: Duration) -> NetworkExtensionResult<()> {
            unimplemented!()
        }

        fn clear_timer(&self, _timer_id: usize) -> NetworkExtensionResult<()> {
            unimplemented!()
        }

        fn send_local_message(&self, _message: &Encodable) {
            unimplemented!()
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    enum Callback {
        Initialize,
        NodeAdded,
        NodeRemoved,
        Negotiated,
        NegotiationAllowed,
        NegotiationDenied,
        Message,
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

    impl NetworkExtension for TestExtension {
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

        fn on_node_added(&self, _id: &NodeToken) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::NodeAdded);
        }

        fn on_node_removed(&self, _id: &NodeToken) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::NodeRemoved);
        }

        fn on_negotiated(&self, _id: &NodeToken) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Negotiated);
        }

        fn on_negotiation_allowed(&self, _id: &NodeToken) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::NegotiationAllowed);
        }

        fn on_negotiation_denied(&self, _id: &NodeToken, _error: NetworkExtensionError) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::NegotiationDenied);
        }

        fn on_message(&self, _id: &NodeToken, _message: &[u8]) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Message);
        }

        fn on_timeout(&self, _timer_id: usize) {
            let mut callbacks = self.callbacks.lock();
            callbacks.push(Callback::Timeout);
        }
    }

    #[test]
    fn broadcast_node_added() {
        let p2p_service = IoService::start().unwrap();
        let timer_service = IoService::start().unwrap();

        let client = Client::new(p2p_service.channel(), timer_service.channel());

        let e1 = Arc::new(TestExtension::new("e1".to_string()));
        client.register_extension(Arc::clone(&e1) as Arc<NetworkExtension>);
        client.initialize_extension(&"e1".to_string());
        let e2 = Arc::new(TestExtension::new("e2".to_string()));
        client.register_extension(Arc::clone(&e2) as Arc<NetworkExtension>);
        client.initialize_extension(&"e2".to_string());

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
        let p2p_service = IoService::start().unwrap();
        let timer_service = IoService::start().unwrap();

        let client = Client::new(p2p_service.channel(), timer_service.channel());

        let e1 = Arc::new(TestExtension::new("e1".to_string()));
        client.register_extension(Arc::clone(&e1) as Arc<NetworkExtension>);
        client.initialize_extension(&"e1".to_string());
        let e2 = Arc::new(TestExtension::new("e2".to_string()));
        client.register_extension(Arc::clone(&e2) as Arc<NetworkExtension>);
        client.initialize_extension(&"e2".to_string());

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
            assert_eq!(
                callbacks.deref(),
                &vec![Callback::Initialize, Callback::Message, Callback::Message, Callback::Message]
            );
        }
    }
}
