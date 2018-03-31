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

use cio::{IoChannel, IoContext, IoHandler, IoManager, IoHandlerResult, StreamToken};
use mio::{PollOpt, Ready, Token};
use mio::deprecated::EventLoop;
use mio::net::UdpSocket;
use parking_lot::Mutex;
use rlp::{UntrustedRlp, Encodable, Decodable, DecoderError};

use super::HandshakeMessage;
use super::super::session::{Nonce, Session, SessionError, SessionTable, SharedSecret};
use super::super::Address;
use super::super::connection;


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
    UnexpectedNonce(Nonce),
    SessionAlreadyExists,
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &HandshakeError::IoError(ref err) => write!(f, "IoError {}", err),
            &HandshakeError::RlpError(ref err) => write!(f, "RlpError {}", err),
            &HandshakeError::SendError(ref msg, unsent) => write!(f, "SendError {} bytes of {:?} are not sent", unsent, msg),
            &HandshakeError::SessionError(ref err) => write!(f, "SessionError {}", err),
            &HandshakeError::NoSession => write!(f, "NoSession"),
            &HandshakeError::UnexpectedNonce(ref nonce) => write!(f, "{:?} is an unexpected nonce", nonce),
            &HandshakeError::SessionAlreadyExists => write!(f, "Session already exists"),
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
            &HandshakeError::UnexpectedNonce(_) => "Unexpected nonce",
            &HandshakeError::SessionAlreadyExists => "Session already exists",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &HandshakeError::IoError(ref err) => Some(err),
            &HandshakeError::RlpError(ref err) => Some(err),
            &HandshakeError::SendError(_, _) => None,
            &HandshakeError::SessionError(ref err) => Some(err),
            &HandshakeError::NoSession => None,
            &HandshakeError::UnexpectedNonce(_) => None,
            &HandshakeError::SessionAlreadyExists => None,
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
                if self.table.get(&address).is_none() {
                    return Err(HandshakeError::NoSession)
                }

                let raw_bytes = &buf[0..received_size];
                let rlp = UntrustedRlp::new(&raw_bytes);
                let message = Decodable::decode(&rlp)?;

                info!("Handshake {:?} received from {:?}", message, address);
                Ok(Some((message, Address::from(address))))
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(HandshakeError::from(e)),
        }
    }

    fn send_to(&self, message: &HandshakeMessage, target: &Address) -> Result<(), HandshakeError> {
        if self.table.get(&target).is_none() {
            return Err(HandshakeError::NoSession)
        }

        let unencrypted_bytes = message.rlp_bytes();

        let length_to_send = unencrypted_bytes.len();

        let sent_size = self.socket.send_to(&unencrypted_bytes, target.socket())?;
        if sent_size != length_to_send {
            return Err(HandshakeError::SendError(message.clone(), length_to_send - sent_size))
        }
        info!("Handshake {:?} sent to {:?}", message, target);
        Ok(())
    }

    fn send_ping_to(&mut self, target: &Address, nonce: Nonce) -> Result<(), HandshakeError> {
        let nonce = {
            let mut session = self.table.get_mut(&target).ok_or(HandshakeError::NoSession)?;
            session.set_ready(nonce);
            encode_and_encrypt_nonce(&session, nonce)?
        };
        self.send_to(&HandshakeMessage::connection_request(0, nonce), target) // FIXME: seq
    }

    fn on_packet(&mut self, message: &HandshakeMessage, from: &Address, extension: &IoChannel<connection::HandlerMessage>) -> IoHandlerResult<()> {
        match message {
            &HandshakeMessage::ConnectionRequest(_, _, ref nonce) => {
                let encrypted_bytes = {
                    let session = self.table.get(from).ok_or(HandshakeError::NoSession)?;
                    if session.is_ready() {
                        info!("A nonce already exists");
                    }
                    let nonce = decrypt_and_decode_nonce(&session, &nonce)?;

                    // FIXME: let nonce = f(nonce)

                    extension.send(connection::HandlerMessage::RegisterSession(from.clone(), session.clone()))?;

                    encode_and_encrypt_nonce(&session, nonce)?
                };

                let pong = HandshakeMessage::connection_allowed(0, encrypted_bytes); // FIXME: seq
                self.send_to(&pong, &from)?;
                Ok(())
            },
            &HandshakeMessage::ConnectionAllowed(_, _, ref nonce) => {
                let session = self.table.get(from).ok_or(HandshakeError::NoSession)?;
                if !session.is_ready() {
                    return Err(From::from(SessionError::NotReady))
                }
                let nonce = decrypt_and_decode_nonce(&session, &nonce)?;

                if !session.is_expected_nonce(&nonce) {
                    return Err(From::from(HandshakeError::UnexpectedNonce(nonce)))
                }
                extension.send(connection::HandlerMessage::RequestConnection(from.clone(), session.clone()))?;
                Ok(())
            },
            &HandshakeMessage::ConnectionDenied(_, _, ref reason) => {
                info!("Connection to {:?} refused(reason: {}", from, reason);
                Ok(())
            },
        }
    }

    fn nonce() -> Nonce {
        10000 // FIXME
    }
}

