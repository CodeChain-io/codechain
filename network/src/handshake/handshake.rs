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

use std::collections::VecDeque;
use std::error;
use std::fmt;
use std::io;
use std::result::Result;

use cio::{IoContext, IoHandler, IoManager, StreamToken};
use mio::{PollOpt, Ready, Token};
use mio::deprecated::EventLoop;
use mio::net::UdpSocket;
use parking_lot::Mutex;
use rlp::{UntrustedRlp, Encodable, Decodable, DecoderError};

use super::HandshakeMessage;
use super::super::session::{Nonce, Session, SessionError, SessionTable, SharedSecret};
use super::super::Address;


pub struct Handshake {
    socket: UdpSocket,
    table: SessionTable,
}

#[derive(Debug)]
enum HandshakeError {
    IoError(io::Error),
    RlpError(DecoderError),
    SendError(HandshakeMessage, usize),
    SessionError(SessionError),
    NoSession,
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &HandshakeError::IoError(ref err) => write!(f, "IoError {}", err),
            &HandshakeError::RlpError(ref err) => write!(f, "RlpError {}", err),
            &HandshakeError::SendError(ref msg, unsent) => write!(f, "SendError {} bytesa of {:?} are not sent", unsent, msg),
            &HandshakeError::SessionError(ref err) => write!(f, "SessionErrorError {}", err),
            &HandshakeError::NoSession => write!(f, "NoSession"),
        }
    }
}

impl error::Error for HandshakeError {
    fn description(&self) -> &str {
        match self {
            &HandshakeError::IoError(ref err) => err.description(),
            &HandshakeError::RlpError(ref err) => err.description(),
            &HandshakeError::SendError(_, _) => "Unsent data",
            &HandshakeError::SessionError(ref err) => err.description(),
            &HandshakeError::NoSession => "No session",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &HandshakeError::IoError(ref err) => Some(err),
            &HandshakeError::RlpError(ref err) => Some(err),
            &HandshakeError::SendError(_, _) => None,
            &HandshakeError::SessionError(ref err) => Some(err),
            &HandshakeError::NoSession => None,
        }
    }
}

impl From<io::Error> for HandshakeError {
    fn from(err: io::Error) -> HandshakeError {
        HandshakeError::IoError(err)
    }
}

impl From<DecoderError> for HandshakeError {
    fn from(err: DecoderError) -> HandshakeError {
        HandshakeError::RlpError(err)
    }
}

impl From<SessionError> for HandshakeError {
    fn from(err: SessionError) -> HandshakeError {
        HandshakeError::SessionError(err)
    }
}
const MAX_HANDSHAKE_PACKET_SIZE: usize = 1024;

impl Handshake {
    fn bind(address: &Address) -> Result<Self, HandshakeError> {
        let socket = address.socket();
        let socket = UdpSocket::bind(socket)?;
        Ok(Self {
            socket,
            table: SessionTable::new(),
        })
    }

    fn receive(&self) -> Result<Option<(HandshakeMessage, Address)>, HandshakeError> {
        let mut buf: [u8; MAX_HANDSHAKE_PACKET_SIZE] = [0; MAX_HANDSHAKE_PACKET_SIZE];
        match self.socket.recv_from(&mut buf) {
            Ok((received_size, address)) => {
                let address = Address::from(address);
                let session = self.table.get(&address).ok_or(HandshakeError::NoSession)?;

                let encrypted_bytes = &buf[0..received_size];

                let unencrypted_bytes = session.decrypt(&encrypted_bytes)?;

                let rlp = UntrustedRlp::new(&unencrypted_bytes);
                let message = Decodable::decode(&rlp)?;
                info!("Handshake {:?} received from {:?}", message, address);
                Ok(Some((message, Address::from(address))))
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(HandshakeError::from(e)),
        }
    }

    fn send_to(&self, message: &HandshakeMessage, target: &Address) -> Result<(), HandshakeError> {
        let session = self.table.get(&target).ok_or(HandshakeError::NoSession)?;

        let unencrypted_bytes = message.rlp_bytes();
        let encrypted_bytes = session.encrypt(&unencrypted_bytes)?;

        let length_to_send = encrypted_bytes.len();

        let sent_size = self.socket.send_to(&encrypted_bytes, target.socket())?;
        if sent_size != length_to_send {
            return Err(HandshakeError::SendError(message.clone(), length_to_send - sent_size))
        }
        info!("Handshake {:?} sent to {:?}", message, target);
        Ok(())
    }

    fn send_ping_to(&mut self, target: &Address, nonce: Nonce) -> Result<(), HandshakeError> {
        if let Some(session) = self.table.get_mut(&target) {
            session.set_ready(nonce);
        } else {
            return Err(HandshakeError::NoSession)
        }
        self.send_to(&HandshakeMessage::ConnectionRequest(nonce), target)
    }

    fn on_packet(&mut self, message: &HandshakeMessage, from: &Address) {
        match message {
            &HandshakeMessage::ConnectionRequest(nonce) => {
                let nonce = {
                    if let Some(session) = self.table.get(from) {
                        if session.is_ready() {
                            info!("A nonce already exists");
                        }
                        nonce // FIXME: must return nonce + 1
                    } else {
                        info!("There is no shared secret");
                        return;
                    }
                };

                let pong = HandshakeMessage::ConnectionAllowed(nonce);
                if let Ok(_) = self.send_to(&pong, &from) {
                } else {
                    info!("Cannot send {:?} to {:?}", pong, from);
                }
            },
            &HandshakeMessage::ConnectionAllowed(nonce) => {
                if let Some(ref session) = self.table.get(from) {
                    if !session.is_ready() {
                        info!("A nonce doesn't exists");
                        return;
                    } else if session.is_expected_nonce(&nonce) {
                        // FIXME: Create TCP connection
                    } else {
                        info!("Nonce({}) is not expected", nonce);
                        return;
                    }
                } else {
                    info!("There is no shared secret");
                    return;
                }
            },
            &HandshakeMessage::ConnectionDenied(ref reason) => {
                info!("Connection to {:?} refused(reason: {}", from, reason);
            },
        }
    }

    fn nonce() -> Nonce {
        10000 // FIXME
    }
}

struct Internal {
    handshake: Option<Handshake>,
    connect_queue: VecDeque<Address>,
}

pub struct Handler {
    address: Address,
    internal: Mutex<Internal>,
}

impl Handler {
    pub fn new(address: Address) -> Self {
        Self {
            address,
            internal: Mutex::new(Internal {
                handshake: None,
                connect_queue: VecDeque::new(),
            })
        }
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    Bind,
    ConnectTo(Address),
}

const RECV_TOKEN: usize = 0;

impl IoHandler<HandlerMessage> for Handler {
    fn initialize(&self, io: &IoContext<HandlerMessage>) {
        io.message(HandlerMessage::Bind).expect("Cannot run UDP io service");
    }

