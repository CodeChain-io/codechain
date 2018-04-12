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

use std::collections::HashMap;
use std::error;
use std::fmt;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ccrypto::aes::SymmetricCipherError;
use cfinally::finally;
use cio::{IoChannel, IoContext, IoError as CIoError, IoHandler, IoHandlerResult, IoManager, StreamToken, TimerToken};
use ckeys::{exchange, Error as KeysError, Generator, Private, Random};
use ctypes::Secret;
use mio::deprecated::EventLoop;
use mio::Token;
use parking_lot::{Mutex, RwLock};
use rand::{OsRng, Rng};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};

use super::super::connection;
use super::super::session::{Nonce, Session};
use super::super::token_generator::TokenGenerator;
use super::super::{DiscoveryApi, SocketAddr};
use super::server::{Error as ServerError, Server};
use super::{HandshakeMessage, HandshakeMessageBody};


pub struct Handshake {
    server: Server,
    secrets: HashMap<SocketAddr, Secret>,
    nonces: HashMap<SocketAddr, Nonce>,
    temporary_nonces: HashMap<SocketAddr, Nonce>,
    requested: HashMap<SocketAddr, Private>,
    seq_counter: AtomicUsize,

    tmp_nonce_tokens: TokenGenerator,
    tmp_nonce_token_to_addr: HashMap<TimerToken, SocketAddr>,
    addr_to_tmp_nonce_token: HashMap<SocketAddr, TimerToken>,
}

#[derive(Debug)]
enum Error {
    Server(ServerError),
    Io(io::Error),
    CIo(CIoError),
    Decoder(DecoderError),
    SymmetricCipher(SymmetricCipherError),
    NoSession,
    UnexpectedNonce(Nonce),
    SessionAlreadyExists,
    SessionNotReady,
    ECDHIsNotRequested,
    ECDHAlreadyRequested,
    Keys(KeysError),
    TooManyTmpNonces,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Server(ref err) => err.fmt(f),
            &Error::Io(ref err) => err.fmt(f),
            &Error::CIo(ref err) => err.fmt(f),
            &Error::Decoder(ref err) => err.fmt(f),
            &Error::SymmetricCipher(_) => fmt::Debug::fmt(&self, f),
            &Error::NoSession => fmt::Debug::fmt(&self, f),
            &Error::UnexpectedNonce(_) => fmt::Debug::fmt(&self, f),
            &Error::SessionAlreadyExists => fmt::Debug::fmt(&self, f),
            &Error::SessionNotReady => fmt::Debug::fmt(&self, f),
            &Error::ECDHIsNotRequested => fmt::Debug::fmt(&self, f),
            &Error::ECDHAlreadyRequested => fmt::Debug::fmt(&self, f),
            &Error::Keys(ref err) => err.fmt(f),
            &Error::TooManyTmpNonces => fmt::Debug::fmt(&self, f),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            &Error::Server(ref err) => err.description(),
            &Error::Io(ref err) => err.description(),
            &Error::CIo(ref err) => err.description(),
            &Error::Decoder(ref err) => err.description(),
            &Error::SymmetricCipher(_) => "SymmetricCipherError",
            &Error::NoSession => "No session",
            &Error::UnexpectedNonce(_) => "Unexpected nonce",
            &Error::SessionAlreadyExists => "Session already exists",
            &Error::SessionNotReady => "Session is not ready yet",
            &Error::ECDHIsNotRequested => "Ecdh is not requested",
            &Error::ECDHAlreadyRequested => "Ecdh is already requested",
            &Error::Keys(_) => "KeysError",
            &Error::TooManyTmpNonces => "Too many tmp nonces",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &Error::Server(ref err) => Some(err),
            &Error::Io(ref err) => Some(err),
            &Error::CIo(_) => None,
            &Error::Decoder(ref err) => Some(err),
            &Error::SymmetricCipher(_) => None,
            &Error::NoSession => None,
            &Error::UnexpectedNonce(_) => None,
            &Error::SessionAlreadyExists => None,
            &Error::SessionNotReady => None,
            &Error::ECDHIsNotRequested => None,
            &Error::ECDHAlreadyRequested => None,
            &Error::Keys(_) => None,
            &Error::TooManyTmpNonces => None,
        }
    }
}

