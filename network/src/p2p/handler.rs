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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ccrypto::aes::SymmetricCipherError;
use cfinally::finally;
use cio::{IoContext, IoHandler, IoHandlerResult, IoManager, StreamToken, TimerToken};
use ctoken_generator::TokenGenerator;
use ctypes::util::unexpected::Mismatch;
use mio::deprecated::EventLoop;
use mio::{PollOpt, Ready, Token};
use parking_lot::Mutex;
use rlp::UntrustedRlp;

use super::super::addr::convert_to_node_id;
use super::super::client::Client;
use super::super::{FiltersControl, IntoSocketAddr, NodeId, RoutingTable, SocketAddr};
use super::connections::{ConnectionType, Connections, ReceivedMessage};
use super::listener::Listener;
use super::message::{HandshakeMessage, Message as NetworkMessage, Version};
use super::stream::Stream;
use super::NegotiationBody;

pub const MAX_CONNECTIONS: usize = 200;

const ACCEPT_TOKEN: TimerToken = 0;

const FIRST_CONNECTION_TOKEN: TimerToken = ACCEPT_TOKEN + 1;
const LAST_CONNECTION_TOKEN: TimerToken = FIRST_CONNECTION_TOKEN + MAX_CONNECTIONS;

const CREATE_CONNECTIONS_TOKEN: TimerToken = 0;
const PULL_CONNECTIONS_MS: u64 = 10 * 1000;

#[derive(Clone, Debug, PartialEq)]
pub enum IgnoreConnectionLimit {
    Ignore,
    Not,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    RequestConnection(SocketAddr, IgnoreConnectionLimit),

    RequestNegotiation {
        node_id: NodeId,
    },
    SendExtensionMessage {
        node_id: NodeId,
        extension_name: String,
        need_encryption: bool,
        data: Vec<u8>,
    },
    Disconnect(SocketAddr),
    ApplyFilters,
}

#[derive(Debug)]
enum Error {
    InvalidStream(StreamToken),
    InvalidNode(NodeId),
    InvalidSign,
    UnexpectedNodeId(Mismatch<NodeId>),
    SymmetricCipherError(SymmetricCipherError),
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            Error::InvalidStream(_) => ::std::fmt::Debug::fmt(self, f),
            Error::InvalidNode(_) => ::std::fmt::Debug::fmt(self, f),
            Error::InvalidSign => ::std::fmt::Debug::fmt(&self, f),
            Error::UnexpectedNodeId(_) => ::std::fmt::Debug::fmt(&self, f),
            Error::SymmetricCipherError(err) => ::std::fmt::Debug::fmt(&err, f),
        }
    }
}

pub struct Handler {
    socket_address: SocketAddr,

    listener: Listener,

    establish_lock: Mutex<()>,
    tokens: Mutex<TokenGenerator>,

    routing_table: Arc<RoutingTable>,
    filters: Arc<FiltersControl>,
    connections: Connections,

    client: Arc<Client>,

    min_peers: usize,
    max_peers: usize,
}

impl Handler {
    pub fn try_new(
        socket_address: SocketAddr,
        client: Arc<Client>,
        routing_table: Arc<RoutingTable>,
        filters: Arc<FiltersControl>,
        min_peers: usize,
        max_peers: usize,
    ) -> ::std::result::Result<Self, String> {
        if MAX_CONNECTIONS < max_peers {
            return Err(format!("Max peers must be less than {}", MAX_CONNECTIONS))
        }
        Ok(Self {
            socket_address,
            listener: Listener::bind(&socket_address).expect("Cannot listen TCP port"),

            establish_lock: Mutex::new(()),
            tokens: Mutex::new(TokenGenerator::new(FIRST_CONNECTION_TOKEN, LAST_CONNECTION_TOKEN)),

            routing_table,
            filters,
            connections: Connections::new(),

            client,

            min_peers,
            max_peers,
        })
    }

    pub fn get_port(&self) -> u16 {
        self.socket_address.port()
    }

    pub fn get_peer_count(&self) -> usize {
        self.connections.established_count()
    }

    pub fn established_peers(&self) -> Vec<SocketAddr> {
        self.connections.established_peers()
    }