    fn message(&self, io: &IoContext<HandlerMessage>, message: &HandlerMessage) {
        match message {
            &HandlerMessage::Bind => {
                info!("Handshake service bind to {:?}", &self.address);
                let ref mut internal = self.internal.lock();
                debug_assert!(internal.handshake.is_none());

                let mut handshake = Handshake::bind(&self.address).expect("Cannot bind UDP port");
                if let Err(err) = io.register_stream(RECV_TOKEN) {
                    info!("Cannot register udp stream {:?}", err);
                }

                {
                    let ref mut queue = internal.connect_queue;
                    for address in queue.iter() {
                        handshake.table.insert(address.clone(), Session::new(SharedSecret::zero())); // FIXME: Remove it
                        connect_to(&mut handshake, &address);
                    }
                    queue.clear();
                }

                internal.handshake = Some(handshake);
            },
            &HandlerMessage::ConnectTo(ref address) => {
                let ref mut internal = self.internal.lock();
                if let Some(mut handshake) = internal.handshake.as_mut() {
                    connect_to(&mut handshake, &address);
                    return;
                }
                let ref mut queue = internal.connect_queue;
                queue.push_back(address.clone());
            },
        };
    }

    fn stream_hup(&self, _io: &IoContext<HandlerMessage>, _stream: StreamToken) {
        info!("handshake server closed");
        let ref mut internal = self.internal.lock();
        internal.handshake = None;
    }

    fn stream_readable(&self, _io: &IoContext<HandlerMessage>, stream: StreamToken) {
        match stream {
            RECV_TOKEN => {
                loop {
                    if let Some(mut handshake) = self.internal.lock().handshake.as_mut() {
                        match handshake.receive() {
                            Ok(None) => {
                                break;
                            },
                            Ok(Some((msg, address))) => {
                                info!("{:?} from {:?}", msg, address);
                                handshake.on_packet(&msg, &address);
                            },
                            Err(err) => {
                                info!("handshake receive error {}", err);
                            },
                        };
                    };
                };
            },
            _ => {
                info!("Unknown stream token {}", stream);
            },
        };
    }

    fn register_stream(&self, stream: StreamToken, _reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) {
        match stream {
            RECV_TOKEN => {
                let mut internal = self.internal.lock();
                if let Some(mut handshake) = internal.handshake.as_mut() {
                    if let Err(err) = event_loop.register(&handshake.socket, Token(RECV_TOKEN), Ready::all(), PollOpt::edge()) {
                        info!("Cannot register udp socket {:?}", err);
                    }
                } else {
                    panic!("Error in creating UDP socket");
                }
            },
            _ => {
                info!("Unexpected stream registration {}", stream);
            }
        }
    }

    fn deregister_stream(&self, _stream: StreamToken, _event_loop: &mut EventLoop<IoManager<HandlerMessage>>) {
    }
}

fn connect_to(handshake: &mut Handshake, address: &Address) {
    let nonce = Handshake::nonce();
    if let Err(err) = handshake.send_ping_to(&address, nonce) {
        info!("Cannot ping to {:?} since {}", &address, err);
    } else {
        info!("Ping to {:?}", &address);
    }
}
