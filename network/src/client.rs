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

use mio::Token;
use parking_lot::RwLock;

use super::Extension;

#[derive(Clone, Eq, PartialEq)]
pub struct NodeId(Token);

pub enum Error {
    DuplicatedApplicationName,
    ExtensionDropped,
}

pub struct ClientApi {
    client: Weak<Client>,
    extension: Weak<Extension>,
}

impl ClientApi {
    fn send_async(&self, id: &NodeId, message: &Vec<u8>) {
        if let Some(extension) = self.extension.upgrade() {
            let _need_encryption = extension.need_encryption();
            info!("send {:?} to {:?}", message, id.0);
        } else {
            info!("The extension already dropped");
        }
    }

    fn send_sync(&self, id: &NodeId, message: &Vec<u8>) -> Result<(), Error> {
        if let Some(extension) = self.extension.upgrade() {
            let _need_encryption = extension.need_encryption();
            info!("send {:?} to {:?}", message, id.0);
            Ok(())
        } else {
            info!("The extension already dropped");
            Err(Error::ExtensionDropped)
        }
    }

    fn connect_sync(&self, id: &NodeId) -> Result<(), Error> {
        if let Some(extension) = self.extension.upgrade() {
            let _need_encryption = extension.need_encryption();
            info!("connect_async to {:?}", id.0);
            Ok(())
        } else {
            info!("The extension already dropped");
            Err(Error::ExtensionDropped)
        }
    }
}

struct Client {
    extensions: RwLock<HashMap<String, Weak<Extension>>>,
}

impl Client {
    pub fn register_extension(client: Arc<Client>, extension: Arc<Extension>) -> Result<(), Error> {
        let name = extension.name();
        let mut extensions = client.extensions.write();
        if extensions.contains_key(&name) {
            return Err(Error::DuplicatedApplicationName)
        }
        let api = Arc::new(ClientApi {
            client: Arc::downgrade(&client),
            extension: Arc::downgrade(&extension),
        });
        if let Some(_) = extensions.insert(name, Arc::downgrade(&extension)) {
            unreachable!();
        }
        extension.on_initialize(api);
        Ok(())
    }

    fn new() -> Self {
        Self {
            extensions: RwLock::new(HashMap::new()),
        }
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
