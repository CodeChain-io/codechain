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

use std::collections::{HashMap, HashSet};
use std::convert::From;
use std::io;
use std::sync::Arc;

use cfinally::finally;
use cio::{IoChannel, IoContext, IoHandler, IoHandlerResult, IoManager, StreamToken, TimerToken};
use mio::deprecated::EventLoop;
use mio::{PollOpt, Ready, Token};
use parking_lot::{Mutex, RwLock};

use super::super::client::Client;
use super::super::extension::NodeToken;
use super::super::session::Session;
use super::super::session_initiator::Message as SessionMessage;
use super::super::token_generator::TokenGenerator;
use super::super::{DiscoveryApi, NodeId, SocketAddr};
use super::connection::{Connection, ExtensionCallback as ExtensionChannel};
use super::listener::Listener;
use super::message::Version;
use super::pending_connection::WaitSyncConnection;
use super::session_candidate::SessionCandidate;
use super::stream::Stream;

struct Manager {
    listener: Listener,

    tokens: TokenGenerator,
    wait_sync_tokens: HashSet<StreamToken>,
    connections: HashMap<StreamToken, Connection>,
    wait_sync_connections: HashMap<StreamToken, WaitSyncConnection>,

    registered_sessions: SessionCandidate,

    waiting_sync_tokens: TokenGenerator,
    waiting_sync_stream_to_timer: HashMap<StreamToken, TimerToken>,
    waiting_sync_timer_to_stream: HashMap<TimerToken, StreamToken>,

    peer_to_local: HashMap<NodeId, NodeId>,

    port: u16,
}

pub const MAX_CONNECTIONS: usize = 200;

const ACCEPT_TOKEN: TimerToken = 0;

const FIRST_CONNECTION_TOKEN: TimerToken = ACCEPT_TOKEN + 1;
const LAST_CONNECTION_TOKEN: TimerToken = FIRST_CONNECTION_TOKEN + MAX_CONNECTIONS;

const PULL_CONNECTIONS_TOKEN: TimerToken = 0;
const PULL_CONNECTIONS_MS: u64 = 1 * 1000;

const FIRST_WAIT_SYNC_TOKEN: TimerToken = PULL_CONNECTIONS_TOKEN + 1;
const MAX_SYNC_WAITS: usize = 10;
const LAST_WAIT_SYNC_TOKEN: TimerToken = FIRST_WAIT_SYNC_TOKEN + MAX_SYNC_WAITS;

const WAIT_SYNC_MS: u64 = 10 * 1000;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Message {
    RegisterSession {
        local_node_id: NodeId,
        remote_node_id: NodeId,
        remote_addr: SocketAddr,
        session: Session,
    },

    RequestConnection(SocketAddr, Session),

    RequestNegotiation {
        node_id: NodeToken,
        extension_name: String,
        version: Version,
    },
    SendExtensionMessage {
        node_id: NodeToken,
        extension_name: String,
        need_encryption: bool,
        data: Vec<u8>,
    },
}

#[derive(Debug)]
enum Error {
    InvalidStream(StreamToken),
    InvalidNode(NodeToken),
    General(&'static str),
}

type Result<T> = ::std::result::Result<T, Error>;

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            Error::InvalidStream(_) => ::std::fmt::Debug::fmt(self, f),
            Error::InvalidNode(_) => ::std::fmt::Debug::fmt(self, f),
            Error::General(_) => ::std::fmt::Debug::fmt(self, f),
        }
    }
}

const WAIT_CREATE_CONNECTION: usize = 5;

impl Manager {
    pub fn listen(socket_address: &SocketAddr) -> io::Result<Self> {
        Ok(Manager {
            listener: Listener::bind(&socket_address)?,

            tokens: TokenGenerator::new(FIRST_CONNECTION_TOKEN, LAST_CONNECTION_TOKEN),
            wait_sync_tokens: HashSet::new(),
            connections: HashMap::new(),
            wait_sync_connections: HashMap::new(),

            registered_sessions: SessionCandidate::new(WAIT_CREATE_CONNECTION),

            waiting_sync_tokens: TokenGenerator::new(FIRST_WAIT_SYNC_TOKEN, LAST_WAIT_SYNC_TOKEN),
            waiting_sync_stream_to_timer: HashMap::new(),
            waiting_sync_timer_to_stream: HashMap::new(),

            peer_to_local: HashMap::new(),

            port: socket_address.port(),
        })
    }

