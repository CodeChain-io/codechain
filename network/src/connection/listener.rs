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

use cio::{IoContext, IoHandler, IoManager, StreamToken};
use mio::deprecated::EventLoop;
use mio::net::{TcpListener, TcpStream};
use mio::{PollOpt, Ready, Token};
use parking_lot::Mutex;

use super::connection::Connection;
use super::super::Address;
use super::super::session::{Session, SessionTable};
use super::limited_table::{Key as ConnectionToken, LimitedTable};

pub struct Listener {
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
}

impl Listener {
    pub fn listen(address: &Address) -> io::Result<Self> {
        Ok(Listener {
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
        assert!(session.is_ready());
        if self.address_to_session.contains_key(&address) {
            info!("Session registration is requested to the address which already has one");
            return
        }

        self.address_to_session.insert(address, session);
    }

    pub fn register_stream(&self, token: ConnectionToken, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) {
        self.connections.get(token).map(|connection| {
            if let Err(err) = event_loop.register(connection.stream(), Token(token), Ready::readable() | Ready::writable(), PollOpt::edge()) {
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
    listener: Mutex<Listener>,
}

impl Handler {
    pub fn new(address: Address) -> Self {
        let listener = Mutex::new(Listener::listen(&address).expect("Cannot listen TCP port"));
        Self {
            address,
            listener,
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
                assert!(session.is_ready());
                let mut listener = self.listener.lock();
                info!("Register session {:?}", session);
                listener.register_session(address.clone(), session.clone());
            },
            HandlerMessage::RequestConnection(ref address) => {
                let mut listener = self.listener.lock();
                info!("Connecting to {:?}", address);
                if let Some(token) = listener.connect(&address) {
                    if let Err(err) = io.register_stream(token) {
                        info!("Cannot register stream for token {:?} : {:?}", token, err);
                    }
                } else {
                    info!("There are no available tokens");
                }
            },
        };
    }

    fn stream_readable(&self, io: &IoContext<HandlerMessage>, stream: StreamToken) {
        match stream {
            ACCEPT_TOKEN => {
                loop {
                    let mut listener = self.listener.lock();
                    if let Some(token) = listener.accept(io) {
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
                    let mut listener = self.listener.lock();
                    if let Some(mut connection) = listener.connections.get_mut(stream) {
                        if !connection.receive() {
                            break
                        }
                    } else {
                        info!("readable event for unregistered stream({:?})", stream);
                    }
                }
            }
            _ => unimplemented!(),
        }
    }

    fn stream_writable(&self, _io: &IoContext<HandlerMessage>, stream: StreamToken) {
        match stream {
            ACCEPT_TOKEN => {},
            FIRST_TOKEN...LAST_TOKEN => {
                loop {
                    let mut listener = self.listener.lock();
                    if let Some(mut connection) = listener.connections.get_mut(stream) {
                    }
                }
            },
            _ => unimplemented!(),
        }
    }

    fn register_stream(&self, stream: StreamToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) {
        match stream {
            ACCEPT_TOKEN => {
                let listener = self.listener.lock();
                if let Err(err) = event_loop.register(&listener.listener, Token(ACCEPT_TOKEN), Ready::readable() | Ready::writable(), PollOpt::edge()) {
                    info!("Cannot register tcp listener {:?}", err);
                }
                info!("TCP connection starts for {:?}", self.address);
            },
            FIRST_TOKEN...LAST_TOKEN => {
                let mut listener = self.listener.lock();
                listener.register_stream(stream, event_loop);
            }
            _ => {
                unreachable!();
            },
        }
    }
}
