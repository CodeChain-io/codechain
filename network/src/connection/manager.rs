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
use std::convert::{From, Into};
use std::io;
use std::sync::Arc;

use cio::{IoContext, IoHandler, IoHandlerResult, IoManager, StreamToken, TimerToken};
use mio::deprecated::EventLoop;
use mio::net::{TcpListener, TcpStream};
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use parking_lot::{Mutex, RwLock};

use super::super::client::Client;
use super::super::extension::{Error as ExtensionError, NodeToken};
use super::super::session::{Nonce, Session, SessionTable};
use super::super::timer_info::{Error as TimerInfoError, TimerInfo};
use super::super::token_generator::TokenGenerator;
use super::super::SocketAddr;
use super::connection::{Connection, ExtensionCallback as ExtensionChannel};
use super::message::Version;
use super::unprocessed_connection::UnprocessedConnection;

pub struct Manager {
    listener: TcpListener,

    tokens: TokenGenerator,
    unprocessed_tokens: HashSet<StreamToken>,
    connections: HashMap<StreamToken, Connection>,
    unprocessed_connections: HashMap<StreamToken, UnprocessedConnection>,

    registered_sessions: HashMap<Nonce, Session>,
    socket_to_session: SessionTable,
}

const MAX_CONNECTIONS: usize = 32;

const ACCEPT_TOKEN: usize = 0;

const FIRST_CONNECTION_TOKEN: usize = ACCEPT_TOKEN + 1;
const LAST_CONNECTION_TOKEN: usize = FIRST_CONNECTION_TOKEN + MAX_CONNECTIONS;

const FIRST_TIMER_TOKEN: usize = LAST_CONNECTION_TOKEN;
const MAX_TIMERS: usize = 100;

type TimerId = usize;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    RegisterSession(SocketAddr, Session),

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

    SetTimer {
        extension_name: String,
        timer_id: TimerId,
        ms: u64,
    },
    SetTimerOnce {
        extension_name: String,
        timer_id: TimerId,
        ms: u64,
    },
    ClearTimer {
        extension_name: String,
        timer_id: TimerId,
    },
}

#[derive(Debug)]
enum Error {
    UnavailableSession,
    InvalidStream(StreamToken),
    InvalidTimer(TimerToken),
    InvalidNode(NodeToken),
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            &Error::UnavailableSession => write!(f, "Unavailable session"),
            &Error::InvalidStream(token) => write!(f, "{} is an invalid stream token", token),
            &Error::InvalidTimer(token) => write!(f, "{} is an invalid timer token", token),
            &Error::InvalidNode(id) => write!(f, "{} is an invalid node id", id),
        }
    }
}


impl Manager {
    pub fn listen(socket_address: &SocketAddr) -> io::Result<Self> {
        Ok(Manager {
            listener: TcpListener::bind(socket_address.into())?,

            tokens: TokenGenerator::new(FIRST_CONNECTION_TOKEN, LAST_CONNECTION_TOKEN),
            unprocessed_tokens: HashSet::new(),
            connections: HashMap::new(),
            unprocessed_connections: HashMap::new(),

            registered_sessions: HashMap::new(),
            socket_to_session: SessionTable::new(),
        })
    }

    fn register_unprocessed_connection(&mut self, stream: TcpStream) -> Option<StreamToken> {
        self.tokens.gen().map(|token| {
            let connection = UnprocessedConnection::new(stream);

            let con = self.unprocessed_connections.insert(token, connection);
            debug_assert!(con.is_none());

            let t = self.unprocessed_tokens.insert(token);
            debug_assert!(t);

            token
        })
    }

    fn register_connection(&mut self, connection: Connection, token: &StreamToken) -> StreamToken {
        let con = self.connections.insert(*token, connection);
        debug_assert!(con.is_none());

        *token
    }

    fn process_connection(&mut self, unprocessed_token: &StreamToken) -> StreamToken {
        let t = self.unprocessed_tokens.remove(&unprocessed_token);
        debug_assert!(t);
        let unprocessed = self.unprocessed_connections.remove(&unprocessed_token).expect("It must exist");

        let mut connection = unprocessed.process();
        connection.enqueue_ack();
        self.register_connection(connection, unprocessed_token)
    }