    fn register_unprocessed_connection(&mut self, stream: Stream) -> Result<(StreamToken, TimerToken)> {
        let token = self.tokens.gen().ok_or(Error::General("TooManyConnections"))?;
        let timer_token = {
            if let Some(timer_token) = self.waiting_sync_tokens.gen() {
                timer_token
            } else {
                return Err(Error::General("TooManyWaitingSync"))
            }
        };

        let t = self.waiting_sync_stream_to_timer.insert(token, timer_token);
        debug_assert!(t.is_none());
        let t = self.waiting_sync_timer_to_stream.insert(token, timer_token);
        debug_assert!(t.is_none());

        let connection = WaitSyncConnection::new(stream);

        let con = self.wait_sync_connections.insert(token, connection);
        debug_assert!(con.is_none());

        let t = self.wait_sync_tokens.insert(token);
        debug_assert!(t);

        Ok((token, timer_token))
    }

    fn register_connection(&mut self, connection: Connection, token: &StreamToken, client: &Client) {
        let con = self.connections.insert(*token, connection);
        client.on_node_added(token);
        debug_assert!(con.is_none());
    }

    fn process_connection(&mut self, wait_sync_token: &StreamToken) -> Connection {
        let wait_sync_connection = self.remove_waiting_sync_by_stream_token(&wait_sync_token).unwrap();

        let mut connection = wait_sync_connection.process();
        connection.enqueue_ack();
        connection
    }

    fn deregister_unprocessed_connection(&mut self, token: &StreamToken) {
        if let Some(_) = self.wait_sync_connections.remove(&token) {
            let t = self.tokens.restore(*token);
            debug_assert!(t);
            let t = self.wait_sync_tokens.remove(&token);
            debug_assert!(t);
        } else {
            unreachable!()
        }
    }

    fn deregister_connection(&mut self, token: &StreamToken) {
        if let Some(_) = self.connections.remove(&token) {
            let t = self.tokens.restore(*token);
            debug_assert!(t);
        } else {
            unreachable!()
        }
    }

    fn create_connection(
        &mut self,
        stream: Stream,
        session: &Session,
        client: &Client,
    ) -> IoHandlerResult<StreamToken> {
        let peer_node_id: NodeId = stream.peer_addr()?.into();
        let mut connection = Connection::new(stream, session.secret().clone(), session.id().clone(), peer_node_id);
        let local_id =
            self.peer_to_local.get(&peer_node_id).ok_or(Error::General("Node id is not registrerd"))?.clone();
        connection.enqueue_sync(self.port, local_id);
        let removed = self.registered_sessions.remove(connection.peer_node_id());
        debug_assert!(removed);

        Ok(self.tokens
            .gen()
            .map(|token| {
                self.register_connection(connection, &token, client);
                token
            })
            .expect("The number of peers must be checked before"))
    }

    pub fn accept(&mut self) -> IoHandlerResult<Option<(StreamToken, TimerToken, SocketAddr)>> {
        match self.listener.accept()? {
            Some((stream, socket_address)) => {
                let (stream_token, timer_token) = self.register_unprocessed_connection(stream)?;
                Ok(Some((stream_token, timer_token, socket_address)))
            }
            None => Ok(None),
        }
    }

    pub fn connect(
        &mut self,
        socket_address: &SocketAddr,
        session: &Session,
        client: &Client,
    ) -> IoHandlerResult<Option<StreamToken>> {
        Ok(match Stream::connect(socket_address)? {
            Some(stream) => Some(self.create_connection(stream, session, client)?),
            None => None,
        })
    }

    fn register_session(&mut self, node_id: &NodeId, socket_address: &SocketAddr, session: &Session) -> Result<()> {
        if self.registered_sessions.contains_key(node_id) {
            return Err(Error::General("SessionAlreadyRegistered"))
        }
        let inserted = self.registered_sessions.insert(node_id.clone(), session.clone(), socket_address.clone());
        debug_assert!(inserted);
        Ok(())
    }

    pub fn register_stream(
        &self,
        token: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        if let Some(connection) = self.connections.get(&token) {
            return Ok(connection.register(reg, event_loop)?)
        }

        let connection = self.wait_sync_connections.get(&token).ok_or(Error::InvalidStream(token))?;
        Ok(connection.register(reg, event_loop)?)
    }

    pub fn reregister_stream(
        &self,
        token: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        if let Some(connection) = self.connections.get(&token) {
            return Ok(connection.reregister(reg, event_loop)?)
        }

        let connection = self.wait_sync_connections.get(&token).ok_or(Error::InvalidStream(token))?;
        Ok(connection.reregister(reg, event_loop)?)
    }