    fn accept(&self) -> IoHandlerResult<Option<(StreamToken, SocketAddr)>> {
        match self.listener.accept()? {
            Some((stream, socket_address)) => {
                let ip = socket_address.ip();
                if !self.filters.is_allowed(&ip) {
                    return Err(format!("P2P connection request from {} is received. But it's not allowed", ip).into())
                }
                let token = self.tokens.lock().gen().ok_or("TooManyConnections")?;
                self.connections.accept(token, stream);
                Ok(Some((token, socket_address)))
            }
            None => Ok(None),
        }
    }

    fn connect(&self, io: &IoContext<Message>, socket_address: &SocketAddr) -> IoHandlerResult<Option<StreamToken>> {
        let ip = socket_address.ip();
        if !self.filters.is_allowed(&ip) {
            return Err(format!("P2P connection to {} is requested. But it's not allowed", ip).into())
        }

        Ok(match Stream::connect(socket_address)? {
            Some(stream) => {
                let remote_node_id = socket_address.into();

                let _establish_lock = self.establish_lock.lock();
                let local_node_id = self.routing_table.local_node_id(&remote_node_id).ok_or("Not handshaked")?;
                let session = self.routing_table.unestablished_session(&socket_address).ok_or("Session doesn't exist")?;

                let mut tokens = self.tokens.lock();
                let token = tokens.gen().ok_or("TooManyConnections")?;
                if !self.connections.connect(token, stream, local_node_id, session, socket_address, self.get_port()) {
                    tokens.restore(token);
                    return Err(format!("Cannot create connection to {}", socket_address).into())
                }
                const CONNECTION_TIMEOUT_MS: u64 = 3_000;
                io.register_timer_once(token as TimerToken, CONNECTION_TIMEOUT_MS)?;
                self.routing_table.set_establishing(socket_address);
                Some(token)
            }
            None => None,
        })
    }

    // Return false if there is no message
    fn receive(&self, stream: &StreamToken, client: &Client, io: &IoContext<Message>) -> IoHandlerResult<bool> {
        Ok(match self.connections.receive(stream)? {
            None => false,
            Some(ReceivedMessage::Ack {
                ..
            }) => {
                let _establish_lock = self.establish_lock.lock();
                if !self.connections.establish_wait_ack_connection(stream) {
                    return Err(Error::InvalidStream(*stream).into())
                }

                let node_id = self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                self.routing_table.establish(&node_id.into_addr());
                io.clear_timer(*stream as TimerToken)?;
                io.message(Message::RequestNegotiation {
                    node_id,
                })?;
                true
            }
            Some(ReceivedMessage::Sync(signed_message)) => {
                let rlp = UntrustedRlp::new(&signed_message.message);
                let message = rlp.as_val::<NetworkMessage>()?;

                match message {
                    NetworkMessage::Handshake(HandshakeMessage::Sync {
                        port,
                        node_id,
                        ..
                    }) => {
                        let remote_addr =
                            self.connections.remote_addr_of_waiting_sync(stream).ok_or("Cannot find remote address")?;
                        let remote_node_id = convert_to_node_id(remote_addr.ip(), port);

                        if remote_node_id != node_id {
                            return Err(Error::UnexpectedNodeId(Mismatch {
                                expected: remote_node_id,
                                found: node_id,
                            }).into())
                        }

                        let remote_addr = SocketAddr::new(remote_addr.ip(), port);

                        let _establish_lock = self.establish_lock.lock();
                        let session =
                            self.routing_table.unestablished_session(&remote_addr).ok_or("Cannot find session")?;
                        if !signed_message.is_valid(&session) {
                            return Err(Error::InvalidSign.into())
                        }

                        self.routing_table.establish(&remote_addr);
                        self.connections.ready_session(stream, remote_node_id, session);
                        true
                    }
                    _ => unreachable!(),
                }
            }
            Some(ReceivedMessage::Extension(msg)) => {
                let session = self.connections.established_session(stream).ok_or("Invalid stream")?;
                // FIXME: check version of extension
                let message = msg.unencrypted_data(&session).map_err(Error::from)?;
                let node_id = self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                client.on_message(msg.extension_name(), &node_id, &message);
                true
            }
            Some(ReceivedMessage::Negotiation(msg)) => {
                match msg.body() {
                    NegotiationBody::Request {
                        ref extension_name,
                        ..
                    } => {
                        let seq = msg.seq();
                        // FIXME: version negotiation
                        const VERSION: Version = 0;
                        if self.connections.enqueue_negotiation_allowed(stream, seq, VERSION) {
                            let node_id = self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                            client.on_node_added(&extension_name, &node_id, VERSION);
                        } else {
                            return Err(format!("Cannot enqueue negotiation message for {}", stream).into())
                        }
                    }
                    NegotiationBody::Allowed(extension_version) => {
                        let seq = msg.seq();
                        if let Some(name) = self.connections.remove_requested_negotiation(stream, &seq) {
                            let node_id = self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                            client.on_node_added(&name, &node_id, *extension_version);
                        } else {
                            return Err("Negotiation::Allowed message received from non requested seq".into())
                        }
                    }
                    NegotiationBody::Denied => {
                        let seq = msg.seq();
                        if let Some(_) = self.connections.remove_requested_negotiation(stream, &seq) {
                            self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                        } else {
                            return Err("Negotiation::Denied message received from non requested seq".into())
                        }
                    }
                };
                true
            }
        })
    }