    fn create_connection(
        &mut self,
        stream: TcpStream,
        socket_address: &SocketAddr,
    ) -> IoHandlerResult<Option<StreamToken>> {
        let session = self.socket_to_session.get(&socket_address).ok_or(Error::UnavailableSession)?.clone();
        let mut connection = Connection::new(stream, session.secret().clone(), session.nonce().clone());
        let nonce = session.nonce();
        connection.enqueue_sync(nonce.clone());

        Ok(self.tokens.gen().map(|token| self.register_connection(connection, &token)))
    }

    pub fn accept(&mut self) -> IoHandlerResult<Option<(StreamToken, SocketAddr)>> {
        match self.listener.accept() {
            Ok((stream, socket_address)) => {
                Ok(self.register_unprocessed_connection(stream).map(|token| (token, socket_address.into())))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(From::from(e)),
        }
    }

    pub fn connect(&mut self, socket_address: &SocketAddr) -> IoHandlerResult<Option<StreamToken>> {
        match TcpStream::connect(socket_address.into()) {
            Ok(stream) => self.create_connection(stream, &socket_address),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(From::from(e)),
        }
    }

    pub fn register_session(&mut self, socket_address: SocketAddr, session: Session) -> IoHandlerResult<()> {
        if self.socket_to_session.contains_key(&socket_address) {
            info!("Session registration is requested to the address which already has one");
            return Ok(())
        }

        self.registered_sessions.insert(session.nonce().clone(), session.clone());
        self.socket_to_session.insert(socket_address, session);
        Ok(())
    }

    pub fn register_stream(
        &self,
        token: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<HandlerMessage>>,
    ) -> IoHandlerResult<()> {
        if let Some(connection) = self.connections.get(&token) {
            event_loop.register(
                connection.stream(),
                reg,
                Ready::readable() | Ready::writable() | UnixReady::hup(),
                PollOpt::edge(),
            )?;
            return Ok(())
        }
        if let Some(connection) = self.unprocessed_connections.get(&token) {
            event_loop.register(connection.stream(), reg, Ready::readable() | Ready::writable(), PollOpt::edge())?;
            return Ok(())
        }
        Err(From::from(Error::InvalidStream(token)))
    }

    pub fn update_stream(
        &self,
        token: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<HandlerMessage>>,
    ) -> IoHandlerResult<()> {
        if let Some(connection) = self.connections.get(&token) {
            event_loop.reregister(
                connection.stream(),
                reg,
                Ready::readable() | Ready::writable() | UnixReady::hup(),
                PollOpt::edge(),
            )?;
            return Ok(())
        }
        if let Some(connection) = self.unprocessed_connections.get(&token) {
            event_loop.reregister(connection.stream(), reg, Ready::readable() | Ready::writable(), PollOpt::edge())?;
            return Ok(())
        }
        Err(From::from(Error::InvalidStream(token)))
    }

    fn deregister_stream(
        &mut self,
        token: StreamToken,
        event_loop: &mut EventLoop<IoManager<HandlerMessage>>,
    ) -> IoHandlerResult<()> {
        if let Some(connection) = self.connections.remove(&token) {
            let t = self.tokens.restore(token);
            debug_assert!(t);
            event_loop.deregister(connection.stream())?;
            return Ok(())
        }

        if let Some(connection) = self.unprocessed_connections.remove(&token) {
            let t = self.tokens.restore(token);
            debug_assert!(t);
            let t = self.unprocessed_tokens.remove(&token);
            debug_assert!(t);
            event_loop.deregister(connection.stream())?;
            return Ok(())
        }

        Err(From::from(Error::InvalidStream(token)))
    }

    fn receive(&mut self, stream: &StreamToken, client: &Client) -> IoHandlerResult<bool> {
        if let Some(connection) = self.connections.get_mut(&stream) {
            return Ok(connection.receive(&ExtensionChannel::new(&client, *stream)))
        } else if let Some(connection) = self.unprocessed_connections.get_mut(&stream) {
            if let Some(_) = connection.receive(&self.registered_sessions)? {
                // Sync
            } else {
                return Ok(true)
            }
        } else {
            info!("readable event for unregistered stream({:?})", stream);
            return Ok(false)
        }

        // receive Sync message
        let session = {
            let connection = self.unprocessed_connections.get(&stream).unwrap();
            connection.session().clone().unwrap()
        };
        let nonce = session.nonce().clone();
        self.registered_sessions.insert(nonce, session);
        let processed_token = self.process_connection(&stream);
        debug_assert_eq!(&processed_token, stream);
        client.on_node_added(&stream);
        Ok(false)
    }

    fn send(&mut self, stream: &StreamToken) -> IoHandlerResult<bool> {
        if let Some(connection) = self.connections.get_mut(&stream) {
            return Ok(connection.send()?)
        } else {
            return Err(From::from(Error::InvalidStream(stream.clone())))
        }
    }
}

pub struct Handler {
    socket_address: SocketAddr,
    manager: Mutex<Manager>,
    client: Arc<Client>,
    timer: Mutex<TimerInfo>,