    // return false if it's wait sync connection
    fn deregister_stream(
        &self,
        token: StreamToken,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<bool> {
        if let Some(connection) = self.connections.get(&token) {
            connection.deregister(event_loop)?;
            return Ok(true)
        }

        if let Some(connection) = self.wait_sync_connections.get(&token) {
            connection.deregister(event_loop)?;
            return Ok(false)
        }

        Err(From::from(Error::InvalidStream(token)))
    }

    // Return false if the received message is sync
    fn receive(&mut self, stream: &StreamToken, client: &Client) -> IoHandlerResult<bool> {
        if let Some(connection) = self.connections.get_mut(&stream) {
            return Ok(connection.receive(&ExtensionChannel::new(&client, *stream)))
        }

        {
            // connection borrows *self as mutable
            let connection = self.wait_sync_connections.get_mut(&stream).ok_or(Error::InvalidStream(stream.clone()))?;
            if let Some(_) = connection.receive(&self.registered_sessions)? {
                // Sync
            } else {
                return Ok(true)
            }
        }

        // receive Sync message
        let connection = self.process_connection(&stream);

        // Session is not reusable
        let removed = self.registered_sessions.remove(connection.peer_node_id());
        debug_assert!(removed);

        self.register_connection(connection, stream, client);
        Ok(false)
    }

    fn send(&mut self, stream: &StreamToken) -> IoHandlerResult<bool> {
        let connection = self.connections.get_mut(&stream).ok_or(Error::InvalidStream(stream.clone()))?;
        Ok(connection.send()?)
    }

    fn remove_waiting_sync_by_stream_token(&mut self, stream: &StreamToken) -> Option<WaitSyncConnection> {
        if let Some(timer) = self.waiting_sync_stream_to_timer.remove(&stream) {
            let t = self.waiting_sync_tokens.restore(timer);
            debug_assert!(t);

            let t = self.waiting_sync_timer_to_stream.remove(&stream);
            debug_assert!(t.is_some());

            let t = self.wait_sync_tokens.remove(&stream);
            debug_assert!(t);

            let t = self.wait_sync_connections.remove(&stream);
            debug_assert!(t.is_some());
            t
        } else {
            None
        }
    }

    fn remove_waiting_sync_by_timer_token(&mut self, timer: &TimerToken) {
        if let Some(stream) = self.waiting_sync_timer_to_stream.remove(&timer) {
            let t = self.waiting_sync_tokens.restore(*timer);
            debug_assert!(t);

            let t = self.waiting_sync_stream_to_timer.remove(&stream);
            debug_assert!(t.is_some());

            let t = self.wait_sync_tokens.remove(&stream);
            debug_assert!(t);

            let t = self.wait_sync_connections.remove(&stream);
            debug_assert!(t.is_some());
        }
    }
}

pub struct Handler {
    socket_address: SocketAddr,
    manager: Mutex<Manager>,
    client: Arc<Client>,

    discovery: RwLock<Option<Arc<DiscoveryApi>>>,
    session_initiator: IoChannel<SessionMessage>,

    min_peers: usize,
    max_peers: usize,
}

impl Handler {
    pub fn try_new(
        socket_address: SocketAddr,
        client: Arc<Client>,
        session_initiator: IoChannel<SessionMessage>,
        min_peers: usize,
        max_peers: usize,
    ) -> ::std::result::Result<Self, String> {
        if MAX_CONNECTIONS < max_peers {
            return Err(format!("Max peers must be less than {}", MAX_CONNECTIONS))
        }
        let manager = Mutex::new(Manager::listen(&socket_address).expect("Cannot listen TCP port"));
        debug_assert!(max_peers < MAX_CONNECTIONS);
        Ok(Self {
            socket_address,
            manager,
            client,

            discovery: RwLock::new(None),
            session_initiator,

            min_peers,
            max_peers,
        })
    }

    pub fn set_discovery_api(&self, api: Arc<DiscoveryApi>) {
        *self.discovery.write() = Some(api);
    }
}

impl IoHandler<Message> for Handler {
    fn initialize(&self, io: &IoContext<Message>) -> IoHandlerResult<()> {
        io.register_stream(ACCEPT_TOKEN)?;
        io.register_timer_once(PULL_CONNECTIONS_TOKEN, PULL_CONNECTIONS_MS)?;
        Ok(())
    }

