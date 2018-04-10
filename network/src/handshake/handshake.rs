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

use std::collections::{HashMap, VecDeque};
use std::error;
use std::fmt;
use std::io;
use std::result::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ccrypto::aes::SymmetricCipherError;
use cio::{IoChannel, IoContext, IoError as CIoError, IoHandler, IoHandlerResult, IoManager, StreamToken};
use ckeys::{exchange, Error as KeysError, Generator, Private, Random};
use ctypes::Secret;
use mio::deprecated::EventLoop;
use mio::net::UdpSocket;
use mio::{PollOpt, Ready, Token};
use parking_lot::{Mutex, RwLock};
use rand::{OsRng, Rng};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};

use super::super::connection;
use super::super::session::{Nonce, Session};
use super::super::{DiscoveryApi, SocketAddr};
use super::{HandshakeMessage, HandshakeMessageBody};


pub struct Handshake {
    socket: UdpSocket,
    secrets: HashMap<SocketAddr, Secret>,
    nonces: HashMap<SocketAddr, Nonce>,
    temporary_nonces: HashMap<SocketAddr, Nonce>,
    requested: HashMap<SocketAddr, Private>,
    seq_counter: AtomicUsize,
}

#[derive(Debug)]
enum HandshakeError {
    IoError(io::Error),
    CIoError(CIoError),
    RlpError(DecoderError),
    SendError(HandshakeMessage, usize),
    SymmetricCipherError(SymmetricCipherError),
    NoSession,
    UnexpectedNonce(Nonce),
    SessionAlreadyExists,
    SessionNotReady,
    ECDHIsNotRequested,
    ECDHAlreadyRequested,
    KeysError(KeysError),
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &HandshakeError::IoError(ref err) => write!(f, "IoError {}", err),
            &HandshakeError::CIoError(ref err) => err.fmt(f),
            &HandshakeError::RlpError(ref err) => write!(f, "RlpError {}", err),
            &HandshakeError::SendError(ref msg, unsent) => {
                write!(f, "SendError {} bytes of {:?} are not sent", unsent, msg)
            }
            &HandshakeError::SymmetricCipherError(ref err) => write!(f, "SymmetricCipherError {:?}", err),
            &HandshakeError::NoSession => write!(f, "NoSession"),
            &HandshakeError::UnexpectedNonce(ref nonce) => write!(f, "{:?} is an unexpected nonce", nonce),
            &HandshakeError::SessionAlreadyExists => write!(f, "Session already exists"),
            &HandshakeError::SessionNotReady => write!(f, "Session is not ready yet"),
            &HandshakeError::ECDHIsNotRequested => write!(f, "Ecdh is not requested"),
            &HandshakeError::ECDHAlreadyRequested => write!(f, "Ecdh is already requested"),
            &HandshakeError::KeysError(ref err) => err.fmt(f),
        }
    }
}

impl error::Error for HandshakeError {
    fn description(&self) -> &str {
        match self {
            &HandshakeError::IoError(ref err) => err.description(),
            &HandshakeError::CIoError(ref err) => err.description(),
            &HandshakeError::RlpError(ref err) => err.description(),
            &HandshakeError::SendError(..) => "Unsent data",
            &HandshakeError::SymmetricCipherError(_) => "SymmetricCipherError",
            &HandshakeError::NoSession => "No session",
            &HandshakeError::UnexpectedNonce(_) => "Unexpected nonce",
            &HandshakeError::SessionAlreadyExists => "Session already exists",
            &HandshakeError::SessionNotReady => "Session is not ready yet",
            &HandshakeError::ECDHIsNotRequested => "Ecdh is not requested",
            &HandshakeError::ECDHAlreadyRequested => "Ecdh is already requested",
            &HandshakeError::KeysError(_) => "KeysError",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &HandshakeError::IoError(ref err) => Some(err),
            &HandshakeError::CIoError(_) => None,
            &HandshakeError::RlpError(ref err) => Some(err),
            &HandshakeError::SendError(..) => None,
            &HandshakeError::SymmetricCipherError(_) => None,
            &HandshakeError::NoSession => None,
            &HandshakeError::UnexpectedNonce(_) => None,
            &HandshakeError::SessionAlreadyExists => None,
            &HandshakeError::SessionNotReady => None,
            &HandshakeError::ECDHIsNotRequested => None,
            &HandshakeError::ECDHAlreadyRequested => None,
            &HandshakeError::KeysError(_) => None,
        }
    }
}

impl From<io::Error> for HandshakeError {
    fn from(err: io::Error) -> HandshakeError {
        HandshakeError::IoError(err)
    }
}

impl From<CIoError> for HandshakeError {
    fn from(err: CIoError) -> HandshakeError {
        HandshakeError::CIoError(err)
    }
}
impl From<DecoderError> for HandshakeError {
    fn from(err: DecoderError) -> HandshakeError {
        HandshakeError::RlpError(err)
    }
}

impl From<SymmetricCipherError> for HandshakeError {
    fn from(err: SymmetricCipherError) -> HandshakeError {
        HandshakeError::SymmetricCipherError(err)
    }
}