impl From<ServerError> for Error {
    fn from(err: ServerError) -> Self {
        Error::Server(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<CIoError> for Error {
    fn from(err: CIoError) -> Error {
        Error::CIo(err)
    }
}
impl From<DecoderError> for Error {
    fn from(err: DecoderError) -> Error {
        Error::Decoder(err)
    }
}

impl From<SymmetricCipherError> for Error {
    fn from(err: SymmetricCipherError) -> Error {
        Error::SymmetricCipher(err)
    }
}

impl From<KeysError> for Error {
    fn from(err: KeysError) -> Error {
        Error::Keys(err)
    }
}

type Result<T> = ::std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum HandlerMessage {
    ConnectTo(SocketAddr),
}

const START_OF_TMP_NONCE_TOKEN: TimerToken = 0;
const NUM_OF_TMP_NONCES: usize = 100;
const END_OF_TMP_NONCE_TOKEN: TimerToken = START_OF_TMP_NONCE_TOKEN + NUM_OF_TMP_NONCES;

const TMP_NONCE_TIMEOUT_MS: u64 = 10 * 1000;

impl Handshake {
    fn bind(socket_address: &SocketAddr) -> Result<Self> {
        let server = Server::bind(socket_address)?;
        Ok(Self {
            server,
            secrets: HashMap::new(),
            nonces: HashMap::new(),
            temporary_nonces: HashMap::new(),
            requested: HashMap::new(),
            seq_counter: AtomicUsize::new(0),

            tmp_nonce_tokens: TokenGenerator::new(START_OF_TMP_NONCE_TOKEN, NUM_OF_TMP_NONCES),
            tmp_nonce_token_to_addr: HashMap::new(),
            addr_to_tmp_nonce_token: HashMap::new(),
        })
    }

    fn receive(&self) -> Result<Option<(HandshakeMessage, SocketAddr)>> {
        Ok(self.server.receive()?)
    }

    // return false if there is no message to be sent
    fn read(
        &mut self,
        extension: &IoChannel<connection::HandlerMessage>,
        io: &IoContext<HandlerMessage>,
    ) -> Result<bool> {
        match self.receive() {
            Ok(None) => Ok(false),
            Ok(Some((msg, socket_address))) => {
                self.on_packet(&msg, &socket_address, extension, io)?;
                Ok(true)
            }
            Err(err) => Err(From::from(err)),
        }
    }

    // return false if there is no message to be sent
    fn send(&mut self) -> Result<bool> {
        Ok(self.server.send()?)
    }

    fn create_new_connection(&mut self, target: &SocketAddr) -> Result<()> {
        let ephemeral = Random.generate()?;
        self.requested.insert(target.clone(), ephemeral.private().clone());

        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
        let message = HandshakeMessage::ecdh_request(seq as u64, *ephemeral.public());
        self.server.enqueue(message, target.clone())?;
        Ok(())
    }

    fn on_packet(
        &mut self,
        message: &HandshakeMessage,
        from: &SocketAddr,
        extension: &IoChannel<connection::HandlerMessage>,
        io: &IoContext<HandlerMessage>,
    ) -> Result<()> {
        match message.body() {
            &HandshakeMessageBody::ConnectionRequest(ref received_nonce) => {
                let encrypted_bytes = {
                    let secret = self.secrets.get(from).ok_or(Error::NoSession)?;
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
                self.server.enqueue(pong, from.clone())?;
                Ok(())
            }
            &HandshakeMessageBody::ConnectionAllowed(ref nonce) => {
                let temporary_nonce = self.temporary_nonces.get(&from).ok_or(Error::SessionNotReady)?;
                let secret = self.secrets.get(from).ok_or(Error::NoSession)?;
                let temporary_session = Session::new(*secret, temporary_nonce.clone());
                let nonce = decrypt_and_decode_nonce(&temporary_session, &nonce)?;

                if temporary_nonce != &nonce {
                    return Err(From::from(Error::UnexpectedNonce(nonce)))
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
                    let message = HandshakeMessage::ecdh_denied(message.seq(), "ECDH Already requested".to_string());
                    self.server.enqueue(message, from.clone())?;
                    return Err(Error::ECDHAlreadyRequested)
                }
                let message = HandshakeMessage::ecdh_allowed(message.seq(), *ephemeral.public());
                self.server.enqueue(message, from.clone())?;
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
                        return Err(Error::SessionAlreadyExists)
                    }

                    let token = self.tmp_nonce_tokens.gen().ok_or(Error::TooManyTmpNonces)?;
                    let t = self.secrets.insert(from.clone(), secret);
                    debug_assert!(t.is_none());
                    let t = self.temporary_nonces.insert(from.clone(), nonce);
                    debug_assert!(t.is_none());

                    let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
                    let message = HandshakeMessage::connection_request(seq as u64, encrypted_nonce);
                    if let Err(err) = self.server.enqueue(message, from.clone()) {
                        let t = self.tmp_nonce_tokens.restore(token);
                        debug_assert!(t);
                        return Err(From::from(err))
                    };

                    let t = self.tmp_nonce_token_to_addr.insert(token, from.clone());
                    debug_assert!(t.is_none());
                    let t = self.addr_to_tmp_nonce_token.insert(from.clone(), token);
                    debug_assert!(t.is_none());

                    io.register_timer_once(token, TMP_NONCE_TIMEOUT_MS)?;
                    Ok(())
                } else {
                    Err(Error::ECDHIsNotRequested)
                }
            }
            &HandshakeMessageBody::EcdhDenied(ref reason) => {
                info!("Connection to {:?} refused(reason: {}", from, reason);
                if self.requested.remove(from).is_none() {
                    Err(Error::ECDHIsNotRequested)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn remove_temporary_nonce(&mut self, timer: &TimerToken) -> bool {
        if let Some(socket_address) = self.tmp_nonce_token_to_addr.remove(&timer) {
            let t = self.addr_to_tmp_nonce_token.remove(&socket_address);
            debug_assert!(t.is_some());
            let t = self.tmp_nonce_tokens.restore(*timer);
            debug_assert!(t);
            let t = self.temporary_nonces.remove(&socket_address);
            debug_assert!(t.is_some());
            true
        } else {
            false
        }
    }

    fn register(&self, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) -> io::Result<()> {
        Ok(self.server.register(reg, event_loop)?)
    }

    fn reregister(&self, reg: Token, event_loop: &mut EventLoop<IoManager<HandlerMessage>>) -> io::Result<()> {
        Ok(self.server.reregister(reg, event_loop)?)
    }
}

fn encode_and_encrypt_nonce(session: &Session, nonce: &Nonce) -> Result<Vec<u8>> {
    let unencrypted_bytes = nonce.rlp_bytes();
    Ok(session.encrypt(&unencrypted_bytes)?)
}

fn decrypt_and_decode_nonce(session: &Session, encrypted_bytes: &Vec<u8>) -> Result<Nonce> {
    let unencrypted_bytes = session.decrypt(&encrypted_bytes)?;
    let rlp = UntrustedRlp::new(&unencrypted_bytes);
    Ok(Decodable::decode(&rlp)?)
}

pub struct Handler {
    handshake: Mutex<Handshake>,
    extension: IoChannel<connection::HandlerMessage>,
    #[allow(dead_code)]
    discovery: RwLock<Arc<DiscoveryApi>>,
    #[allow(dead_code)]
    secret_key: Secret,
}

impl Handler {
    pub fn new(
        socket_address: SocketAddr,
        secret_key: Secret,
        extension: IoChannel<connection::HandlerMessage>,
        discovery: Arc<DiscoveryApi>,
    ) -> Self {
        let handshake = Mutex::new(Handshake::bind(&socket_address).expect("Cannot bind UDP port"));
        let discovery = RwLock::new(discovery);
        Self {
            handshake,
            extension,
            discovery,
            secret_key,
        }
    }
}

const RECEIVE_TOKEN: usize = 0;

impl IoHandler<HandlerMessage> for Handler {
    fn initialize(&self, io: &IoContext<HandlerMessage>) -> IoHandlerResult<()> {
        io.register_stream(RECEIVE_TOKEN)?;
        Ok(())
    }

    fn timeout(&self, _io: &IoContext<HandlerMessage>, timer: TimerToken) -> IoHandlerResult<()> {
        match timer {
            START_OF_TMP_NONCE_TOKEN...END_OF_TMP_NONCE_TOKEN => {
                let mut handshake = self.handshake.lock();
                let t = handshake.remove_temporary_nonce(&timer);
                debug_assert!(t);
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn message(&self, io: &IoContext<HandlerMessage>, message: &HandlerMessage) -> IoHandlerResult<()> {
        match message {
            &HandlerMessage::ConnectTo(ref socket_address) => {
                let mut handshake = self.handshake.lock();
                handshake.create_new_connection(&socket_address)?;
                io.update_registration(RECEIVE_TOKEN)?;
            }
        };
        Ok(())
    }

    fn stream_hup(&self, _io: &IoContext<HandlerMessage>, _stream: StreamToken) -> IoHandlerResult<()> {
        unreachable!()
    }

    fn stream_readable(&self, io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        if stream != RECEIVE_TOKEN {
            unreachable!()
        }
        let _f = finally(|| {
            if let Err(err) = io.update_registration(stream) {
                info!("Cannot update registration for handshake : {:?}", err);
            }
        });
        loop {
            let mut handshake = self.handshake.lock();
            if !handshake.read(&self.extension, io)? {
                break
            }
        }
        Ok(())
    }

    fn stream_writable(&self, io: &IoContext<HandlerMessage>, stream: StreamToken) -> IoHandlerResult<()> {
        if stream != RECEIVE_TOKEN {
            unreachable!()
        }

        let _f = finally(|| {
            if let Err(err) = io.update_registration(stream) {
                info!("Cannot update registration for handshake : {:?}", err);
            }
        });
        loop {
            let mut handshake = self.handshake.lock();
            if !handshake.send()? {
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
        if stream != RECEIVE_TOKEN {
            unreachable!()
        }
        let handshake = self.handshake.lock();
        Ok(handshake.register(reg, event_loop)?)
    }

    fn update_stream(
        &self,
        stream: usize,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<HandlerMessage>>,
    ) -> IoHandlerResult<()> {
        if stream != RECEIVE_TOKEN {
            unreachable!()
        }
        let handshake = self.handshake.lock();
        Ok(handshake.reregister(reg, event_loop)?)
    }

    fn deregister_stream(
        &self,
        _stream: usize,
        _event_loop: &mut EventLoop<IoManager<HandlerMessage>>,
    ) -> IoHandlerResult<()> {
        unreachable!()
    }
}
