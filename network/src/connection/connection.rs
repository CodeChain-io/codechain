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
use std::io::{self, Write};
use std::result;

use ccrypto::aes::SymmetricCipherError;
use ctypes::Secret;
use mio::deprecated::TryRead;
use mio::net::TcpStream;
use rlp::{DecoderError, Encodable, UntrustedRlp};

use super::super::SocketAddr;
use super::super::client::Client;
use super::super::extension::{Error as ExtensionError, NodeId};
use super::super::session::{Nonce, Session};
use super::SignedMessage;
use super::message::{Seq, Version};
use super::{ApplicationMessage, HandshakeMessage, Message, NegotiationBody, NegotiationMessage};

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum State {
    New,         // create socket
    Requested,   // send sync
    Established, // send ack or receive ack
}

pub struct Connection {
    stream: TcpStream,
    session: Session,
    state: State,
    send_queue: VecDeque<Message>,
    next_negotiation_seq: Seq,
    requested_negotiation: HashMap<Seq, String>,
}

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    DecoderError(DecoderError),
    InvalidSign,
    InvalidState {
        expected: State,
        actual: State,
    },
    UnreadySession,
    SymmetricCipherError(SymmetricCipherError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::IoError(ref err) => err.fmt(f),
            &Error::DecoderError(ref err) => err.fmt(f),
            &Error::InvalidSign => write!(f, "InvalidSign"),
            &Error::InvalidState {
                ref expected,
                ref actual,
            } => write!(f, "InvalidState expected: {:?}, actual: {:?}", expected, actual),
            &Error::UnreadySession => write!(f, "UnreadySession"),
            &Error::SymmetricCipherError(ref err) => write!(f, "{:?}", err),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Connection Error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &Error::IoError(ref err) => Some(err),
            &Error::DecoderError(ref err) => Some(err),
            &Error::InvalidSign => None,
            &Error::InvalidState {
                ..
            } => None,
            &Error::UnreadySession => None,
            &Error::SymmetricCipherError(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<DecoderError> for Error {
    fn from(err: DecoderError) -> Self {
        Error::DecoderError(err)
    }
}

impl From<SymmetricCipherError> for Error {
    fn from(err: SymmetricCipherError) -> Self {
        Error::SymmetricCipherError(err)
    }
}

pub type Result<T> = result::Result<T, Error>;

impl Connection {
    pub fn new(stream: TcpStream, secret: Secret, nonce: Nonce) -> Self {
        Self {
            stream,
            session: Session::new(secret, nonce),
            state: State::New,
            send_queue: VecDeque::new(),
            next_negotiation_seq: 0,
            requested_negotiation: HashMap::new(),
        }
    }

    pub fn send(&mut self) -> Result<bool> {
        if let Some(message) = self.send_queue.pop_front() {
            if let Some(signed) = SignedMessage::new(message, &self.session) {
                let bytes_to_send = signed.rlp_bytes();

                let _ = self.stream.set_nodelay(true)?;

                self.stream.write_all(&bytes_to_send)?;
            } else {
                info!("Cannot sign the message");
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn enqueue(&mut self, message: Message) {
        self.send_queue.push_back(message);
    }

    pub fn enqueue_sync(&mut self, nonce: Nonce) {
        const VERSION: u64 = 0;
        self.enqueue(Message::Handshake(HandshakeMessage::Sync(VERSION, nonce)));
        self.state = State::Requested;
    }

    pub fn enqueue_ack(&mut self) {
        const VERSION: u64 = 0;
        self.enqueue(Message::Handshake(HandshakeMessage::Ack(VERSION)));
        self.state = State::Established;
    }

    pub fn enqueue_negotiation_request(&mut self, name: String, version: Version) {
        let seq = self.next_negotiation_seq;
        self.next_negotiation_seq += 1;
        if let Some(_) = self.requested_negotiation.insert(seq, name.clone()) {
            unreachable!();
        }
        self.enqueue(Message::Negotiation(NegotiationMessage::request(seq, name, version)));
    }

    pub fn enqueue_negotiation_allowed(&mut self, seq: Seq) {
        self.enqueue(Message::Negotiation(NegotiationMessage::allowed(seq)));
    }

    pub fn enqueue_extension_message(&mut self, extension_name: String, need_encryption: bool, message: Vec<u8>) {
        if !self.session.is_ready() {
            info!("Cannot send extension message since session is not ready");
            return
        }

        const VERSION: u64 = 0;
        let message = if need_encryption {
            let session_key = (self.session.secret().clone(), self.session.initialization_vector().unwrap());
            match ApplicationMessage::encrypted_from_unencrypted_data(extension_name, VERSION, message, &session_key) {
                Ok(message) => message,
                Err(err) => {
                    info!("Cannot encrypt message : {:?}", err);
                    return
                }
            }
        } else {
            ApplicationMessage::unencrypted(extension_name, VERSION, message)
        };
        self.enqueue(Message::Application(message));
    }

    pub fn receive(&mut self, callback: &ExtensionCallback) -> bool {
        self.receive_internal(&callback).unwrap_or_else(|err| {
            info!("Cannot receive message {:?}", err);
            false
        })
    }

    fn receive_internal(&mut self, callback: &ExtensionCallback) -> Result<bool> {
        self.receive_message().and_then(|messages| {
            match messages {
                None => Ok(false),
                Some(Message::Application(msg)) => {
                    let _ = self.expect_state(State::Established)?;

                    debug_assert!(self.session.is_ready());
                    let session_key = (self.session.secret().clone(), self.session.initialization_vector().unwrap());

                    // FIXME: check version of application
                    callback.on_message(&msg.extension_name(), &msg.unencrypted_data(&session_key)?);
                    Ok(true)
                }
                Some(Message::Handshake(msg)) => {
                    info!("handshake message received {:?}", msg);
                    match msg {
                        HandshakeMessage::Sync(_version, _nonce) => {
                            unreachable!(); // This message must be handled in UnprocessedConnection
                        }
                        HandshakeMessage::Ack(_) => {
                            let _ = self.expect_state(State::Requested)?;
                            self.state = State::Established;
                            callback.on_node_added();
                        }
                    }
                    Ok(true)
                }
                Some(Message::Negotiation(msg)) => {
                    let _ = self.expect_state(State::Established)?;
                    match msg.body() {
                        &NegotiationBody::Request {
                            ref application_name,
                            ..
                        } => {
                            let seq = msg.seq();
                            // FIXME: version negotiation
                            callback.on_connected(&application_name);
                            self.enqueue_negotiation_allowed(seq);
                        }
                        &NegotiationBody::Allowed => {
                            let seq = msg.seq();
                            if let Some(name) = self.requested_negotiation.remove(&seq) {
                                callback.on_connection_allowed(&name);
                            } else {
                                info!("Negotiation::Allowed message received from non requested seq");
                            }
                        }
                        &NegotiationBody::Denied(_) => {
                            // FIXME: Call on_connection_denied
                        }
                    };
                    Ok(true)
                }
            }
        })
    }

    fn receive_message(&mut self) -> Result<Option<(Message)>> {
        let mut result: Vec<u8> = Vec::new();
        let mut bytes: [u8; 1024] = [0; 1024];

        loop {
            if let Some(read_size) = self.stream.try_read(&mut bytes)? {
                result.extend_from_slice(&bytes[..read_size]);
            } else {
                break
            }
        }

        if result.len() == 0 {
            return Ok(None)
        }
        let rlp = UntrustedRlp::new(&result);
        let signed_message = rlp.as_val::<SignedMessage>()?;
        let message = {
            let rlp = UntrustedRlp::new(&signed_message.message);
            rlp.as_val::<Message>()?
        };
        if !signed_message.is_valid(&self.session) {
            return Err(Error::InvalidSign)
        }
        Ok(Some(message))
    }

    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }

    fn expect_state(&self, expected: State) -> Result<()> {
        if self.state != expected {
            Err(Error::InvalidState {
                expected,
                actual: self.state.clone(),
            })
        } else {
            Ok(())
        }
    }

    pub fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(SocketAddr::from(self.stream.peer_addr()?))
    }

    pub fn session(&self) -> &Session {
        &self.session
    }
}

pub struct ExtensionCallback<'a> {
    client: &'a Client,
    id: NodeId,
}

impl<'a> ExtensionCallback<'a> {
    pub fn new(client: &'a Client, id: NodeId) -> Self {
        Self {
            client,
            id,
        }
    }

    fn on_connected(&self, name: &String) {
        self.client.on_connected(&name, &self.id);
    }

    fn on_connection_allowed(&self, name: &String) {
        self.client.on_connection_allowed(&name, &self.id);
    }

    fn on_connection_denied(&self, name: &String, error: ExtensionError) {
        self.client.on_connection_denied(&name, &self.id, error);
    }

    fn on_message(&self, name: &String, data: &Vec<u8>) {
        self.client.on_message(&name, &self.id, &data);
    }

    fn on_node_added(&self) {
        self.client.on_node_added(&self.id);
    }
}
