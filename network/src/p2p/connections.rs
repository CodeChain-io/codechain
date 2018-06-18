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
use std::io;

use cio::{IoManager, StreamToken};
use mio::deprecated::EventLoop;
use mio::Token;
use parking_lot::RwLock;

use super::super::session::Session;
use super::super::{NodeId, SocketAddr};
use super::connection::{Connection, Result};
use super::stream::Stream;

pub use super::connection::{ConnectionType, ReceivedMessage};

pub struct Connections {
    // stream token => established connection
    connections: RwLock<HashMap<StreamToken, Connection>>,

    connected_nodes: RwLock<HashMap<NodeId, StreamToken>>,
    reversed_connected_nodes: RwLock<HashMap<StreamToken, NodeId>>,
}

impl Connections {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),

            connected_nodes: RwLock::new(HashMap::new()),
            reversed_connected_nodes: RwLock::new(HashMap::new()),
        }
    }

    pub fn accept(&self, token: StreamToken, stream: Stream) {
        let mut connections = self.connections.write();
        let t = connections.insert(token, Connection::accept(stream));
        debug_assert!(t.is_none());
    }

    pub fn connect(
        &self,
        token: StreamToken,
        stream: Stream,
        local_node_id: NodeId,
        session: Session,
        socket_address: &SocketAddr,
        local_port: u16,
    ) -> bool {
        let mut connections = self.connections.write();

        let mut connected_nodes = self.connected_nodes.write();
        let mut reversed_connected_nodes = self.reversed_connected_nodes.write();

        let remote_node_id = socket_address.into();
        if connected_nodes.contains_key(&remote_node_id) {
            return false
        }

        let connection = Connection::connect(stream, session, local_port, local_node_id, remote_node_id.clone());
        let t = connections.insert(token, connection);
        debug_assert!(t.is_none());
        let t = connected_nodes.insert(remote_node_id, token);
        debug_assert!(t.is_none());
        let t = reversed_connected_nodes.insert(token, remote_node_id);
        debug_assert!(t.is_none());
        true
    }

    pub fn establish_wait_ack_connection(&self, token: &StreamToken) -> bool {
        let connections = self.connections.read();

        connections
            .get(token)
            .map(|connection| {
                let established = connection.establish();
                debug_assert!(established);
            })
            .is_some()
    }

    pub fn establish_wait_sync_connection(&self, token: &StreamToken) -> bool {
        let connections = self.connections.read();
        let mut connected_nodes = self.connected_nodes.write();
        let mut reversed_connected_nodes = self.reversed_connected_nodes.write();

        connections
            .get(token)
            .and_then(|connection| {
                let remote_node_id =
                    connection.remote_node_id().expect("EstablishedConnection MUST have remote node id");
                if connected_nodes.contains_key(&remote_node_id) {
                    return None
                }
                let t = connection.establish();
                debug_assert!(t);
                let t = connected_nodes.insert(remote_node_id, *token);
                debug_assert!(t.is_none());
                let t = reversed_connected_nodes.insert(*token, remote_node_id);
                debug_assert!(t.is_none());
                Some(())
            })
            .is_some()
    }

    pub fn register<Message>(
        &self,
        token: &StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> io::Result<ConnectionType>
    where
        Message: Send + Sync + Clone + 'static, {
        let connections = self.connections.read();
        if let Some(connection) = connections.get(token) {
            let result = connection.register(reg, event_loop)?;
            debug_assert_ne!(result, ConnectionType::None);
            Ok(result)
        } else {
            Ok(ConnectionType::None)
        }
    }

    pub fn reregister<Message>(
        &self,
        token: &StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> io::Result<ConnectionType>
    where
        Message: Send + Sync + Clone + 'static, {
        let connections = self.connections.read();
        if let Some(connection) = connections.get(token) {
            let result = connection.reregister(reg, event_loop)?;
            debug_assert_ne!(result, ConnectionType::None);
            Ok(result)
        } else {
            Ok(ConnectionType::None)
        }
    }

    pub fn deregister<Message>(
        &self,
        token: &StreamToken,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> io::Result<ConnectionType>
    where
        Message: Send + Sync + Clone + 'static, {
        let connections = self.connections.read();
        if let Some(connection) = connections.get(token) {
            let result = connection.deregister(event_loop)?;
            debug_assert_ne!(result, ConnectionType::None);
            Ok(result)
        } else {
            Ok(ConnectionType::None)
        }
    }

    pub fn remove(&self, token: &StreamToken) {
        let mut connections = self.connections.write();
        let mut connected_nodes = self.connected_nodes.write();
        let mut reversed_connected_nodes = self.reversed_connected_nodes.write();

        let t = connections.remove(token);
        assert!(t.is_some());

        let node_id = reversed_connected_nodes.remove(token).unwrap();
        let t = connected_nodes.remove(&node_id);
        assert_eq!(t, Some(*token));
    }

    // Return true if the queue is not empty
    pub fn send(&self, token: &StreamToken) -> Result<(ConnectionType, bool)> {
        let connections = self.connections.read();
        if let Some(connection) = connections.get(token) {
            let (result, remain) = connection.send()?;
            debug_assert_ne!(result, ConnectionType::None);
            Ok((result, remain))
        } else {
            Ok((ConnectionType::None, false))
        }
    }

    pub fn receive(&self, token: &StreamToken) -> Result<Option<ReceivedMessage>> {
        let connections = self.connections.read();

        if let Some(connection) = connections.get(token) {
            Ok(connection.receive()?)
        } else {
            Ok(None)
        }
    }

    pub fn enqueue_negotiation_request(&self, token: &StreamToken, name: String, versions: Vec<u64>) -> bool {
        let connections = self.connections.read();
        if let Some(connection) = connections.get(token) {
            connection.enqueue_negotiation_request(name, versions)
        } else {
            false
        }
    }

    pub fn enqueue_negotiation_allowed(&self, token: &StreamToken, seq: u64, version: u64) -> bool {
        let connections = self.connections.read();
        if let Some(connection) = connections.get(token) {
            connection.enqueue_negotiation_allowed(seq, version)
        } else {
            false
        }
    }

    pub fn enqueue_extension_message(
        &self,
        token: &StreamToken,
        extension_name: &String,
        need_encryption: bool,
        data: &[u8],
    ) -> bool {
        let connections = self.connections.read();
        if let Some(connection) = connections.get(token) {
            connection.enqueue_extension_message(extension_name, need_encryption, &data)
        } else {
            false
        }
    }

    pub fn remove_requested_negotiation(&self, token: &StreamToken, seq: &u64) -> Option<String> {
        let connections = self.connections.read();
        connections.get(token).and_then(|connection| connection.remove_requested_negotiation(seq))
    }

    pub fn remote_addr_of_waiting_sync(&self, token: &StreamToken) -> Option<SocketAddr> {
        let connections = self.connections.read();
        connections.get(token).and_then(|connection| connection.remote_addr_of_waiting_sync())
    }

    pub fn ready_session(&self, token: &StreamToken, remote_node_id: NodeId, session: Session) -> bool {
        let connections = self.connections.read();
        connections.get(token).map(|connection| connection.ready_session(remote_node_id, session)).is_some()
    }

    pub fn stream_token(&self, node: &NodeId) -> Option<StreamToken> {
        let connected_nodes = self.connected_nodes.read();
        connected_nodes.get(node).cloned()
    }

    pub fn node_id(&self, token: &StreamToken) -> Option<NodeId> {
        let reversed_connected_nodes = self.reversed_connected_nodes.read();
        reversed_connected_nodes.get(token).cloned()
    }

    pub fn established_session(&self, token: &StreamToken) -> Option<Session> {
        let connections = self.connections.read();
        connections.get(token).and_then(|con| con.established_session())
    }

    pub fn len(&self) -> usize {
        let connections = self.connections.read();
        connections.len()
    }
}