impl From<KeysError> for HandshakeError {
    fn from(err: KeysError) -> HandshakeError {
        HandshakeError::KeysError(err)
    }
}

const MAX_HANDSHAKE_PACKET_SIZE: usize = 1024;

impl Handshake {
    fn bind(socket_address: &SocketAddr) -> Result<Self, HandshakeError> {
        let socket = UdpSocket::bind(socket_address.into())?;
        Ok(Self {
            socket,
            secrets: HashMap::new(),
            nonces: HashMap::new(),
            temporary_nonces: HashMap::new(),
            requested: HashMap::new(),
            seq_counter: AtomicUsize::new(0),
        })
    }

    fn receive(&self) -> Result<Option<(HandshakeMessage, SocketAddr)>, HandshakeError> {
        let mut buf: [u8; MAX_HANDSHAKE_PACKET_SIZE] = [0; MAX_HANDSHAKE_PACKET_SIZE];
        match self.socket.recv_from(&mut buf) {
            Ok((received_size, socket_address)) => {
                let socket_address = SocketAddr::from(socket_address);
                let raw_bytes = &buf[0..received_size];
                let rlp = UntrustedRlp::new(&raw_bytes);
                let message = Decodable::decode(&rlp)?;

                info!("Handshake {:?} received from {:?}", message, socket_address);
                Ok(Some((message, SocketAddr::from(socket_address))))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(HandshakeError::from(e)),
        }
    }

    fn send_to(&self, message: &HandshakeMessage, target: &SocketAddr) -> Result<(), HandshakeError> {
        let unencrypted_bytes = message.rlp_bytes();

        let length_to_send = unencrypted_bytes.len();

        debug_assert!(length_to_send < MAX_HANDSHAKE_PACKET_SIZE);
        let sent_size = self.socket.send_to(&unencrypted_bytes, target.into())?;
        if sent_size != length_to_send {
            return Err(HandshakeError::SendError(message.clone(), length_to_send - sent_size))
        }
        info!("Handshake {:?} sent to {:?}", message, target);
        Ok(())
    }

    fn send_ping_to(&mut self, target: &SocketAddr) -> Result<(), HandshakeError> {
        let ephemeral = Random.generate()?;
        self.requested.insert(target.clone(), ephemeral.private().clone());

        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
        self.send_to(&HandshakeMessage::ecdh_request(seq as u64, *ephemeral.public()), target)?;
        Ok(())
    }

    fn on_packet(
        &mut self,
        message: &HandshakeMessage,
        from: &SocketAddr,
        extension: &IoChannel<connection::HandlerMessage>,
    ) -> Result<(), HandshakeError> {
        match message.body() {
            &HandshakeMessageBody::ConnectionRequest(ref received_nonce) => {
                let encrypted_bytes = {
                    let secret = self.secrets.get(from).ok_or(HandshakeError::NoSession)?;
                    if self.nonces.contains_key(&from) {
                        info!("A nonce already exists");
                    }

                    let temporary_session = Session::new_with_zero_nonce(secret.clone());

                    let temporary_nonce = decrypt_and_decode_nonce(&temporary_session, received_nonce)?;
                    let temporary_session = Session::new(*secret, temporary_nonce.clone());

                    // FIXME: let nonce = f(nonce)
                    let nonce = temporary_nonce;
                    let session = Session::new(*secret, nonce.clone());

                    let encrypted_nonce = encode_and_encrypt_nonce(&temporary_session, &nonce)?;
                    extension.send(connection::HandlerMessage::RegisterSession(from.clone(), session))?;
                    self.nonces.insert(from.clone(), nonce);
                    encrypted_nonce
                };

                let pong = HandshakeMessage::connection_allowed(message.seq(), encrypted_bytes);
                self.send_to(&pong, &from)?;
                Ok(())
            }
            &HandshakeMessageBody::ConnectionAllowed(ref nonce) => {
                let secret = self.secrets.get(from).ok_or(HandshakeError::NoSession)?;
                let temporary_nonce = self.temporary_nonces.get(&from);
                if temporary_nonce.is_none() {
                    return Err(From::from(HandshakeError::SessionNotReady))
                }
                let temporary_nonce = temporary_nonce.expect("Nonce must exist");
                let temporary_session = Session::new(*secret, temporary_nonce.clone());
                let nonce = decrypt_and_decode_nonce(&temporary_session, &nonce)?;

                if temporary_nonce != &nonce {
                    return Err(From::from(HandshakeError::UnexpectedNonce(nonce)))
                }

                let session = Session::new(*secret, nonce);
                extension.send(connection::HandlerMessage::RequestConnection(from.clone(), session))?;
                Ok(())
            }
            &HandshakeMessageBody::ConnectionDenied(ref reason) => {
                info!("Connection to {:?} refused(reason: {}", from, reason);
                Ok(())
            }
            &HandshakeMessageBody::EcdhRequest(ref key) => {
                let ephemeral = Random.generate()?;
                let secret = exchange(key, &ephemeral.private())?;
                if self.secrets.insert(from.clone(), secret).is_some() {
                    self.send_to(
                        &HandshakeMessage::ecdh_denied(message.seq(), "ECDH Already requested".to_string()),
                        from,
                    )?;
                    return Err(HandshakeError::ECDHAlreadyRequested)
                }
                self.send_to(&HandshakeMessage::ecdh_allowed(message.seq(), *ephemeral.public()), from)?;
                Ok(())
            }
            &HandshakeMessageBody::EcdhAllowed(ref key) => {
                if let Some(local_private) = self.requested.remove(from) {
                    let secret = exchange(key, &local_private)?;
                    let session = Session::new_with_zero_nonce(secret);

                    let mut rng = OsRng::new().expect("Cannot generate random number");
                    let nonce = rng.gen();
                    let encrypted_nonce = encode_and_encrypt_nonce(&session, &nonce)?;

                    if self.secrets.contains_key(&from) {
                        return Err(HandshakeError::SessionAlreadyExists)
                    }
                    let t = self.secrets.insert(from.clone(), secret);
                    debug_assert!(t.is_none());
                    let t = self.temporary_nonces.insert(from.clone(), nonce);
                    debug_assert!(t.is_none());

                    let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
                    self.send_to(&HandshakeMessage::connection_request(seq as u64, encrypted_nonce), from)?;
                    Ok(())
                } else {
                    Err(HandshakeError::ECDHIsNotRequested)
                }
            }
            &HandshakeMessageBody::EcdhDenied(ref reason) => {
                info!("Connection to {:?} refused(reason: {}", from, reason);
                if self.requested.remove(from).is_none() {
                    Err(HandshakeError::ECDHIsNotRequested)
                } else {
                    Ok(())
                }
            }
        }
    }
}

fn encode_and_encrypt_nonce(session: &Session, nonce: &Nonce) -> Result<Vec<u8>, HandshakeError> {
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
    connect_queue: VecDeque<SocketAddr>,
}

pub struct Handler {
    socket_address: SocketAddr,
    internal: Mutex<Internal>,
    extension: IoChannel<connection::HandlerMessage>,
    discovery: RwLock<Arc<DiscoveryApi>>,
    secret_key: Secret,
}

impl Handler {
    pub fn new(
        socket_address: SocketAddr,
        secret_key: Secret,
        extension: IoChannel<connection::HandlerMessage>,
        discovery: Arc<DiscoveryApi>,
    ) -> Self {
        let handshake = Handshake::bind(&socket_address).expect("Cannot bind UDP port");
        let discovery = RwLock::new(discovery);
        Self {
            socket_address,
            internal: Mutex::new(Internal {
                handshake,
                connect_queue: VecDeque::new(),
            }),
            extension,
            discovery,
            secret_key,
        }
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    ConnectTo(SocketAddr),
}

const RECV_TOKEN: usize = 0;

impl IoHandler<HandlerMessage> for Handler {
    fn initialize(&self, io: &IoContext<HandlerMessage>) -> IoHandlerResult<()> {
        io.register_stream(RECV_TOKEN)?;
        Ok(())
    }

