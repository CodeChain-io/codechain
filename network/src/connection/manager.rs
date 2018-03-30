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

use std::collections::HashSet;
use std::convert::From;
use std::io;
use std::sync::Arc;

use cio::{IoContext, IoHandler, IoManager, StreamToken};
use mio::deprecated::EventLoop;
use mio::net::{TcpListener, TcpStream};
use mio::{PollOpt, Ready, Token};
use parking_lot::Mutex;

use super::connection::Connection;
use super::super::Address;
use super::super::client::Client;
use super::super::extension::NodeId;
use super::super::session::{Session, SessionTable};
use super::limited_table::{Key as ConnectionToken, LimitedTable};

pub struct Manager {
    listener: TcpListener,
    connections: LimitedTable<Connection>,
    address_to_session: SessionTable,
    inbound_tokens: HashSet<ConnectionToken>,
}

const ACCEPT_TOKEN: usize = 0;
const FIRST_TOKEN: usize = 100;
const MAX_SESSIONS: usize = 32;
const LAST_TOKEN: usize = FIRST_TOKEN + MAX_SESSIONS;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    RegisterSession(Address, Session),

    RequestConnection(Address),

    SendExtensionMessage {
        node_id: NodeId,
        extension_name: String,
        need_encryption: bool,
        data: Vec<u8 >,
    },
}

impl Manager {
    pub fn listen(address: &Address) -> io::Result<Self> {
        Ok(Manager {
            listener: TcpListener::bind(address.socket())?,
            connections: LimitedTable::new(FIRST_TOKEN, MAX_SESSIONS),
            address_to_session: SessionTable::new(),
            inbound_tokens: HashSet::new(),
        })
    }

    fn register_token(&mut self, stream: TcpStream, address: &Address, is_inbound: bool) -> Option<ConnectionToken> {
        if let Some(session) = self.address_to_session.get(&address) {
            match Connection::new(stream, session.clone()) {
                Ok(connection) => {
                    if let Some(token) = self.connections.insert(connection) {
                        if is_inbound {
                            let _ = self.inbound_tokens.insert(token);
                        }
                        Some(token)
                    } else {
                        None
                    }
                },
                Err(err) => {
                    info!("Cannot create connection {:?}", err);
                    None
                },
            }
        } else {
            info!("There is no available session");
            None
        }
    }

    pub fn accept(&mut self, _io: &IoContext<HandlerMessage>) -> Option<ConnectionToken> {
        match self.listener.accept() {
            Ok((stream, address)) => {
                self.register_token(stream, &Address::from(address), true)
            },
            Err(e) => {
                if e.kind() != io::ErrorKind::WouldBlock {
                    info!("Cannot accept connection : {:?}", e);
                }
                None
            },
        }
    }

    pub fn connect(&mut self, address: &Address) -> Option<ConnectionToken> {
        match TcpStream::connect(address.socket()) {
            Ok(stream) => {
                self.register_token(stream, &address, false)
            },
            Err(err) => {
                info!("Cannot create connection to {:?} : {:?}", address, err);
                None
            },
        }
    }

    pub fn register_session(&mut self, address: Address, session: Session) {
        debug_assert!(session.is_ready());
        if self.address_to_session.contains_key(&address) {
            info!("Session registration is requested to the address which already has one");
            return
        }

        self.address_to_session.insert(address, session);
    }

    pub fn register_stream(&self, token: ConnectionToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) {
        self.connections.get(token).map(|connection| {
            if let Err(err) = event_loop.register(connection.stream(), reg, Ready::readable() | Ready::writable(), PollOpt::edge()) {
                info!("Cannot register TCP stream {:?}", err);
            } else {
                info!("Register TCP stream on {}", token)
            }
        });
    }

    pub fn update_stream(&self, token: ConnectionToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) {
        self.connections.get(token).map(|connection| {
            if let Err(err) = event_loop.reregister(connection.stream(), reg, Ready::readable() | Ready::writable(), PollOpt::edge()) {
                info!("Cannot register TCP stream {:?}", err);
            } else {
                info!("Register TCP stream on {}", token)
            }
        });
    }

    pub fn is_inbound(&self, token: ConnectionToken) -> bool {
        self.inbound_tokens.contains(&token)
    }
}

pub struct Handler {
    address: Address,
    manager: Mutex<Manager>,
    client: Arc<Client>,
}

