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
use super::super::Address;

pub struct Handshake {
    socket: UdpSocket,
}

#[derive(Debug)]
enum HandshakeError {
    IoError(io::Error),
    RlpError(DecoderError),
    SendError(HandshakeMessage, usize),
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &HandshakeError::IoError(ref err) => write!(f, "IoError {}", err),
            &HandshakeError::RlpError(ref err) => write!(f, "RlpError {}", err),
            &HandshakeError::SendError(ref msg, unsent) => write!(f, "SendError {} bytesa of {:?} are not sent", unsent, msg),
        }
    }
}

impl error::Error for HandshakeError {
    fn description(&self) -> &str {
        match self {
            &HandshakeError::IoError(ref err) => err.description(),
            &HandshakeError::RlpError(ref err) => err.description(),
            &HandshakeError::SendError(_, _) => "Unsent data",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &HandshakeError::IoError(ref err) => Some(err),
            &HandshakeError::RlpError(ref err) => Some(err),
            &HandshakeError::SendError(_, _) => None,
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

const MAX_HANDSHAKE_PACKET_SIZE: usize = 1024;

pub type Nonce = u32;

impl Handshake {
    fn bind(address: &Address) -> Result<Self, HandshakeError> {
        let socket = address.socket();
        let socket = UdpSocket::bind(socket)?;
        let _ = socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
        })
    }

    fn receive(&self) -> Result<Option<(HandshakeMessage, Address)>, HandshakeError> {
        let mut buf: [u8; MAX_HANDSHAKE_PACKET_SIZE] = [0; MAX_HANDSHAKE_PACKET_SIZE];
        match self.socket.recv_from(&mut buf) {
            Ok((_size, addr)) => {
                let rlp = UntrustedRlp::new(&buf);
                let message = Decodable::decode(&rlp)?;
                info!("Handshake {:?} received from {:?}", message, addr);
                Ok(Some((message, Address::from(addr))))
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(HandshakeError::from(e)),
        }
    }

    fn send_to(&self, message: &HandshakeMessage, target: &Address) -> Result<(), HandshakeError> {
        let bytes = message.rlp_bytes();
        let length_to_send = bytes.len();
        debug_assert!(length_to_send <= MAX_HANDSHAKE_PACKET_SIZE);
        let sent_size = self.socket.send_to(&bytes, target.socket().clone())?;
        if sent_size != length_to_send {
            return Err(HandshakeError::SendError(message.clone(), length_to_send - sent_size))
        }
        info!("Handshake {:?} sent to {:?}", message, target);
        Ok(())
    }

    fn send_ping_to(&self, target: &Address, nonce: Nonce) -> Result<(), HandshakeError> {
        self.send_to(&HandshakeMessage::Ping(nonce), target)
    }

    fn on_packet(&self, message: &HandshakeMessage, from: &Address) {
        match message {
            &HandshakeMessage::Ping(nonce) => {
                let pong = HandshakeMessage::Pong(nonce + 1);
                if let Ok(_) = self.send_to(&pong, &from) {
                } else {
                    info!("Cannot send {:?} to {:?}", pong, from);
                }
            },
            &HandshakeMessage::Pong(_) => {
            },
        }
    }

    fn nonce() -> Nonce {
        const MIN_NONCE: u32 = 1000;
        const MAX_NONCE: u32 = 100000;

        let mut range = Range::new(MIN_NONCE, MAX_NONCE);
        let mut rng = rand::thread_rng();
        range.sample(&mut rng)
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
                    if let Some(handshake) = self.handshake.lock().as_ref() {
                        match handshake.receive() {
                            Ok(None) => {
                                break;
                            },
                            Ok(Some((msg, address))) => {
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

                if let Some(handshake) = self.handshake.lock().as_ref() {
                    for address in queue.iter() {
                        connect_to(&handshake, &address);
                    }
                }
            },
            &HandlerMessage::ConnectTo(ref address) => {
                if let Some(handshake) = self.handshake.lock().as_ref() {
                    connect_to(&handshake, &address);
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

fn connect_to(handshake: &Handshake, address: &Address) {
    let nonce = Handshake::nonce();
    if let Err(err) = handshake.send_ping_to(&address, nonce) {
        info!("Cannot ping to {:?} since {}", &address, err);
    } else {
        info!("Ping to {:?}", &address);
    }
}