fn encode_and_encrypt_nonce(session: &Session, nonce: Nonce) -> Result<Vec<u8>, HandshakeError> {
    let unencrypted_bytes = nonce.rlp_bytes();
    Ok(session.encrypt(&unencrypted_bytes)?)
}

fn decrypt_and_decode_nonce(session: &Session, encrypted_bytes: &Vec<u8>) -> Result<Nonce, HandshakeError> {
    let unencrypted_bytes = session.decrypt(&encrypted_bytes)?;
    let rlp = UntrustedRlp::new(&unencrypted_bytes);
    Ok(Decodable::decode(&rlp)?)
}

struct Internal {
    handshake: Handshake,
    connect_queue: VecDeque<Address>,
}

pub struct Handler {
    address: Address,
    internal: Mutex<Internal>,
    extension: IoChannel<connection::HandlerMessage>,
}

impl Handler {
    pub fn new(address: Address, extension: IoChannel<connection::HandlerMessage>) -> Self {
        let handshake = Handshake::bind(&address).expect("Cannot bind UDP port");
        Self {
            address,
            internal: Mutex::new(Internal {
                handshake,
                connect_queue: VecDeque::new(),
            }),
            extension,
        }
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    ConnectTo(Address),
}

const RECV_TOKEN: usize = 0;

impl IoHandler<HandlerMessage> for Handler {
    fn initialize(&self, io: &IoContext<HandlerMessage>) -> IoHandlerResult<()> {
        io.register_stream(RECV_TOKEN)?;
        Ok(())
    }

    fn message(&self, io: &IoContext<HandlerMessage>, message: &HandlerMessage) -> IoHandlerResult<()> {
        match message {
            &HandlerMessage::ConnectTo(ref address) => {
                let mut internal = self.internal.lock();
                {
                    let ref mut queue = internal.connect_queue;
                    queue.push_back(address.clone());
                }
                {
                    let ref mut handshake = internal.handshake;
                    handshake.table.insert(address.clone(), Session::new(SharedSecret::zero())); // FIXME: Remove it
                }
            },
        };
        Ok(())
    }

    fn stream_hup(&self, _io: &IoContext<HandlerMessage>, _stream: StreamToken) -> IoHandlerResult<()> {
        info!("handshake server closed");
        Ok(())
    }

    fn stream_readable(&self, _io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            RECV_TOKEN => {
                loop {
                    let mut internal = self.internal.lock();
                    let ref mut handshake = internal.handshake;
                    match handshake.receive() {
                        Ok(None) => {
                            break;
                        },
                        Ok(Some((msg, address))) => {
                            info!("{:?} from {:?}", msg, address);
                            let _ = handshake.on_packet(&msg, &address, &self.extension);
                        },
                        Err(err) => {
                            info!("handshake receive error {}", err);
                        },
                    };
                };
            },
            _ => {
                info!("Unknown stream token {}", stream);
            },
        };
        Ok(())
    }

    fn stream_writable(&self, _io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        loop {
            let mut internal = self.internal.lock();
            if let Some(ref address) = internal.connect_queue.pop_front().as_ref() {
                let ref mut handshake = internal.handshake;
                connect_to(handshake, &address);
            } else {
                break
            }
        }
        Ok(())
    }

    fn register_stream(&self, stream: StreamToken, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) -> IoHandlerResult<()> {
        match stream {
            RECV_TOKEN => {
                let mut internal = self.internal.lock();
                let ref mut handshake = internal.handshake;
                event_loop.register(&handshake.socket, reg, Ready::readable() | Ready::writable(), PollOpt::edge())?;
            },
            _ => {
                unreachable!();
            }
        }
        Ok(())
    }
}

fn connect_to(handshake: &mut Handshake, address: &Address) -> IoHandlerResult<()> {
    let nonce = Handshake::nonce();
    handshake.send_ping_to(&address, nonce)?;
    Ok(())
}