impl Handler {
    pub fn new(address: Address, client: Arc<Client>) -> Self {
        let manager = Mutex::new(Manager::listen(&address).expect("Cannot listen TCP port"));
        Self {
            address,
            manager,
            client,
        }
    }
}

impl IoHandler<HandlerMessage> for Handler {
    fn initialize(&self, io: &IoContext<HandlerMessage>) {
        if let Err(err) = io.register_stream(ACCEPT_TOKEN) {
            info!("Cannot register tcp stream {:?}", err);
        }
    }

    fn message(&self, io: &IoContext<HandlerMessage>, message: &HandlerMessage) {
        match *message {
            HandlerMessage::RegisterSession(ref address, ref session) => {
                debug_assert!(session.is_ready());
                let mut manager = self.manager.lock();
                info!("Register session {:?}", session);
                manager.register_session(address.clone(), session.clone());
            },
            HandlerMessage::RequestConnection(ref address) => {
                let mut manager = self.manager.lock();
                info!("Connecting to {:?}", address);
                if let Some(token) = manager.connect(&address) {
                    if let Err(err) = io.register_stream(token) {
                        info!("Cannot register stream for token {:?} : {:?}", token, err);
                    }
                } else {
                    info!("There are no available tokens");
                }
            },
            HandlerMessage::SendExtensionMessage { node_id, ref extension_name, ref need_encryption, ref data } => {
                let mut manager = self.manager.lock();
                if let Some(mut connection) = manager.connections.get_mut(node_id) {
                    connection.enqueue_extension_message(extension_name.clone(), *need_encryption, data.clone());
                    let _ = io.update_registration(node_id);
                    info!("Send extension message to node({})", node_id);
                } else {
                    info!("{} is not a valid node id", node_id);
                }
            },
        };
    }

    fn stream_readable(&self, io: &IoContext<HandlerMessage>, stream: StreamToken) {
        match stream {
            ACCEPT_TOKEN => {
                loop {
                    let mut manager = self.manager.lock();
                    if let Some(token) = manager.accept(io) {
                        if let Err(err) = io.register_stream(token) {
                            info!("Cannot register stream for accepted connection({:?}) : {:?}", token, err);
                        }
                    } else {
                        break;
                    }
                }
            },
            FIRST_TOKEN...LAST_TOKEN => {
                loop {
                    let mut manager = self.manager.lock();
                    if let Some(mut connection) = manager.connections.get_mut(stream) {
                        if !connection.receive() {
                            break
                        }
                    } else {
                        info!("readable event for unregistered stream({:?})", stream);
                    }
                }
                let _ = io.update_registration(stream);
            }
            _ => unimplemented!(),
        }
    }

    fn stream_writable(&self, _io: &IoContext<HandlerMessage>, stream: StreamToken) {
        match stream {
            ACCEPT_TOKEN => {},
            FIRST_TOKEN...LAST_TOKEN => {
                loop {
                    let mut manager = self.manager.lock();
                    if let Some(mut connection) = manager.connections.get_mut(stream) {
                        match connection.send() {
                            Ok(true) => {},
                            Ok(false) => break,
                            Err(err) => {
                                info!("Error in sending a message to {:?} : {:?}", connection.stream().peer_addr(), err);
                            }
                        }
                    } else {
                        info!("{} is not a valid token", stream);
                        break
                    }
                }
            },
            _ => unimplemented!(),
        }
    }

    fn register_stream(&self, stream: StreamToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) {
        match stream {
            ACCEPT_TOKEN => {
                let manager = self.manager.lock();
                if let Err(err) = event_loop.register(&manager.listener, reg, Ready::readable() | Ready::writable(), PollOpt::edge()) {
                    info!("Cannot register tcp manager {:?}", err);
                }
                info!("TCP connection starts for {:?}", self.address);
            },
            FIRST_TOKEN...LAST_TOKEN => {
                let mut manager = self.manager.lock();
                manager.register_stream(stream, reg, event_loop);
                self.client.on_node_added(&stream);
                if !manager.is_inbound(stream) {
                    if let Some(connection) = manager.connections.get_mut(stream) {
                        connection.enqueue_sync();
                    }
                }
            }
            _ => {
                unreachable!();
            },
        }
    }

    fn update_stream(&self, stream: StreamToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) {
        match stream {
            FIRST_TOKEN...LAST_TOKEN => {
                let mut manager = self.manager.lock();
                manager.update_stream(stream, reg, event_loop);
            },
            _ => {
                unreachable!();
            },
        }
    }
}
