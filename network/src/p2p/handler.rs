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

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ccrypto::aes::SymmetricCipherError;
use cfinally::finally;
use cio::{IoContext, IoHandler, IoHandlerResult, IoManager, StreamToken, TimerToken};
use mio::deprecated::EventLoop;
use mio::{PollOpt, Ready, Token};
use parking_lot::Mutex;
use rlp::UntrustedRlp;
use unexpected::Mismatch;

use super::super::addr::convert_to_node_id;
use super::super::client::Client;
use super::super::token_generator::TokenGenerator;
use super::super::RoutingTable;
use super::super::{IntoSocketAddr, NodeId, SocketAddr};
use super::connections::{ConnectionType, Connections, ReceivedMessage};
use super::listener::Listener;
use super::message::{HandshakeMessage, Message as NetworkMessage, Version};
use super::stream::Stream;
use super::NegotiationBody;

struct Manager {
    listener: Listener,

    tokens: TokenGenerator,

    routing_table: Arc<RoutingTable>,
    connections: Connections,

    port: u16,
}

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
}

#[derive(Debug)]
enum Error {
    InvalidStream(StreamToken),
    InvalidNode(NodeId),
    InvalidSign,
    UnexpectedNodeId(Mismatch<NodeId>),
    SymmetricCipherError(SymmetricCipherError),
    General(&'static str),
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            Error::InvalidStream(_) => ::std::fmt::Debug::fmt(self, f),
            Error::InvalidNode(_) => ::std::fmt::Debug::fmt(self, f),
            Error::InvalidSign => ::std::fmt::Debug::fmt(&self, f),
            Error::UnexpectedNodeId(_) => ::std::fmt::Debug::fmt(&self, f),
            Error::SymmetricCipherError(err) => ::std::fmt::Debug::fmt(&err, f),
            Error::General(_) => ::std::fmt::Debug::fmt(self, f),
        }
    }
}

impl Manager {
    pub fn listen(socket_address: &SocketAddr, routing_table: Arc<RoutingTable>) -> io::Result<Self> {
        Ok(Manager {
            listener: Listener::bind(&socket_address)?,

            tokens: TokenGenerator::new(FIRST_CONNECTION_TOKEN, LAST_CONNECTION_TOKEN),

            routing_table,
            connections: Connections::new(),

            port: socket_address.port(),
        })
    }

    pub fn accept(&mut self) -> IoHandlerResult<Option<(StreamToken)>> {
        match self.listener.accept()? {
            Some((stream, _socket_address)) => {
                let token = self.tokens.gen().ok_or(Error::General("TooManyConnections"))?;
                self.connections.accept(token, stream);
                Ok(Some(token))
            }
            None => Ok(None),
        }
    }

    pub fn connect(&mut self, socket_address: &SocketAddr) -> IoHandlerResult<Option<StreamToken>> {
        Ok(match Stream::connect(socket_address)? {
            Some(stream) => {
                let remote_node_id = socket_address.into();

                let local_node_id =
                    self.routing_table.local_node_id(&remote_node_id).ok_or(Error::General("Not handshaked"))?;
                let session = self.routing_table
                    .unestablished_session(&socket_address)
                    .ok_or(Error::General("Session doesn't exist"))?;

                let token = self.tokens.gen().ok_or(Error::General("TooManyConnections"))?;
                if self.connections.connect(token, stream, local_node_id, session, socket_address, self.port) {
                    self.routing_table.establish(socket_address);
                    Some(token)
                } else {
                    cwarn!(NET, "Cannot create connection to {:?}", socket_address);
                    self.tokens.restore(token);
                    None
                }
            }
            None => None,
        })
    }

    pub fn register_stream(
        &self,
        token: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        self.connections.register(&token, reg, event_loop)?;
        Ok(())
    }

    pub fn reregister_stream(
        &self,
        token: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        self.connections.reregister(&token, reg, event_loop)?;
        Ok(())
    }

