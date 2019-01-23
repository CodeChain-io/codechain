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
use std::io;
use std::sync::Arc;

use ccrypto::aes::SymmetricCipherError;
use cio::{IoChannel, IoContext, IoError as CIoError, IoHandler, IoHandlerResult, IoManager, StreamToken, TimerToken};
use ckey::{Error as KeyError, Secret};
use finally::finally;
use mio::deprecated::EventLoop;
use mio::Token;
use parking_lot::RwLock;
use rlp::DecoderError;
use token_generator::TokenGenerator;

use crate::socket::{message, Socket};
use crate::{p2p, FiltersControl, IntoSocketAddr, RoutingTable, SocketAddr};

const REFRESH_TIMER_TOKEN: TimerToken = 0;
const BEGIN_OF_REQUEST_TOKEN: TimerToken = 1;
const NUMBER_OF_REQUESTS: usize = 100;
const END_OF_REQUEST_TOKEN: TimerToken = BEGIN_OF_REQUEST_TOKEN + NUMBER_OF_REQUESTS;

struct Requests {
    request_tokens: TokenGenerator,
    requests: HashMap<usize, SocketAddr>,
    manually_connected_address: HashSet<SocketAddr>,
}

impl Requests {
    fn new() -> Self {
        Self {
            request_tokens: TokenGenerator::new(BEGIN_OF_REQUEST_TOKEN, NUMBER_OF_REQUESTS),
            requests: HashMap::new(),
            manually_connected_address: HashSet::new(),
        }
    }

    fn gen(&mut self, socket_address: SocketAddr) -> IoHandlerResult<usize> {
        let seq = self.request_tokens.gen().ok_or("Too many connections")?;
        let t = self.requests.insert(seq, socket_address);
        debug_assert!(t.is_none());
        Ok(seq)
    }

    fn restore(&mut self, seq: usize, address: Option<SocketAddr>) -> IoHandlerResult<Option<SocketAddr>> {
        if let Some(address) = address {
            match self.requests.get(&seq) {
                None => {
                    debug_assert!(!self.request_tokens.is_assigned(seq));
                    return Ok(None)
                }
                Some(sent_address) => {
                    if sent_address != &address {
                        return Err("Invalid address".into())
                    }
                }
            }
        }
        let t = self.request_tokens.restore(seq);
        let address = self.requests.remove(&seq);
        debug_assert_eq!(t, address.is_some());
        Ok(address)
    }
}

struct SessionInitiator {
    socket: Socket,

    routing_table: Arc<RoutingTable>,
    requests: Requests,
    channel_to_p2p: IoChannel<p2p::Message>,
    filters: Arc<FiltersControl>,
}

#[derive(Debug)]
enum Error {
    Io(io::Error),
    CIo(CIoError),
    Decoder(DecoderError),
    SymmetricCipher(SymmetricCipherError),
    Key(KeyError),
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

impl From<KeyError> for Error {
    fn from(err: KeyError) -> Error {
        Error::Key(err)
    }
}

type Result<T> = ::std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub enum Message {
    ConnectTo(SocketAddr),
    ManuallyConnectTo(SocketAddr),
    PreimportSecret(Secret, SocketAddr),
    RequestSession(usize),
}

const MESSAGE_TIMEOUT_MS: u64 = 3_000;

impl SessionInitiator {
    fn bind(
        socket_address: &SocketAddr,
        routing_table: Arc<RoutingTable>,
        channel_to_p2p: IoChannel<p2p::Message>,
        filters: Arc<FiltersControl>,
    ) -> Result<Self> {
        let socket = Socket::bind(socket_address)?;
        Ok(Self {
            socket,
            routing_table,
            requests: Requests::new(),
            channel_to_p2p,
            filters,
        })
    }

    fn receive(&self) -> IoHandlerResult<Option<(message::Message, SocketAddr)>> {
        Ok(self.socket.receive().map_err(|err| format!("{:?}", err))?)
    }

    // return false if there is no message to be sent
    fn read(&mut self, io: &IoContext<Message>) -> IoHandlerResult<bool> {
        match self.receive()? {
            None => Ok(false),
            Some((msg, socket_address)) => {
                let ip = socket_address.ip();
                if !self.filters.is_allowed(&ip) {
                    return Err(format!("Message from {} is received. But it's not allowed", ip).into())
                }
                self.on_packet(&msg, &socket_address, io)?;
                Ok(true)
            }
        }
    }

    // return false if there is no message to be sent
    fn send(&mut self) -> IoHandlerResult<bool> {
        Ok(self.socket.flush().map_err(|err| format!("{:?}", err))?)
    }

    fn create_new_connection(&mut self, target: &SocketAddr, io: &IoContext<Message>) -> IoHandlerResult<()> {
        let seq = self.requests.gen(*target)?;
        io.register_timer_once(seq, MESSAGE_TIMEOUT_MS);
        let message = message::Message::node_id_request(seq as u64, target.into());
        self.socket.send(message, *target).map_err(|err| format!("{:?}", err))?;
        Ok(())
    }

