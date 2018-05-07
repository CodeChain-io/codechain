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

use super::super::p2p;
use super::super::session::{Nonce, Session};
use super::super::token_generator::TokenGenerator;
use super::super::{DiscoveryApi, SocketAddr};
use super::message;
use super::server::{Error as ServerError, Server};


struct SessionInitiator {
    server: Server,
    secrets: HashMap<SocketAddr, Secret>,
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
    Keys(KeysError),
    General(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Server(ref err) => err.fmt(f),
            &Error::Io(ref err) => err.fmt(f),
            &Error::CIo(ref err) => err.fmt(f),
            &Error::Decoder(ref err) => err.fmt(f),
            &Error::SymmetricCipher(_) => fmt::Debug::fmt(&self, f),
            &Error::Keys(ref err) => err.fmt(f),
            &Error::General(_) => fmt::Debug::fmt(&self, f),
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
            &Error::Keys(_) => "KeysError",
            &Error::General(ref str) => str,
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &Error::Server(ref err) => Some(err),
            &Error::Io(ref err) => Some(err),
            &Error::CIo(_) => None,
            &Error::Decoder(ref err) => Some(err),
            &Error::SymmetricCipher(_) => None,
            &Error::Keys(_) => None,
            &Error::General(_) => None,
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
pub enum Message {
    ConnectTo(SocketAddr),
}

const START_OF_TMP_NONCE_TOKEN: TimerToken = 0;
const NUM_OF_TMP_NONCES: usize = 100;
const END_OF_TMP_NONCE_TOKEN: TimerToken = START_OF_TMP_NONCE_TOKEN + NUM_OF_TMP_NONCES;

const TMP_NONCE_TIMEOUT_MS: u64 = 10 * 1000;

impl SessionInitiator {
    fn bind(socket_address: &SocketAddr) -> Result<Self> {
        let server = Server::bind(socket_address)?;
        Ok(Self {
            server,
            secrets: HashMap::new(),
            temporary_nonces: HashMap::new(),
            requested: HashMap::new(),
            seq_counter: AtomicUsize::new(0),

            tmp_nonce_tokens: TokenGenerator::new(START_OF_TMP_NONCE_TOKEN, NUM_OF_TMP_NONCES),
            tmp_nonce_token_to_addr: HashMap::new(),
            addr_to_tmp_nonce_token: HashMap::new(),
        })
    }

    fn receive(&self) -> Result<Option<(message::Message, SocketAddr)>> {
        Ok(self.server.receive()?)
    }

    // return false if there is no message to be sent
    fn read(&mut self, channel_to_p2p: &IoChannel<p2p::Message>, io: &IoContext<Message>) -> Result<bool> {
        match self.receive() {
            Ok(None) => Ok(false),
            Ok(Some((msg, socket_address))) => {
                self.on_packet(&msg, &socket_address, channel_to_p2p, io)?;
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
        let message = message::Message::ecdh_request(seq as u64, *ephemeral.public());
        self.server.enqueue(message, target.clone())?;
        Ok(())
    }

    fn on_packet(
        &mut self,
        message: &message::Message,
        from: &SocketAddr,
        channel_to_p2p: &IoChannel<p2p::Message>,
        io: &IoContext<Message>,
    ) -> Result<()> {
        match message.body() {
            &message::Body::ConnectionRequest(ref received_nonce) => {
                let encrypted_bytes = {
                    let secret = self.secrets.get(from).ok_or(Error::General("NoSession"))?;

                    let temporary_session = Session::new_with_zero_nonce(secret.clone());

                    let temporary_nonce = decrypt_and_decode_nonce(&temporary_session, received_nonce)?;
                    let temporary_session = Session::new(*secret, temporary_nonce.clone());

                    let mut rng = OsRng::new().expect("Cannot generate random number");
                    let nonce: Nonce = rng.gen();
                    let encrypted_nonce = encode_and_encrypt_nonce(&temporary_session, &nonce)?;

                    let session = Session::new(*secret, nonce);
                    channel_to_p2p.send(p2p::Message::RegisterSession(from.clone(), session))?;
                    encrypted_nonce
                };

                let pong = message::Message::connection_allowed(message.seq(), encrypted_bytes);
                self.server.enqueue(pong, from.clone())?;
                Ok(())
            }
            &message::Body::ConnectionAllowed(ref nonce) => {
                let temporary_nonce = self.temporary_nonces.get(&from).ok_or(Error::General("SessionNotReady"))?;
                let secret = self.secrets.get(from).ok_or(Error::General("NoSession"))?;
                let temporary_session = Session::new(*secret, temporary_nonce.clone());
                let nonce = decrypt_and_decode_nonce(&temporary_session, &nonce)?;

                let session = Session::new(*secret, nonce);
                channel_to_p2p.send(p2p::Message::RegisterSession(from.clone(), session))?;
                Ok(())
            }
            &message::Body::ConnectionDenied(ref reason) => {
                info!(target:"net", "Connection to {:?} refused(reason: {}", from, reason);
                Ok(())
            }
            &message::Body::EcdhRequest(ref key) => {
                let ephemeral = Random.generate()?;
                let secret = exchange(key, &ephemeral.private())?;
                if self.secrets.insert(from.clone(), secret).is_some() {
                    let message = message::Message::ecdh_denied(message.seq(), "ECDH Already requested".to_string());
                    self.server.enqueue(message, from.clone())?;
                    return Err(Error::General("ECDHAlreadyRequested"))
                }
                let message = message::Message::ecdh_allowed(message.seq(), *ephemeral.public());
                self.server.enqueue(message, from.clone())?;
                Ok(())
            }
            &message::Body::EcdhAllowed(ref key) => {
                let local_private = self.requested.remove(from).ok_or(Error::General("ECDHIsNotRequested"))?;
                let secret = exchange(key, &local_private)?;
                let session = Session::new_with_zero_nonce(secret);

                let mut rng = OsRng::new().expect("Cannot generate random number");
                let nonce = rng.gen();
                let encrypted_nonce = encode_and_encrypt_nonce(&session, &nonce)?;

                if self.secrets.contains_key(&from) {
                    return Err(Error::General("SessionAlreadyExists"))
                }

                let token = self.tmp_nonce_tokens.gen().ok_or(Error::General("TooManyTemporaryNonces"))?;
                let t = self.secrets.insert(from.clone(), secret);
                debug_assert!(t.is_none());
                let t = self.temporary_nonces.insert(from.clone(), nonce);
                debug_assert!(t.is_none());

                let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);
                let message = message::Message::connection_request(seq as u64, encrypted_nonce);
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
            }
            &message::Body::EcdhDenied(ref reason) => {
                info!(target:"net", "Connection to {:?} refused(reason: {}", from, reason);
                let _ = self.requested.remove(from).ok_or(Error::General("ECDHIsNotRequested"))?;
                Ok(())
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

    fn register(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()> {
        Ok(self.server.register(reg, event_loop)?)
    }

    fn reregister(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()> {
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
    session_initiator: Mutex<SessionInitiator>,
    channel_to_p2p: IoChannel<p2p::Message>,
    discovery: RwLock<Option<Arc<DiscoveryApi>>>,
}

impl Handler {
    pub fn new(socket_address: SocketAddr, channel_to_p2p: IoChannel<p2p::Message>) -> Self {
        let session_initiator = Mutex::new(SessionInitiator::bind(&socket_address).expect("Cannot bind UDP port"));
        Self {
            session_initiator,
            channel_to_p2p,
            discovery: RwLock::new(None),
        }
    }

    pub fn set_discovery_api(&self, api: Arc<DiscoveryApi>) {
        *self.discovery.write() = Some(api);
    }
}

const RECEIVE_TOKEN: usize = 0;

impl IoHandler<Message> for Handler {
    fn initialize(&self, io: &IoContext<Message>) -> IoHandlerResult<()> {
        io.register_stream(RECEIVE_TOKEN)?;
        Ok(())
    }

    fn timeout(&self, _io: &IoContext<Message>, timer: TimerToken) -> IoHandlerResult<()> {
        match timer {
            START_OF_TMP_NONCE_TOKEN...END_OF_TMP_NONCE_TOKEN => {
                let mut session_initiator = self.session_initiator.lock();
                let t = session_initiator.remove_temporary_nonce(&timer);
                debug_assert!(t);
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn message(&self, io: &IoContext<Message>, message: &Message) -> IoHandlerResult<()> {
        match message {
            &Message::ConnectTo(ref socket_address) => {
                let mut session_initiator = self.session_initiator.lock();
                session_initiator.create_new_connection(&socket_address)?;
                io.update_registration(RECEIVE_TOKEN)?;
            }
        };
        Ok(())
    }

    fn stream_hup(&self, _io: &IoContext<Message>, _stream: StreamToken) -> IoHandlerResult<()> {
        unreachable!()
    }

    fn stream_readable(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        if stream != RECEIVE_TOKEN {
            unreachable!()
        }
        let _f = finally(|| {
            if let Err(err) = io.update_registration(stream) {
                warn!(target:"net", "Cannot update registration for session_initiator : {:?}", err);
            }
        });
        loop {
            let mut session_initiator = self.session_initiator.lock();
            if !session_initiator.read(&self.channel_to_p2p, io)? {
                break
            }
        }
        Ok(())
    }

    fn stream_writable(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        if stream != RECEIVE_TOKEN {
            unreachable!()
        }

        let _f = finally(|| {
            if let Err(err) = io.update_registration(stream) {
                warn!(target: "net", "Cannot update registration for session_initiator : {:?}", err);
            }
        });
        loop {
            let mut session_initiator = self.session_initiator.lock();
            if !session_initiator.send()? {
                break
            }
        }
        Ok(())
    }

    fn register_stream(
        &self,
        stream: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        if stream != RECEIVE_TOKEN {
            unreachable!()
        }
        let session_initiator = self.session_initiator.lock();
        Ok(session_initiator.register(reg, event_loop)?)
    }

    fn update_stream(
        &self,
        stream: usize,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        if stream != RECEIVE_TOKEN {
            unreachable!()
        }
        let session_initiator = self.session_initiator.lock();
        Ok(session_initiator.reregister(reg, event_loop)?)
    }

    fn deregister_stream(
        &self,
        _stream: usize,
        _event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        unreachable!()
    }
}
