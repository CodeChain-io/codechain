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
use super::connection::{Connection, EstablishedConnection, Result as ConnectionResult};
use super::message::{ExtensionMessage, HandshakeMessage, Message, NegotiationMessage, SignedMessage};
use super::pending_connection::{WaitAckConnection, WaitSyncConnection};
use super::stream::Stream;

pub struct Connections {
    // stream token => Accepted connection
    waiting_sync_connections: RwLock<HashMap<StreamToken, WaitSyncConnection>>,

    // stream token => connection which is requested by local
    // These connection already know session but it must wait ack to establish connection.
    waiting_ack_connections: RwLock<HashMap<StreamToken, WaitAckConnection>>,

    // stream token => established connection
    connections: RwLock<HashMap<StreamToken, EstablishedConnection>>,

    connected_nodes: RwLock<HashMap<NodeId, StreamToken>>,
    reversed_connected_nodes: RwLock<HashMap<StreamToken, NodeId>>,
}

impl Connections {
    pub fn new() -> Self {
        Self {
            waiting_ack_connections: RwLock::new(HashMap::new()),
            waiting_sync_connections: RwLock::new(HashMap::new()),
            connections: RwLock::new(HashMap::new()),

            connected_nodes: RwLock::new(HashMap::new()),
            reversed_connected_nodes: RwLock::new(HashMap::new()),
        }
    }

    pub fn accept(&self, token: StreamToken, stream: Stream) {
        let mut waiting_sync_connections = self.waiting_sync_connections.write();

        let t = waiting_sync_connections.insert(token, WaitSyncConnection::new(stream));
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
        let mut waiting_ack_connections = self.waiting_ack_connections.write();
        let mut connected_nodes = self.connected_nodes.write();
        let mut reversed_connected_nodes = self.reversed_connected_nodes.write();

        let remote_node_id = socket_address.into();
        if connected_nodes.contains_key(&remote_node_id) {
            return false
        }

        let connection = WaitAckConnection::new(stream, session, local_port, local_node_id, remote_node_id.clone());
        let t = waiting_ack_connections.insert(token, connection);
        debug_assert!(t.is_none());
        let t = connected_nodes.insert(remote_node_id, token);
        debug_assert!(t.is_none());
        let t = reversed_connected_nodes.insert(token, remote_node_id);
        debug_assert!(t.is_none());
        true
    }

    pub fn establish_wait_ack_connection(&self, token: &StreamToken) -> bool {
        let mut waiting_ack_connections = self.waiting_ack_connections.write();
        let mut connections = self.connections.write();

        waiting_ack_connections
            .remove(token)
            .map(|a| a.establish())
            .map(|connection| {
                let t = connections.insert(*token, connection);
                debug_assert!(t.is_none());
            })
            .is_some()
    }