    node_token_to_socket: RwLock<HashMap<NodeToken, SocketAddr>>,
    socket_to_node_token: RwLock<HashMap<SocketAddr, NodeToken>>,
}

impl Handler {
    pub fn new(socket_address: SocketAddr, client: Arc<Client>) -> Self {
        let manager = Mutex::new(Manager::listen(&socket_address).expect("Cannot listen TCP port"));
        Self {
            socket_address,
            manager,
            client,
            timer: Mutex::new(TimerInfo::new(FIRST_TIMER_TOKEN, MAX_TIMERS)),

            node_token_to_socket: RwLock::new(HashMap::new()),
            socket_to_node_token: RwLock::new(HashMap::new()),
        }
    }
}

impl IoHandler<HandlerMessage> for Handler {
    fn initialize(&self, io: &IoContext<HandlerMessage>) -> IoHandlerResult<()> {
        io.register_stream(ACCEPT_TOKEN)?;
        Ok(())
    }

    fn timeout(&self, _io: &IoContext<HandlerMessage>, token: TimerToken) -> IoHandlerResult<()> {
        let mut timer = self.timer.lock();
        let info = timer.get_info(token).ok_or(Error::InvalidTimer(token))?;
        if info.once {
            timer.remove_by_token(token);
        }
        self.client.on_timeout(&info.name, info.timer_id);
        Ok(())
    }

    fn message(&self, io: &IoContext<HandlerMessage>, message: &HandlerMessage) -> IoHandlerResult<()> {
        match *message {
            HandlerMessage::RegisterSession(ref socket_address, ref session) => {
                let mut manager = self.manager.lock();
                info!("Register session {:?}", session);

                manager.register_session(socket_address.clone(), session.clone())?;
            }
            HandlerMessage::RequestConnection(ref socket_address, ref session) => {
                let mut manager = self.manager.lock();

                info!("Register session {:?}", session);
                let _ = manager.register_session(socket_address.clone(), session.clone());

                info!("Connecting to {:?}", socket_address);
                if let Some(token) = manager.connect(&socket_address)? {
                    io.register_stream(token)?;
                    self.socket_to_node_token.write().insert(socket_address.clone(), token);
                    self.node_token_to_socket.write().insert(token, socket_address.clone());
                } else {
                    info!("There are no available tokens");
                }
            }
            HandlerMessage::RequestNegotiation {
                node_id,
                ref extension_name,
                version,
            } => {
                let mut manager = self.manager.lock();
                let mut connection = manager.connections.get_mut(&node_id).ok_or(Error::InvalidNode(node_id))?;
                connection.enqueue_negotiation_request(extension_name.clone(), version);
                io.update_registration(node_id)?;
            }
            HandlerMessage::SendExtensionMessage {
                node_id,
                ref extension_name,
                ref need_encryption,
                ref data,
            } => {
                let mut manager = self.manager.lock();
                let mut connection = manager.connections.get_mut(&node_id).ok_or(Error::InvalidNode(node_id))?;
                connection.enqueue_extension_message(extension_name.clone(), *need_encryption, data.clone());
                io.update_registration(node_id)?;
            }

            HandlerMessage::SetTimer {
                ref extension_name,
                timer_id,
                ms,
            } => {
                let mut timer = self.timer.lock();
                match timer.insert(extension_name.clone(), timer_id, false) {
                    Ok(token) => {
                        io.register_timer(token, ms)?;
                        self.client.on_timer_set_allowed(extension_name, timer_id);
                    }
                    Err(TimerInfoError::DuplicatedTimerId) => {
                        self.client.on_timer_set_denied(extension_name, timer_id, ExtensionError::DuplicatedTimerId);
                    }
                    Err(TimerInfoError::NoSpace) => {
                        self.client.on_timer_set_denied(extension_name, timer_id, ExtensionError::NoMoreTimerToken);
                    }
                }
            }
            HandlerMessage::SetTimerOnce {
                ref extension_name,
                timer_id,
                ms,
            } => {
                let mut timer = self.timer.lock();
                match timer.insert(extension_name.clone(), timer_id, true) {
                    Ok(token) => {
                        io.register_timer_once(token, ms)?;
                        self.client.on_timer_set_allowed(extension_name, timer_id);
                    }
                    Err(TimerInfoError::DuplicatedTimerId) => {
                        self.client.on_timer_set_denied(extension_name, timer_id, ExtensionError::DuplicatedTimerId);
                    }
                    Err(TimerInfoError::NoSpace) => {
                        self.client.on_timer_set_denied(extension_name, timer_id, ExtensionError::NoMoreTimerToken);
                    }
                }
            }
            HandlerMessage::ClearTimer {
                ref extension_name,
                timer_id,
            } => {
                let mut timer = self.timer.lock();
                let token = timer.remove_by_info(extension_name.clone(), timer_id).expect("Unexpected timer id");
                io.clear_timer(token)?;
            }
        };
        Ok(())
    }