    fn message(&self, _io: &IoContext<HandlerMessage>, message: &HandlerMessage) -> IoHandlerResult<()> {
        match message {
            &HandlerMessage::ConnectTo(ref socket_address) => {
                let mut internal = self.internal.lock();
                let ref mut queue = internal.connect_queue;
                queue.push_back(socket_address.clone());
            }
        };
        Ok(())
    }

    fn stream_hup(&self, _io: &IoContext<HandlerMessage>, _stream: StreamToken) -> IoHandlerResult<()> {
        info!("handshake server closed");
        Ok(())
    }

    fn stream_readable(&self, _io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            RECV_TOKEN => loop {
                let mut internal = self.internal.lock();
                let ref mut handshake = internal.handshake;
                match handshake.receive() {
                    Ok(None) => break,
                    Ok(Some((msg, socket_address))) => {
                        info!("{:?} from {:?}", msg, socket_address);
                        handshake.on_packet(&msg, &socket_address, &self.extension)?;
                    }
                    Err(err) => {
                        info!("handshake receive error {}", err);
                    }
                };
            },
            _ => {
                info!("Unknown stream token {}", stream);
            }
        };
        Ok(())
    }

    fn stream_writable(&self, _io: &IoContext<HandlerMessage>, _stream: StreamToken) -> IoHandlerResult<()> {
        loop {
            let mut internal = self.internal.lock();
            if let Some(ref socket_address) = internal.connect_queue.pop_front().as_ref() {
                let ref mut handshake = internal.handshake;
                connect_to(handshake, &socket_address)?;
            } else {
                break
            }
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
            RECV_TOKEN => {
                let mut internal = self.internal.lock();
                let ref mut handshake = internal.handshake;
                event_loop.register(&handshake.socket, reg, Ready::readable() | Ready::writable(), PollOpt::edge())?;
            }
            _ => {
                unreachable!();
            }
        }
        Ok(())
    }
}

fn connect_to(handshake: &mut Handshake, socket_address: &SocketAddr) -> IoHandlerResult<()> {
    handshake.send_ping_to(&socket_address)?;
    Ok(())
}