    pub fn establish_wait_sync_connection(&self, token: &StreamToken) -> bool {
        let mut waiting_sync_connections = self.waiting_sync_connections.write();
        let mut connections = self.connections.write();
        let mut connected_nodes = self.connected_nodes.write();
        let mut reversed_connected_nodes = self.reversed_connected_nodes.write();

        waiting_sync_connections
            .remove(token)
            .map(WaitSyncConnection::establish)
            .and_then(|connection| {
                let remote_node_id =
                    connection.remote_node_id().expect("EstablishedConnection MUST have remote node id");
                if connected_nodes.contains_key(&remote_node_id) {
                    return None
                }
                let t = connections.insert(*token, connection);
                debug_assert!(t.is_none());
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
        let waiting_ack_connections = self.waiting_ack_connections.read();
        let waiting_sync_connections = self.waiting_sync_connections.read();
        let connections = self.connections.read();

        if let Some(connection) = connections.get(token) {
            connection.register(reg, event_loop)?;
            return Ok(ConnectionType::Established)
        }
        if let Some(connection) = waiting_ack_connections.get(token) {
            connection.register(reg, event_loop)?;
            return Ok(ConnectionType::AckWaiting)
        }

        if let Some(connection) = waiting_sync_connections.get(token) {
            connection.register(reg, event_loop)?;
            return Ok(ConnectionType::SyncWaiting)
        }

        Ok(ConnectionType::None)
    }

    pub fn reregister<Message>(
        &self,
        token: &StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> io::Result<ConnectionType>
    where
        Message: Send + Sync + Clone + 'static, {
        let waiting_ack_connections = self.waiting_ack_connections.read();
        let waiting_sync_connections = self.waiting_sync_connections.read();
        let connections = self.connections.read();

        if let Some(connection) = connections.get(token) {
            connection.reregister(reg, event_loop)?;
            return Ok(ConnectionType::Established)
        }
        if let Some(connection) = waiting_ack_connections.get(token) {
            connection.reregister(reg, event_loop)?;
            return Ok(ConnectionType::AckWaiting)
        }

        if let Some(connection) = waiting_sync_connections.get(token) {
            connection.reregister(reg, event_loop)?;
            return Ok(ConnectionType::SyncWaiting)
        }

        Ok(ConnectionType::None)
    }

    pub fn deregister<Message>(
        &self,
        token: &StreamToken,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> io::Result<ConnectionType>
    where
        Message: Send + Sync + Clone + 'static, {
        let mut waiting_ack_connections = self.waiting_ack_connections.write();
        let mut waiting_sync_connections = self.waiting_sync_connections.write();
        let mut connections = self.connections.write();

        if let Some(connection) = connections.remove(token) {
            connection.deregister(event_loop)?;
            return Ok(ConnectionType::Established)
        }

        if let Some(connection) = waiting_ack_connections.remove(token) {
            connection.deregister(event_loop)?;
            return Ok(ConnectionType::AckWaiting)
        }

        if let Some(connection) = waiting_sync_connections.remove(token) {
            connection.deregister(event_loop)?;
            return Ok(ConnectionType::SyncWaiting)
        }

        Ok(ConnectionType::None)
    }

    // Return true if the queue is not empty
    pub fn send(&self, token: &StreamToken) -> ConnectionResult<(ConnectionType, bool)> {
        let mut waiting_ack_connections = self.waiting_ack_connections.write();
        let mut waiting_sync_connections = self.waiting_sync_connections.write();
        let mut connections = self.connections.write();

        if let Some(ref mut connection) = connections.get_mut(token) {
            let result = connection.send()?;
            return Ok((ConnectionType::Established, result))
        }

        if let Some(ref mut connection) = waiting_ack_connections.get_mut(token) {
            connection.send()?;
            return Ok((ConnectionType::AckWaiting, false))
        }

        if let Some(ref mut connection) = waiting_sync_connections.get_mut(token) {
            connection.send()?;
            return Ok((ConnectionType::SyncWaiting, false))
        }

        Ok((ConnectionType::None, false))
    }

    pub fn receive(&self, token: &StreamToken) -> ConnectionResult<Option<ReceivedMessage>> {
        let mut waiting_ack_connections = self.waiting_ack_connections.write();
        let mut waiting_sync_connections = self.waiting_sync_connections.write();
        let mut connections = self.connections.write();

        if let Some(ref mut connection) = connections.get_mut(token) {
            return Ok(connection.receive()?.map(|message| match message {
                Message::Negotiation(msg) => ReceivedMessage::Negotiation(msg),
                Message::Extension(msg) => ReceivedMessage::Extension(msg),
                _ => unreachable!(),
            }))
        }

        if let Some(ref mut connection) = waiting_ack_connections.get_mut(token) {
            return Ok(connection.receive()?.map(|message| match message {
                HandshakeMessage::Ack(version) => ReceivedMessage::Ack {
                    version,
                },
                _ => unreachable!(),
            }))
        }

        if let Some(ref mut connection) = waiting_sync_connections.get_mut(token) {
            return Ok(connection.receive()?.map(ReceivedMessage::Sync))
        }

        Ok(None)
    }

    pub fn enqueue_negotiation_request(&self, token: &StreamToken, name: String, version: u64) -> bool {
        let mut connections = self.connections.write();
        if let Some(ref mut connection) = connections.get_mut(token) {
            connection.enqueue_negotiation_request(name, version);
            true
        } else {
            false
        }
    }

    pub fn enqueue_negotiation_allowed(&self, token: &StreamToken, seq: u64) -> bool {
        let mut connections = self.connections.write();
        if let Some(ref mut connection) = connections.get_mut(token) {
            connection.enqueue_negotiation_allowed(seq);
            true
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
        let mut connections = self.connections.write();
        if let Some(ref mut connection) = connections.get_mut(token) {
            connection.enqueue_extension_message(extension_name.clone(), need_encryption, &data);
            true
        } else {
            false
        }
    }

    pub fn remove_requested_negotiation(&self, token: &StreamToken, seq: &u64) -> Option<String> {
        let mut connections = self.connections.write();
        connections.get_mut(token).and_then(|connection| connection.remove_requested_negotiation(seq))
    }

    pub fn remote_addr_of_waiting_sync(&self, token: &StreamToken) -> Option<SocketAddr> {
        let waiting_sync_connections = self.waiting_sync_connections.read();
        waiting_sync_connections.get(token).and_then(|con| con.remote_addr().ok())
    }

    pub fn ready_session(&self, token: &StreamToken, remote_node_id: NodeId, session: Session) -> bool {
        let mut waiting_sync_connections = self.waiting_sync_connections.write();
        waiting_sync_connections
            .get_mut(token)
            .map(|connection| {
                connection.ready_session(remote_node_id, session);
            })
            .is_some()
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
        connections.get(token).and_then(|con| con.session())
    }

    pub fn len(&self) -> usize {
        let waiting_ack_connections = self.waiting_ack_connections.read();
        let waiting_sync_connections = self.waiting_sync_connections.read();
        let connections = self.connections.read();

        waiting_ack_connections.len() + waiting_sync_connections.len() + connections.len()
    }
}


pub enum ReceivedMessage {
    Ack {
        version: u64,
    },
    Sync(SignedMessage),
    Extension(ExtensionMessage),
    Negotiation(NegotiationMessage),
}

#[derive(Debug, PartialEq)]
pub enum ConnectionType {
    None,
    AckWaiting,
    SyncWaiting,
    Established,
}