    fn deregister_stream(
        &self,
        token: StreamToken,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        self.connections.deregister(&token, event_loop)?;
        self.connections.remove(&token);
        Ok(())
    }

    // Return false if there is no message
    fn receive(&mut self, stream: &StreamToken, client: &Client, io: &IoContext<Message>) -> IoHandlerResult<bool> {
        Ok(match self.connections.receive(stream)? {
            None => false,
            Some(ReceivedMessage::Ack {
                ..
            }) => {
                if !self.connections.establish_wait_ack_connection(stream) {
                    return Err(Error::InvalidStream(*stream).into())
                }
                let node_id = self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
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
                        let remote_addr = self.connections
                            .remote_addr_of_waiting_sync(stream)
                            .ok_or(Error::General("Cannot find remote address"))?;
                        let remote_node_id = convert_to_node_id(remote_addr.ip(), port);

                        if remote_node_id != node_id {
                            return Err(Error::UnexpectedNodeId(Mismatch {
                                expected: remote_node_id,
                                found: node_id,
                            }).into())
                        }

                        let remote_addr = SocketAddr::new(remote_addr.ip(), port);
                        let session = self.routing_table
                            .unestablished_session(&remote_addr)
                            .ok_or(Error::General("Cannot find session"))?;
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
                let session = self.connections.established_session(stream).ok_or(Error::General("Invalid stream"))?;
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
                            cwarn!(NET, "Cannot enqueue negotiation message for {}", stream);
                        }
                    }
                    NegotiationBody::Allowed(extension_version) => {
                        let seq = msg.seq();
                        if let Some(name) = self.connections.remove_requested_negotiation(stream, &seq) {
                            let node_id = self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                            client.on_node_added(&name, &node_id, *extension_version);
                        } else {
                            ctrace!(NET, "Negotiation::Allowed message received from non requested seq");
                        }
                    }
                    NegotiationBody::Denied => {
                        let seq = msg.seq();
                        if let Some(_) = self.connections.remove_requested_negotiation(stream, &seq) {
                            self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                        } else {
                            ctrace!(NET, "Negotiation::Denied message received from non requested seq");
                        }
                    }
                };
                true
            }
        })
    }

    fn send(&mut self, stream: &StreamToken) -> IoHandlerResult<bool> {
        let (connection_type, remain) = self.connections.send(stream)?;
        Ok(match connection_type {
            ConnectionType::None => return Err(Error::InvalidStream(stream.clone()).into()),
            ConnectionType::AckWaiting => {
                debug_assert!(!remain);
                false
            }
            ConnectionType::SyncWaiting => {
                // Ack message was sent
                debug_assert!(!remain);
                self.connections.establish_wait_sync_connection(stream);
                self.connections.node_id(&stream).ok_or(Error::InvalidStream(*stream))?;
                false
            }
            ConnectionType::Established => remain,
        })
    }
}

pub struct Handler {
    socket_address: SocketAddr,
    manager: Mutex<Manager>,
    client: Arc<Client>,

    min_peers: usize,
    max_peers: usize,
}