    fn on_packet(
        &mut self,
        message: &message::Message,
        from: &SocketAddr,
        io: &IoContext<Message>,
    ) -> IoHandlerResult<()> {
        match message.body() {
            message::Body::NodeIdRequest(responder_node_id) => {
                if !self.routing_table.add_node(from, *responder_node_id) {
                    ctrace!(NETWORK, "{} is not a new candidate", from);
                }

                let requester_node_id = from.into();
                let message = message::Message::node_id_response(message.seq(), requester_node_id);
                self.socket.send(message, *from).map_err(|e| format!("{:?}", e))?;
                Ok(())
            }
            message::Body::NodeIdResponse(requester_node_id) => {
                if &requester_node_id.into_addr() == from {
                    return Ok(())
                }

                io.clear_timer(message.seq() as TimerToken);
                self.requests
                    .restore(message.seq() as usize, Some(*from))
                    .map_err(|err| format!("Invalid message({:?}) from {}: {:?}", message, from, err))?;

                if !self.routing_table.add_node(from, *requester_node_id) {
                    ctrace!(NETWORK, "{} is not a new candidate", from);
                }

                if self.routing_table.is_secret_preimported(from) {
                    let seq = self.requests.gen(*from)?;
                    io.register_timer_once(seq, MESSAGE_TIMEOUT_MS);

                    let encrypted_nonce = self.routing_table.request_session(from).ok_or("Cannot generate nonce")?;

                    let message = message::Message::nonce_request(seq as u64, encrypted_nonce);
                    self.socket.send(message, *from).map_err(|err| format!("{:?}", err))?;
                } else {
                    let requester_pub_key =
                        self.routing_table.register_key_pair_for_secret(from).ok_or("Cannot register key pair")?;

                    let seq = self.requests.gen(*from)?;
                    io.register_timer_once(seq, MESSAGE_TIMEOUT_MS);

                    let message = message::Message::secret_request(seq as u64, requester_pub_key);
                    self.socket.send(message, *from).map_err(|e| format!("{:?}", e))?;
                }

                Ok(())
            }
            message::Body::SecretRequest(requester_pub_key) => {
                if let Some(responder_pub_key) = self.routing_table.register_key_pair_for_secret(from) {
                    if let Some(_secret) = self.routing_table.share_secret(from, requester_pub_key) {
                        let message = message::Message::secret_allowed(message.seq(), responder_pub_key);
                        self.socket.send(message, *from).map_err(|e| format!("{:?}", e))?;
                        return Ok(())
                    } else if !self.routing_table.remove_node(*from) {
                        cwarn!(NETWORK, "Cannot reset key pair to {}", from);
                    }
                }

                let message = message::Message::secret_denied(message.seq(), "ECDH Already requested".to_string());
                self.socket.send(message, *from).map_err(|e| format!("{:?}", e))?;
                Err("Cannot response to secret request".into())
            }
            message::Body::SecretAllowed(responder_pub_key) => {
                io.clear_timer(message.seq() as TimerToken);
                self.requests
                    .restore(message.seq() as usize, Some(*from))
                    .map_err(|err| format!("Invalid message({:?}) from {}: {:?}", message, from, err))?;

                let _secret = self.routing_table.share_secret(from, responder_pub_key).ok_or("Cannot share secret")?;
                let encrypted_nonce = self.routing_table.request_session(from).ok_or("Cannot generate nonce")?;

                let seq = self.requests.gen(*from)?;
                io.register_timer_once(seq, MESSAGE_TIMEOUT_MS);

                let message = message::Message::nonce_request(seq as u64, encrypted_nonce);
                self.socket.send(message, *from).map_err(|e| format!("{:?}", e))?;

                Ok(())
            }
            message::Body::SecretDenied(reason) => {
                io.clear_timer(message.seq() as TimerToken);
                self.requests
                    .restore(message.seq() as usize, Some(*from))
                    .map_err(|err| format!("Invalid message({:?}) from {}: {:?}", message, from, err))?;

                if self.routing_table.remove_node(*from) {
                    cinfo!(NETWORK, "Shared Secret to {} denied (reason: {})", from, reason);
                }
                Ok(())
            }
            message::Body::NonceRequest(encrypted_temporary_nonce) => {
                if let Some(encrypted_nonce) =
                    self.routing_table.create_requested_session(from, &encrypted_temporary_nonce)
                {
                    let message = message::Message::nonce_allowed(message.seq(), encrypted_nonce);
                    self.socket.send(message, *from).map_err(|e| format!("{:?}", e))?;
                    return Ok(())
                }

                let message = message::Message::nonce_denied(message.seq(), "Cannot create session".to_string());
                self.socket.send(message, *from).map_err(|e| format!("{:?}", e))?;
                Err("Cannot create session".into())
            }
            message::Body::NonceAllowed(encrypted_nonce) => {
                io.clear_timer(message.seq() as TimerToken);
                self.requests
                    .restore(message.seq() as usize, Some(*from))
                    .map_err(|err| format!("Invalid message({:?}) from {}: {:?}", message, from, err))?;

                if self.requests.manually_connected_address.take(from).is_some() {
                    self.channel_to_p2p
                        .send(p2p::Message::RequestConnection(*from, p2p::IgnoreConnectionLimit::Ignore))?;
                }

                if !self.routing_table.create_allowed_session(from, &encrypted_nonce) {
                    return Err(format!("Cannot create session to {}", from).into())
                }
                Ok(())
            }
            message::Body::NonceDenied(reason) => {
                io.clear_timer(message.seq() as TimerToken);
                self.requests
                    .restore(message.seq() as usize, Some(*from))
                    .map_err(|err| format!("Invalid message({:?}) from {}: {:?}", message, from, err))?;

                self.routing_table.reset_imported_secret(from);

                cinfo!(NETWORK, "Connection to {} refused(reason: {})", from, reason);
                Ok(())
            }
        }
    }

