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

use parking_lot::RwLock;

use super::{Api, Extension, NodeId, Result as ExtensionResult};

struct ClientApi {
    client: Weak<Client>,
    extension: Weak<Extension>,
}

impl Api for ClientApi {
    fn send(&self, id: &NodeId, message: &Vec<u8>) {
        if let Some(extension) = self.extension.upgrade() {
            let _need_encryption = extension.need_encryption();
            info!("send {:?} to {:?}", message, id);
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

impl Client {
    pub fn register_extension(client: Arc<Client>, extension: Arc<Extension>) -> Arc<Api> {
        let name = extension.name();
        let mut extensions = client.extensions.write();
        if let Some(_) = extensions.insert(name, Arc::downgrade(&extension)) {
            let name = extension.name();
            panic!("Duplicated application name : {}", name);
        }

        let api = Arc::new(ClientApi {
            client: Arc::downgrade(&client),
            extension: Arc::downgrade(&extension),
        }) as Arc<Api>;
        extension.on_initialize(Arc::clone(&api));
        api
    }

    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            extensions: RwLock::new(HashMap::new()),
        })
    }

    fn on_message(&self, name: &String, id: &NodeId, data: &Vec<u8>) {
        let extensions = self.extensions.read();
        if let Some(extension) = extensions.get(name) {
            if let Some(extension) = extension.upgrade() {
                extension.on_message(&id, data);
            } else {
                info!("The extension already dropped");
            }
        } else {
            info!("The handler for {} doesn't exist.", name);
        }
    }

    fn on_node_added(&self, id: &NodeId) {
        let extensions = self.extensions.read();
        for (_, extension) in extensions.iter() {
            if let Some(extension) = extension.upgrade() {
                extension.on_node_added(&id);
            } else {
                info!("ClientApi already dropped");
            }
        }
    }

    fn on_node_removed(&self, id: &NodeId) {
        let extensions = self.extensions.read();
        for (_, extension) in extensions.iter() {
            if let Some(extension) = extension.upgrade() {
                extension.on_node_removed(&id);
            } else {
                info!("The extension already dropped");
            }
        }
    }
}