    fn send(&self, stream: &StreamToken) -> IoHandlerResult<()> {
        let (connection_type, remain) = self.connections.send(stream)?;
        match connection_type {
            ConnectionType::None => Err(Error::InvalidStream(*stream).into()),
            ConnectionType::AckWaiting => {
                debug_assert!(!remain);
                Ok(())
            }
            ConnectionType::SyncWaiting => {
                if remain {
                    return Err("Cannot send ack message".into())
                }
                // Ack message was sent
                self.connections.establish_wait_sync_connection(stream);
                self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                Ok(())
            }
            ConnectionType::Established => Ok(()),
            ConnectionType::Disconnecting => Err(Error::InvalidStream(*stream).into()),
        }
    }
}


impl IoHandler<Message> for Handler {
    fn initialize(&self, io: &IoContext<Message>) -> IoHandlerResult<()> {
        io.register_stream(ACCEPT_TOKEN)?;
        io.register_timer_once(CREATE_CONNECTIONS_TOKEN, PULL_CONNECTIONS_MS)
            .expect("Pull connections must be registered");
        Ok(())
    }

    fn timeout(&self, io: &IoContext<Message>, token: TimerToken) -> IoHandlerResult<()> {
        match token {
            CREATE_CONNECTIONS_TOKEN => {
                let register_new_timer = AtomicBool::new(false);
                let _f = finally(|| {
                    if register_new_timer.load(Ordering::SeqCst) {
                        io.register_timer_once(CREATE_CONNECTIONS_TOKEN, PULL_CONNECTIONS_MS)
                            .expect("Pull connections must be registered");
                    }
                });
                let number_of_connections = self.connections.len();
                if number_of_connections < self.min_peers {
                    register_new_timer.store(true, Ordering::SeqCst);
                    let count = (self.min_peers - number_of_connections + 1) / 2;
                    let addresses = self.routing_table.unestablished_addresses(count);
                    for address in addresses {
                        io.message(Message::RequestConnection(address, IgnoreConnectionLimit::Not))?;
                    }
                }
                Ok(())
            }
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let node_id = self.connections.node_id(&token).ok_or(Error::InvalidStream(token))?;
                let address = node_id.into_addr();

                if !self.routing_table.reset_session(&address) {
                    return Err("Failed to find session".into())
                }
                self.connections.shutdown(&address)?;
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn message(&self, io: &IoContext<Message>, message: &Message) -> IoHandlerResult<()> {
        match message {
            Message::RequestConnection(socket_address, ignore_connection_limit) => {
                if self.routing_table.is_connected(socket_address) {
                    return Ok(())
                }

                if ignore_connection_limit == &IgnoreConnectionLimit::Not {
                    let number_of_connections = self.connections.len();
                    if self.max_peers <= number_of_connections {
                        return Err(format!("Already has maximum peers({})", number_of_connections).into())
                    }
                }

                ctrace!(NETWORK, "Connecting to {}", socket_address);
                let token = self.connect(io, &socket_address)?.ok_or("Cannot create connection")?;
                cinfo!(NETWORK, "New connection to {}({})", socket_address, token);
                io.register_stream(token)?;
                Ok(())
            }
            Message::RequestNegotiation {
                node_id,
            } => {
                let versions = self.client.extension_versions();
                for (extension_name, versions) in versions.into_iter() {
                    let token = self.connections.stream_token(&node_id).ok_or(Error::InvalidNode(*node_id))?;
                    if !self.connections.enqueue_negotiation_request(&token, extension_name, versions) {
                        return Err(Error::InvalidStream(token).into())
                    }
                    io.update_registration(token)?;
                }
                Ok(())
            }
            Message::SendExtensionMessage {
                node_id,
                extension_name,
                need_encryption,
                data,
            } => {
                let token = self.connections.stream_token(node_id).ok_or(Error::InvalidNode(*node_id))?;
                if !self.connections.enqueue_extension_message(&token, extension_name, *need_encryption, data) {
                    return Err(Error::InvalidStream(token).into())
                }
                io.update_registration(token)?;
                Ok(())
            }
            Message::Disconnect(socket_address) => {
                self.connections.shutdown(&socket_address)?;
                self.routing_table.ban(&socket_address);
                Ok(())
            }
            Message::ApplyFilters => {
                let addresses = self.connections.get_filtered_address(&*self.filters);
                cinfo!(NETWORK, "Connections to the following addresses will be closed: {:?}", addresses);
                for address in addresses.iter() {
                    let _ = self.connections.shutdown(address).map_err(|err| {
                        cwarn!(NETWORK, "Cannot close the connection to {}: {:?}", address, err);
                    });
                }
                Ok(())
            }
        }
    }

    fn stream_hup(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => unreachable!(),
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                ctrace!(NETWORK, "Hup event for {}", stream);
                if !self.connections.is_connected(&stream) {
                    return Err(format!("stream's hup event called twice from {:?}", stream).into())
                }
                let register_new_timer = AtomicBool::new(false);
                let _f = finally(|| {
                    if register_new_timer.load(Ordering::SeqCst) {
                        io.register_timer_once(CREATE_CONNECTIONS_TOKEN, PULL_CONNECTIONS_MS)
                            .expect("Pull connections must be registered");
                    }
                });
                if self.connections.len() < self.min_peers {
                    register_new_timer.store(true, Ordering::SeqCst);
                }
                let was_established = self.connections.is_established(&stream);
                self.connections.set_disconnecting(&stream);
                let node_id = self.connections.node_id(&stream).ok_or(Error::InvalidStream(stream))?;
                self.routing_table.remove_node_on_shutdown(node_id.into_addr());
                if was_established {
                    self.client.on_node_removed(&node_id);
                }
                io.deregister_stream(stream)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn stream_readable(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                if let Some((token, address)) = self.accept()? {
                    cinfo!(NETWORK, "New connection from {}({})", address, token);
                    io.register_stream(token)?;
                }
            }
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let _f = finally(|| {
                    if let Err(err) = io.update_registration(stream) {
                        cwarn!(NETWORK, "Cannot update registration in stream_readable for {} {:?}", stream, err);
                    }
                });
                loop {
                    if !self.receive(&stream, &self.client, io)? {
                        break
                    }
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn stream_writable(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => unreachable!(),
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let _f = finally(|| {
                    if let Err(err) = io.update_registration(stream) {
                        cwarn!(NETWORK, "Cannot update registration in stream_writable for {} {:?}", stream, err);
                    }
                });
                self.send(&stream)
            }
            _ => unreachable!(),
        }
    }

    fn register_stream(
        &self,
        stream: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                event_loop.register(&self.listener, reg, Ready::readable(), PollOpt::edge())?;
                ctrace!(NETWORK, "TCP connection starts for {}", self.socket_address);
                Ok(())
            }
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                self.connections.register(&stream, reg, event_loop)?;
                Ok(())
            }
            _ => {
                unreachable!();
            }
        }
    }

    fn update_stream(
        &self,
        stream: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                unreachable!();
            }
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                self.connections.reregister(&stream, reg, event_loop)?;
                Ok(())
            }
            _ => {
                unreachable!();
            }
        }
    }

    fn deregister_stream(
        &self,
        stream: StreamToken,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => unreachable!(),
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                self.connections.remove(&stream);
                self.connections.deregister(&stream, event_loop)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}

impl From<SymmetricCipherError> for Error {
    fn from(err: SymmetricCipherError) -> Self {
        Error::SymmetricCipherError(err)
    }
}
