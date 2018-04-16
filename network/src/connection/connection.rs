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
use std::result;

use ccrypto::aes::SymmetricCipherError;
use cio::IoManager;
use ctypes::Secret;
use mio::deprecated::EventLoop;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use rlp::DecoderError;

use super::super::client::Client;
use super::super::extension::{Error as ExtensionError, NodeToken};
use super::super::session::{Nonce, Session};
use super::message::{Seq, Version};
use super::stream::{Error as StreamError, SignedStream, Stream};
use super::{ApplicationMessage, HandshakeMessage, Message, NegotiationBody, NegotiationMessage};

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum State {
    New,         // create socket
    Requested,   // send sync
    Established, // send ack or receive ack
}

pub struct Connection {
    stream: SignedStream,
    state: State,
    send_queue: VecDeque<Message>,
    next_negotiation_seq: Seq,
    requested_negotiation: HashMap<Seq, String>,
}

#[derive(Debug)]
pub enum Error {
    StreamError(StreamError),
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
            &Error::StreamError(ref err) => err.fmt(f),
            &Error::DecoderError(ref err) => err.fmt(f),
            &Error::InvalidSign => fmt::Debug::fmt(&self, f),
            &Error::InvalidState {
                ..
            } => fmt::Debug::fmt(&self, f),
            &Error::UnreadySession => fmt::Debug::fmt(&self, f),
            &Error::SymmetricCipherError(ref err) => fmt::Debug::fmt(&err, f),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            &Error::StreamError(ref err) => err.description(),
            &Error::DecoderError(ref err) => err.description(),
            &Error::InvalidSign => "Invalid sign",
            &Error::InvalidState {
                ..
            } => "Invalid state",
            &Error::UnreadySession => "Unready session",
            &Error::SymmetricCipherError(_) => "Symmetric cipher",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &Error::StreamError(ref err) => Some(err),
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

impl From<StreamError> for Error {
    fn from(err: StreamError) -> Self {
        Error::StreamError(err)
    }
}

pub type Result<T> = result::Result<T, Error>;

impl Connection {
    pub fn new(stream: Stream, secret: Secret, nonce: Nonce) -> Self {
        Self {
            stream: SignedStream::new(stream.into(), Session::new(secret, nonce)),
            state: State::New,
            send_queue: VecDeque::new(),
            next_negotiation_seq: 0,
            requested_negotiation: HashMap::new(),
        }
    }

    pub fn send(&mut self) -> Result<bool> {
        if let Some(message) = self.send_queue.pop_front() {
            self.stream.write(&message)?;
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
        const VERSION: u64 = 0;
        let message = if need_encryption {
            let session_key = (*self.stream.session().secret(), self.stream.session().nonce().clone());
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
        if let Some(message) = self.stream.read()? {
            match message {
                Message::Application(msg) => {
                    let _ = self.expect_state(State::Established)?;

                    let session_key = (*self.stream.session().secret(), self.stream.session().nonce().clone());

                    // FIXME: check version of application
                    callback.on_message(&msg.extension_name(), &msg.unencrypted_data(&session_key)?);
                    Ok(true)
                }
                Message::Handshake(msg) => {
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
                Message::Negotiation(msg) => {
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
        } else {
            Ok(false)
        }
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

    pub fn session(&self) -> &Session {
        self.stream.session()
    }

    pub fn interest(&self) -> Ready {
        if self.send_queue.is_empty() {
            Ready::readable() | UnixReady::hup()
        } else {
            Ready::writable() | Ready::readable() | UnixReady::hup()
        }
    }

    pub fn register<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.register(&self.stream, reg, self.interest(), PollOpt::edge())
    }

    pub fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.reregister(&self.stream, reg, self.interest(), PollOpt::edge())
    }

    pub fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.deregister(&self.stream)
    }
}

pub struct ExtensionCallback<'a> {
    client: &'a Client,
    id: NodeToken,
}

impl<'a> ExtensionCallback<'a> {
    pub fn new(client: &'a Client, id: NodeToken) -> Self {
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

    #[allow(dead_code)]
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