    fn register(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()> {
        self.socket.register(reg, event_loop)?;
        Ok(())
    }

    fn reregister(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()> {
        self.socket.reregister(reg, event_loop)?;
        Ok(())
    }
}

pub struct Handler {
    session_initiator: RwLock<SessionInitiator>,
}

impl Handler {
    pub fn new(
        socket_address: SocketAddr,
        routing_table: Arc<RoutingTable>,
        channel_to_p2p: IoChannel<p2p::Message>,
        filters: Arc<FiltersControl>,
    ) -> Self {
        let session_initiator = RwLock::new(
            SessionInitiator::bind(&socket_address, routing_table, channel_to_p2p, filters)
                .expect("Cannot bind UDP port"),
        );
        Self {
            session_initiator,
        }
    }
}

const RECEIVE_TOKEN: usize = 0;

impl IoHandler<Message> for Handler {
    fn initialize(&self, io: &IoContext<Message>) -> IoHandlerResult<()> {
        io.register_stream(RECEIVE_TOKEN);
        io.register_timer(REFRESH_TIMER_TOKEN, 10_000);
        Ok(())
    }

    fn timeout(&self, io: &IoContext<Message>, timer: TimerToken) -> IoHandlerResult<()> {
        match timer {
            REFRESH_TIMER_TOKEN => {
                io.message(Message::RequestSession(10));
                Ok(())
            }
            BEGIN_OF_REQUEST_TOKEN...END_OF_REQUEST_TOKEN => {
                let mut session_initiator = self.session_initiator.write();
                match session_initiator
                    .requests
                    .restore(timer, None)
                    .expect("restore return error only when the address is specified")
                {
                    None => {}
                    Some(address) => {
                        if session_initiator.requests.manually_connected_address.take(&address).is_some() {
                            cinfo!(NETWORK, "Timeout occurred when connecting to {}", address);
                        } else {
                            cinfo!(NETWORK, "The message to {} is dropped because of timeout", address);
                        }
                        session_initiator.routing_table.remove_node(address);
                    }
                }
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn message(&self, io: &IoContext<Message>, message: &Message) -> IoHandlerResult<()> {
        match message {
            Message::ConnectTo(socket_address) => {
                let mut session_initiator = self.session_initiator.write();
                session_initiator.routing_table.add_candidate(*socket_address);
                session_initiator.create_new_connection(&socket_address, io)?;
                io.update_registration(RECEIVE_TOKEN);
            }
            Message::ManuallyConnectTo(socket_address) => {
                let mut session_initiator = self.session_initiator.write();
                session_initiator.filters.add_to_whitelist(socket_address.ip(), None);
                session_initiator.routing_table.unban(&socket_address);
                session_initiator.routing_table.add_candidate(*socket_address);
                session_initiator.requests.manually_connected_address.insert(*socket_address);
                session_initiator.create_new_connection(&socket_address, io)?;
                io.update_registration(RECEIVE_TOKEN);
            }
            Message::RequestSession(n) => {
                let mut session_initiator = self.session_initiator.write();
                let addresses = session_initiator.routing_table.candidates(*n);
                if !addresses.is_empty() {
                    let _f = finally(|| {
                        io.update_registration(RECEIVE_TOKEN);
                    });
                    for address in addresses {
                        session_initiator.create_new_connection(&address, io)?;
                    }
                }
            }
            Message::PreimportSecret(secret, socket_address) => {
                let mut session_initiator = self.session_initiator.write();
                if !session_initiator.routing_table.preimport_secret(*secret, &socket_address) {
                    cwarn!(NETWORK, "Cannot import the secret key for already connected host");
                }
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
            io.update_registration(stream);
        });
        loop {
            let mut session_initiator = self.session_initiator.write();
            if !session_initiator.read(io)? {
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
            io.update_registration(stream);
        });
        loop {
            let mut session_initiator = self.session_initiator.write();
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
        let session_initiator = self.session_initiator.read();
        session_initiator.register(reg, event_loop)?;
        Ok(())
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
        let session_initiator = self.session_initiator.read();
        session_initiator.reregister(reg, event_loop)?;
        Ok(())
    }

    fn deregister_stream(
        &self,
        _stream: usize,
        _event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        unreachable!()
    }
}