impl Handler {
    pub fn try_new(
        socket_address: SocketAddr,
        client: Arc<Client>,
        routing_table: Arc<RoutingTable>,
        min_peers: usize,
        max_peers: usize,
    ) -> ::std::result::Result<Self, String> {
        if MAX_CONNECTIONS < max_peers {
            return Err(format!("Max peers must be less than {}", MAX_CONNECTIONS))
        }
        let manager = Mutex::new(Manager::listen(&socket_address, routing_table).expect("Cannot listen TCP port"));
        debug_assert!(max_peers < MAX_CONNECTIONS);
        Ok(Self {
            socket_address,
            manager,
            client,

            min_peers,
            max_peers,
        })
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
                let manager = self.manager.lock();
                let number_of_connections = manager.connections.len();
                if manager.connections.len() < self.min_peers {
                    register_new_timer.store(true, Ordering::SeqCst);
                    let count = (self.min_peers - number_of_connections + 1) / 2;
                    let addresses = manager.routing_table.unestablished_addresses(count);
                    for address in addresses {
                        io.message(Message::RequestConnection(address, IgnoreConnectionLimit::Not))?;
                    }
                }
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn message(&self, io: &IoContext<Message>, message: &Message) -> IoHandlerResult<()> {
        match message {
            Message::RequestConnection(socket_address, ignore_connection_limit) => {
                let mut manager = self.manager.lock();
                if ignore_connection_limit == &IgnoreConnectionLimit::Not {
                    let number_of_connections = manager.connections.len();
                    if self.max_peers <= manager.connections.len() {
                        ctrace!(NET, "Already has maximum peers({})", number_of_connections);
                        return Ok(())
                    }
                }

                ctrace!(NET, "Connecting to {:?}", socket_address);
                let token = manager.connect(&socket_address)?.ok_or(Error::General("Cannot create connection"))?;
                io.register_stream(token)?;
                Ok(())
            }
            Message::RequestNegotiation {
                node_id,
            } => {
                let mut manager = self.manager.lock();
                let versions = self.client.extension_versions();
                for (extension_name, versions) in versions.into_iter() {
                    let token = manager.connections.stream_token(&node_id).ok_or(Error::InvalidNode(*node_id))?;
                    if !manager.connections.enqueue_negotiation_request(&token, extension_name, versions) {
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
                let mut manager = self.manager.lock();
                let token = manager.connections.stream_token(node_id).ok_or(Error::InvalidNode(*node_id))?;
                if !manager.connections.enqueue_extension_message(&token, extension_name, *need_encryption, data) {
                    return Err(Error::InvalidStream(token).into())
                }
                io.update_registration(token)?;
                Ok(())
            }
        }
    }

    fn stream_hup(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => unreachable!(),
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let register_new_timer = AtomicBool::new(false);
                let _f = finally(|| {
                    if register_new_timer.load(Ordering::SeqCst) {
                        io.register_timer_once(CREATE_CONNECTIONS_TOKEN, PULL_CONNECTIONS_MS)
                            .expect("Pull connections must be registered");
                    }
                });

                let manager = self.manager.lock();
                if manager.connections.len() < self.min_peers {
                    register_new_timer.store(true, Ordering::SeqCst);
                }
                let node_id = manager.connections.node_id(&stream).ok_or(Error::InvalidStream(stream))?;
                manager.routing_table.remove_node(node_id.into_addr());
                self.client.on_node_removed(&node_id);
                io.deregister_stream(stream)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn stream_readable(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                let mut manager = self.manager.lock();
                if let Some(token) = manager.accept()? {
                    io.register_stream(token)?;
                }
            }
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let _f = finally(|| {
                    if let Err(err) = io.update_registration(stream) {
                        cwarn!(NET, "Cannot update registration in stream_readable for {} {:?}", stream, err);
                    }
                });
                loop {
                    let mut manager = self.manager.lock();
                    if !manager.receive(&stream, &self.client, io)? {
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
                        cwarn!(NET, "Cannot update registration in stream_writable for {} {:?}", stream, err);
                    }
                });
                loop {
                    let mut manager = self.manager.lock();
                    if !manager.send(&stream)? {
                        break
                    }
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn register_stream(
        &self,
        stream: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                let manager = self.manager.lock();
                event_loop.register(&manager.listener, reg, Ready::readable(), PollOpt::edge())?;
                ctrace!(NET, "TCP connection starts for {:?}", self.socket_address);
                Ok(())
            }
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let mut manager = self.manager.lock();
                manager.register_stream(stream, reg, event_loop)?;
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
                let mut manager = self.manager.lock();
                manager.reregister_stream(stream, reg, event_loop)?;
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
                let mut manager = self.manager.lock();
                manager.deregister_stream(stream, event_loop)?;
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