    fn stream_readable(&self, io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => loop {
                let mut manager = self.manager.lock();
                if let Some((token, socket_address)) = manager.accept()? {
                    io.register_stream(token)?;
                    self.socket_to_node_token.write().insert(socket_address.clone(), token);
                    self.node_token_to_socket.write().insert(token, socket_address);
                }
                break
            },
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                loop {
                    let mut manager = self.manager.lock();
                    if !manager.receive(&stream, &self.client)? {
                        break
                    }
                }
                io.update_registration(stream)?;
            }
            _ => unimplemented!(),
        }
        Ok(())
    }

    fn stream_writable(&self, _io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {}
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => loop {
                let mut manager = self.manager.lock();
                if manager.unprocessed_tokens.contains(&stream) {
                    break
                }
                if !manager.send(&stream)? {
                    break
                }
            },
            _ => unimplemented!(),
        }
        Ok(())
    }

    fn register_stream(
        &self,
        stream: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<HandlerMessage>>,
    ) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                let manager = self.manager.lock();
                event_loop.register(&manager.listener, reg, Ready::readable() | Ready::writable(), PollOpt::edge())?;
                info!("TCP connection starts for {:?}", self.socket_address);
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
        event_loop: &mut EventLoop<IoManager<HandlerMessage>>,
    ) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                unreachable!();
            }
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let mut manager = self.manager.lock();
                manager.update_stream(stream, reg, event_loop)?;
                Ok(())
            }
            _ => {
                unreachable!();
            }
        }
    }

    fn stream_hup(&self, io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => unreachable!(),
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                io.deregister_stream(stream)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn deregister_stream(
        &self,
        stream: StreamToken,
        event_loop: &mut EventLoop<IoManager<HandlerMessage>>,
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


pub trait AddressConverter: Send + Sync {
    fn node_token_to_address(&self, node: &NodeToken) -> Option<SocketAddr>;
    fn address_to_node_token(&self, address: &SocketAddr) -> Option<NodeToken>;
}

impl AddressConverter for Handler {
    fn node_token_to_address(&self, node_id: &NodeToken) -> Option<SocketAddr> {
        let node_id_to_socket = self.node_token_to_socket.read();
        node_id_to_socket.get(&node_id).map(|socket_address| socket_address.clone())
    }

    fn address_to_node_token(&self, socket_address: &SocketAddr) -> Option<NodeToken> {
        let socket_to_node_token = self.socket_to_node_token.read();
        socket_to_node_token.get(&socket_address).map(|id| id.clone())
    }
}
