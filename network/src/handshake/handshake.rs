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
use std::net::UdpSocket;
use std::result::Result;

use cio::{ IoContext, IoHandler, TimerToken, StreamToken };
use parking_lot::Mutex;
use rand::distributions::{ Range, Sample };
use rand;
use rlp::{ UntrustedRlp, Encodable, Decodable, DecoderError };

use super::HandshakeMessage;
use super::super::session::{ Nonce, Session, SessionError, SessionTable, SharedSecret };
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
        let _ = socket.set_nonblocking(true)?;
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

                let session = match self.table.get(&address) {
                    Some(session) => {
                        session
                    },
                    None => {
                        return Err(HandshakeError::NoSession)
                    },
                };

                let encrypted_bytes = &buf[0..received_size];

                let unencrypted_bytes = session.decrypt(&encrypted_bytes)?;

                let rlp = UntrustedRlp::new(&unencrypted_bytes);
                let message = Decodable::decode(&rlp)?;
                Ok(Some((message, Address::from(address))))
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(HandshakeError::from(e)),
        }
    }

    fn send_to(&self, message: &HandshakeMessage, target: &Address, key: &SharedSecret) -> Result<(), HandshakeError> {
        let session = match self.table.get(&target) {
            Some(session) => session,
            None => return Err(HandshakeError::NoSession),
        };

        let unencrypted_bytes = message.rlp_bytes();
        let encrypted_bytes = session.encrypt(&unencrypted_bytes)?;

        let length_to_send = encrypted_bytes.len();

        let sent_size = self.socket.send_to(&encrypted_bytes, target.socket().clone())?;
        if sent_size != length_to_send {
            return Err(HandshakeError::SendError(message.clone(), length_to_send - sent_size))
        }
        info!("Handshake {:?} sent to {:?}", message, target);
        Ok(())
    }

    fn send_ping_to(&mut self, target: &Address, nonce: Nonce) -> Result<(), HandshakeError> {
        let secret = if let Some(session) = self.table.get_mut(&target) {
            session.set_ready(nonce);
            Ok(session.secret().clone())
        } else {
            Err(HandshakeError::NoSession)
        };
        self.send_to(&HandshakeMessage::ConnectionRequest(nonce), target, &secret?)
    }

    fn on_packet(&mut self, message: &HandshakeMessage, from: &Address) {
        match message {
            &HandshakeMessage::ConnectionRequest(nonce) => {
                let (nonce, secret) = {
                    if let Some(session) = self.table.get(from) {
                        if session.is_ready() {
                            info!("A nonce already exists");
                        }
                        (nonce, session.secret().clone()) // FIXME: must return nonce + 1
                    } else {
                        info!("There is no shared secret");
                        return;
                    }
                };

                let pong = HandshakeMessage::ConnectionAllowed(nonce);
                if let Ok(_) = self.send_to(&pong, &from, &secret) {
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
                        // Fixme: Connect TCP connection
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

pub struct Handler {
    address: Address,
    handshake: Mutex<Option<Handshake>>,
    connect_queue: Mutex<VecDeque<Address>>,
}

impl Handler {
    pub fn new(address: Address) -> Self {
        Self {
            address,
            handshake: Mutex::new(None),
            connect_queue: Mutex::new(VecDeque::new()),
        }
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    Bind,
    ConnectTo(Address),
}

const RECV_TOKEN: usize = 0;
const RECV_MS: u64 = 1000;

impl IoHandler<HandlerMessage> for Handler {
    fn initialize(&self, io: &IoContext<HandlerMessage>) {
        io.message(HandlerMessage::Bind).expect("Cannot run UDP io service");
    }

    fn timeout(&self, _io: &IoContext<HandlerMessage>, token: TimerToken) {
        match token {
            RECV_TOKEN => {
                loop {
                    if let Some(mut handshake) = self.handshake.lock().as_mut() {
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
                info!("Unknown timer token {}", token);
            },
        };
    }

    fn message(&self, io: &IoContext<HandlerMessage>, message: &HandlerMessage) {
        match message {
            &HandlerMessage::Bind => {
                info!("Handshake service bind to {:?}", &self.address);
                let handshake = Handshake::bind(&self.address).expect("Cannot bind UDP port");
                *self.handshake.lock() = Some(handshake);
                let _ = io.register_timer(RECV_TOKEN, RECV_MS);

                let ref mut queue = self.connect_queue.lock();

                if let Some(mut handshake) = self.handshake.lock().as_mut() {
                    for address in queue.iter() {
                        handshake.table.insert(address.clone(), Session::new(SharedSecret::zero())); // FIXME: Remove it
                        connect_to(&mut handshake, &address);
                    }
                }
            },
            &HandlerMessage::ConnectTo(ref address) => {
                if let Some(mut handshake) = self.handshake.lock().as_mut() {
                    connect_to(&mut handshake, &address);
                } else {
                    let ref mut queue = self.connect_queue.lock();
                    queue.push_back(address.clone());
                }
            },
        };
    }

    fn stream_hup(&self, _io: &IoContext<HandlerMessage>, _stream: StreamToken) {
        info!("handshake server closed");
        *self.handshake.lock() = None;
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
