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

use cio::{IoContext, IoHandler, IoManager, IoHandlerResult, StreamToken, TimerToken};
use mio::deprecated::EventLoop;
use mio::net::{TcpListener, TcpStream};
use mio::{PollOpt, Ready, Token};
use parking_lot::Mutex;

use super::connection::{Connection, ExtensionCallback as ExtensionChannel};
use super::message::Version;
use super::super::SocketAddr;
use super::super::client::Client;
use super::super::extension::{Error as ExtensionError, NodeId};
use super::super::limited_table::{Key as ConnectionToken, LimitedTable};
use super::super::session::{Session, SessionTable};
use super::super::timer_info::{Error as TimerInfoError, TimerInfo};

pub struct Manager {
    listener: TcpListener,
    connections: LimitedTable<Connection>,
    socket_to_node_id: HashMap<SocketAddr, NodeId>,
    socket_to_session: SessionTable,
    inbound_tokens: HashSet<ConnectionToken>,
}

const ACCEPT_TOKEN: usize = 0;
const FIRST_CONNECTION_TOKEN: usize = ACCEPT_TOKEN + 1;
const MAX_CONNECTIONS: usize = 32;
const LAST_CONNECTION_TOKEN: usize = FIRST_CONNECTION_TOKEN + MAX_CONNECTIONS;

const FIRST_TIMER_TOKEN: usize = LAST_CONNECTION_TOKEN;
const MAX_TIMERS: usize = 100;

type TimerId = usize;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    RegisterSession(SocketAddr, Session),

    RequestConnection(SocketAddr, Session),

    RequestNegotiation {
        node_id: NodeId,
        extension_name: String,
        version: Version,
    },
    SendExtensionMessage {
        node_id: NodeId,
        extension_name: String,
        need_encryption: bool,
        data: Vec<u8 >,
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
    NoAvailableSession,
    NotValidStream(StreamToken),
    NotValidTimer(TimerToken),
    NotValidNode(NodeId),
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            &Error::NoAvailableSession => write!(f, "No available session"),
            &Error::NotValidStream(token) => write!(f, "{} is not a valid stream token", token),
            &Error::NotValidTimer(token) => write!(f, "{} is not a valid timer token", token),
            &Error::NotValidNode(id) => write!(f, "{} is not a valid node id", id),
        }
    }
}


impl Manager {
    pub fn listen(socket_address: &SocketAddr) -> io::Result<Self> {
        Ok(Manager {
            listener: TcpListener::bind(socket_address.into())?,
            connections: LimitedTable::new(FIRST_CONNECTION_TOKEN, MAX_CONNECTIONS),
            socket_to_node_id: HashMap::new(),
            socket_to_session: SessionTable::new(),
            inbound_tokens: HashSet::new(),
        })
    }

    fn register_token(&mut self, stream: TcpStream, socket_address: &SocketAddr, is_inbound: bool) -> IoHandlerResult<Option<ConnectionToken>> {
        let session = self.socket_to_session.get(&socket_address).ok_or(Error::NoAvailableSession)?;
        let connection = Connection::new(stream, session.clone())?;
        if let Some(token) = self.connections.insert(connection) {
            self.socket_to_node_id.insert(socket_address.clone(), token);
            if is_inbound {
                let _ = self.inbound_tokens.insert(token);
            }
            Ok(Some(token))
        } else {
            Ok(None)
        }
    }