    fn timeout(&self, io: &IoContext<Message>, token: TimerToken) -> IoHandlerResult<()> {
        match token {
            PULL_CONNECTIONS_TOKEN => {
                io.register_timer_once(PULL_CONNECTIONS_TOKEN, PULL_CONNECTIONS_MS)?;
                let mut manager = self.manager.lock();
                manager.registered_sessions.promote();
                if self.min_peers <= manager.connections.len() {
                    return Ok(())
                }

                let num_of_requests = self.min_peers - manager.connections.len();
                // FIXME: Pick random session
                let mut count: usize = 0;
                for (_, &(ref session, ref socket_address)) in manager.registered_sessions.iter().take(num_of_requests)
                {
                    count += 1;
                    io.channel().send(Message::RequestConnection(socket_address.clone(), session.clone()))?;
                }
                if count + manager.connections.len() < self.min_peers {
                    let requests = self.min_peers - count - manager.connections.len();
                    self.session_initiator.send(SessionMessage::RequestSession(requests))?;
                }

                Ok(())
            }
            FIRST_WAIT_SYNC_TOKEN...LAST_WAIT_SYNC_TOKEN => {
                let mut manager = self.manager.lock();
                manager.remove_waiting_sync_by_timer_token(&token);
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn message(&self, io: &IoContext<Message>, message: &Message) -> IoHandlerResult<()> {
        match message {
            Message::RegisterSession {
                local_node_id,
                remote_node_id,
                remote_addr,
                session,
            } => {
                let mut manager = self.manager.lock();
                manager.peer_to_local.insert(remote_node_id.clone(), local_node_id.clone());
                manager.register_session(&remote_node_id, remote_addr, session)?;
                Ok(())
            }
            Message::RequestConnection(socket_address, session) => {
                let mut manager = self.manager.lock();
                if self.max_peers <= manager.connections.len() {
                    ctrace!(NET, "Already has maximum peers({})", manager.connections.len());
                    return Ok(())
                }

                ctrace!(NET, "Connecting to {:?}", socket_address);
                let token = manager
                    .connect(&socket_address, session, &self.client)?
                    .ok_or(Error::General("Cannot create connection"))?;
                io.register_stream(token)?;

                if let Some(ref discovery) = *self.discovery.read() {
                    discovery.add_connection(token, socket_address.clone());
                }
                Ok(())
            }
            Message::RequestNegotiation {
                node_id,
                extension_name,
                version,
            } => {
                let mut manager = self.manager.lock();
                let mut connection = manager.connections.get_mut(node_id).ok_or(Error::InvalidNode(*node_id))?;
                connection.enqueue_negotiation_request(extension_name.clone(), *version);
                io.update_registration(*node_id)?;
                Ok(())
            }
            Message::SendExtensionMessage {
                node_id,
                extension_name,
                need_encryption,
                data,
            } => {
                let mut manager = self.manager.lock();
                let mut connection = manager.connections.get_mut(node_id).ok_or(Error::InvalidNode(*node_id))?;
                connection.enqueue_extension_message(extension_name.clone(), *need_encryption, &data);
                io.update_registration(*node_id)?;
                Ok(())
            }
        }
    }

    fn stream_hup(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => unreachable!(),
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                self.client.on_node_removed(&stream);
                io.deregister_stream(stream)?;
                if let Some(ref discovery) = *self.discovery.read() {
                    discovery.remove_connection(&stream);
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn stream_readable(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => loop {
                let mut manager = self.manager.lock();
                if let Some((token, timer_token, socket_address)) = manager.accept()? {
                    io.register_stream(token)?;
                    io.register_timer_once(timer_token, WAIT_SYNC_MS)?;
                    if let Some(ref discovery) = *self.discovery.read() {
                        discovery.add_connection(token, socket_address.clone());
                    }
                }
                break
            },
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let _f = finally(|| {
                    if let Err(err) = io.update_registration(stream) {
                        cwarn!(NET, "Cannot update registration for connection {:?}", err);
                    }
                });
                loop {
                    let mut manager = self.manager.lock();
                    if !manager.receive(&stream, &self.client)? {
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
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => loop {
                let _f = finally(|| {
                    if let Err(err) = io.update_registration(stream) {
                        cwarn!(NET, "Cannot update registration for connection {:?}", err);
                    }
                });
                let mut manager = self.manager.lock();
                if manager.wait_sync_tokens.contains(&stream) {
                    break
                }
                if !manager.send(&stream)? {
                    break
                }
            },
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
                let is_processed = manager.deregister_stream(stream, event_loop)?;
                if is_processed {
                    manager.deregister_connection(&stream);
                } else {
                    manager.deregister_unprocessed_connection(&stream);
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}