    pub fn accept(&mut self, _io: &IoContext<HandlerMessage>) -> IoHandlerResult<Option<ConnectionToken>> {
        match self.listener.accept() {
            Ok((stream, socket_address)) => {
                self.register_token(stream, &SocketAddr::from(socket_address), true)
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(From::from(e)),
        }
    }

    pub fn connect(&mut self, socket_address: &SocketAddr) -> IoHandlerResult<Option<ConnectionToken>> {
        match TcpStream::connect(socket_address.into()) {
            Ok(stream) => {
                self.register_token(stream, &socket_address, false)
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(From::from(e)),
        }
    }

    pub fn register_session(&mut self, socket_address: SocketAddr, session: Session) ->IoHandlerResult<()> {
        debug_assert!(session.is_ready());
        if self.socket_to_session.contains_key(&socket_address) {
            info!("Session registration is requested to the address which already has one");
            return Ok(())
        }

        self.socket_to_session.insert(socket_address, session);
        Ok(())
    }

    pub fn register_stream(&self, token: ConnectionToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) -> IoHandlerResult<()> {
        let connection = self.connections.get(token).ok_or(Error::NotValidStream(token))?;
        event_loop.register(connection.stream(), reg, Ready::readable() | Ready::writable(), PollOpt::edge())?;
        Ok(())
    }

    pub fn update_stream(&self, token: ConnectionToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) -> IoHandlerResult<()> {
        let connection = self.connections.get(token).ok_or(Error::NotValidStream(token))?;
        event_loop.reregister(connection.stream(), reg, Ready::readable() | Ready::writable(), PollOpt::edge())?;
        Ok(())
    }

    pub fn is_inbound(&self, token: ConnectionToken) -> bool {
        self.inbound_tokens.contains(&token)
    }
}

pub struct Handler {
    socket_address: SocketAddr,
    manager: Mutex<Manager>,
    client: Arc<Client>,
    timer: Mutex<TimerInfo>,
}

impl Handler {
    pub fn new(socket_address: SocketAddr, client: Arc<Client>) -> Self {
        let manager = Mutex::new(Manager::listen(&socket_address).expect("Cannot listen TCP port"));
        Self {
            socket_address,
            manager,
            client,
            timer: Mutex::new(TimerInfo::new(FIRST_TIMER_TOKEN, MAX_TIMERS)),
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
        let info = timer.get_info(token).ok_or(Error::NotValidTimer(token))?;
        if info.once {
            timer.remove_by_token(token);
        }
        self.client.on_timeout(&info.name, info.timer_id);
        Ok(())
    }

    fn message(&self, io: &IoContext<HandlerMessage>, message: &HandlerMessage) -> IoHandlerResult<()> {
        match *message {
            HandlerMessage::RegisterSession(ref socket_address, ref session) => {
                debug_assert!(session.is_ready());
                let mut manager = self.manager.lock();
                info!("Register session {:?}", session);
                manager.register_session(socket_address.clone(), session.clone())?;
            },
            HandlerMessage::RequestConnection(ref socket_address, ref session) => {
                debug_assert!(session.is_ready());
                let mut manager = self.manager.lock();

                info!("Register session {:?}", session);
                let _ = manager.register_session(socket_address.clone(), session.clone());

                info!("Connecting to {:?}", socket_address);
                if let Some(token) = manager.connect(&socket_address)? {
                    io.register_stream(token)?;
                } else {
                    info!("There are no available tokens");
                }
            },
            HandlerMessage::RequestNegotiation { node_id, ref extension_name, version } => {
                let mut manager = self.manager.lock();
                let mut connection = manager.connections.get_mut(node_id).ok_or(Error::NotValidNode(node_id))?;
                connection.enqueue_negotiation_request(extension_name.clone(), version);
                io.update_registration(node_id)?;
            },
            HandlerMessage::SendExtensionMessage { node_id, ref extension_name, ref need_encryption, ref data } => {
                let mut manager = self.manager.lock();
                let mut connection = manager.connections.get_mut(node_id).ok_or(Error::NotValidNode(node_id))?;
                connection.enqueue_extension_message(extension_name.clone(), *need_encryption, data.clone());
                io.update_registration(node_id)?;
            },

            HandlerMessage::SetTimer { ref extension_name, timer_id, ms } => {
                let mut timer = self.timer.lock();
                match timer.insert(extension_name.clone(), timer_id, false) {
                    Ok(token) => {
                        io.register_timer(token, ms)?;
                        self.client.on_timer_set_allowed(extension_name, timer_id);
                    },
                    Err(TimerInfoError::DuplicatedTimerId) => {
                        self.client.on_timer_set_denied(extension_name, timer_id, ExtensionError::DuplicatedTimerId);
                    },
                    Err(TimerInfoError::NoSpace) => {
                        self.client.on_timer_set_denied(extension_name, timer_id, ExtensionError::NoMoreTimerToken);
                    },
                }
            },
            HandlerMessage::SetTimerOnce { ref extension_name, timer_id, ms } => {
                let mut timer = self.timer.lock();
                match timer.insert(extension_name.clone(), timer_id, true) {
                    Ok(token) => {
                        io.register_timer_once(token, ms)?;
                        self.client.on_timer_set_allowed(extension_name, timer_id);
                    },
                    Err(TimerInfoError::DuplicatedTimerId) => {
                        self.client.on_timer_set_denied(extension_name, timer_id, ExtensionError::DuplicatedTimerId);
                    },
                    Err(TimerInfoError::NoSpace) => {
                        self.client.on_timer_set_denied(extension_name, timer_id, ExtensionError::NoMoreTimerToken);
                    },
                }
            },
            HandlerMessage::ClearTimer { ref extension_name, timer_id } => {
                let mut timer = self.timer.lock();
                let token = timer.remove_by_info(extension_name.clone(), timer_id).expect("Unexpected timer id");
                io.clear_timer(token)?;
            },
        };
        Ok(())
    }

    fn stream_readable(&self, io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                loop {
                    let mut manager = self.manager.lock();
                    if let Some(token) = manager.accept(io)? {
                        if let Err(err) = io.register_stream(token) {
                            info!("Cannot register stream for accepted connection({:?}) : {:?}", token, err);
                        }
                    } else {
                        break;
                    }
                }
            },
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                loop {
                    let mut manager = self.manager.lock();
                    if let Some(mut connection) = manager.connections.get_mut(stream) {
                        if !connection.receive(&ExtensionChannel::new(&self.client, stream)) {
                            break
                        }
                    } else {
                        info!("readable event for unregistered stream({:?})", stream);
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
            ACCEPT_TOKEN => {},
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                loop {
                    let mut manager = self.manager.lock();
                    let mut connection = manager.connections.get_mut(stream).ok_or(Error::NotValidStream(stream))?;
                    if !connection.send()? {
                        return Ok(())
                    }
                }
            },
            _ => unimplemented!(),
        }
        Ok(())
    }

    fn register_stream(&self, stream: StreamToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) -> IoHandlerResult<()> {
        match stream {
            ACCEPT_TOKEN => {
                let manager = self.manager.lock();
                event_loop.register(&manager.listener, reg, Ready::readable() | Ready::writable(), PollOpt::edge())?;
                info!("TCP connection starts for {:?}", self.socket_address);
                Ok(())
            },
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let mut manager = self.manager.lock();
                let _ = manager.register_stream(stream, reg, event_loop);
                self.client.on_node_added(&stream);
                if !manager.is_inbound(stream) {
                    let mut connection = manager.connections.get_mut(stream).expect("Connection registered");
                    let nonce = connection.session().nonce().expect("Outbound connection must have nonce");
                    connection.enqueue_sync(nonce);
                }
                Ok(())
            }
            _ => {
                unreachable!();
            },
        }
    }

    fn update_stream(&self, stream: StreamToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) -> IoHandlerResult<()> {
        match stream {
            FIRST_CONNECTION_TOKEN...LAST_CONNECTION_TOKEN => {
                let mut manager = self.manager.lock();
                manager.update_stream(stream, reg, event_loop)?;
                Ok(())
            },
            _ => {
                unreachable!();
            },
        }
    }
}


pub trait AddressConverter: Send + Sync {
    fn node_id_to_address(&self, node_id: &NodeId) -> Option<SocketAddr>;
    fn address_to_node_id(&self, address: &SocketAddr) -> Option<NodeId>;
}

impl AddressConverter for Handler {
    fn node_id_to_address(&self, node_id: &NodeId) -> Option<SocketAddr> {
        let manager = self.manager.lock();
        manager.connections
            .get(*node_id)
            .map(|connection| connection.peer_addr().expect("Peer must exist").clone())
    }

    fn address_to_node_id(&self, socket_address: &SocketAddr) -> Option<NodeId> {
        let manager = self.manager.lock();
        manager.socket_to_node_id.get(&socket_address).map(|id| id.clone())
    }
}
