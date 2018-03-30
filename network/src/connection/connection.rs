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
use std::io::{Write, self};
use std::result;

use mio::deprecated::TryRead;
use mio::net::TcpStream;
use rcrypto::symmetriccipher::SymmetricCipherError;
use rlp::{Encodable, DecoderError, UntrustedRlp};

use super::{ApplicationMessage, HandshakeMessage, Message, NegotiationBody, NegotiationMessage};
use super::SignedMessage;
use super::message::{Seq, Version};
use super::super::client::Client;
use super::super::extension::{Error as ExtensionError, NodeId};
use super::super::session::Session;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum State {
    New, // create socket
    Requested, // send sync
    Established, // send ack or receive ack
}

pub struct Connection {
    stream: TcpStream,
    session: Session,
    state: State,
    send_queue: VecDeque<Message>,
    next_negotiation_seq: Seq,
    requested_negotiation: HashMap<Seq, String>
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
    SymmetricCipherError(SymmetricCipherError)
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

type Result<T> = result::Result<T, Error>;

impl Connection {
    pub fn new(stream: TcpStream, session: Session) -> Result<Self> {
        if !session.is_ready() {
            info!("Try to connect with unready session");
            return Err(Error::UnreadySession)
        }
        Ok(Self {
            stream,
            session,
            state: State::New,
            send_queue: VecDeque::new(),
            next_negotiation_seq: 0,
            requested_negotiation: HashMap::new(),
        })
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

    pub fn enqueue_sync(&mut self) {
        const VERSION: u32 = 0;
        self.enqueue(Message::Handshake(HandshakeMessage::Sync(VERSION)));
        self.state = State::Requested;
    }

    pub fn enqueue_ack(&mut self) {
        const VERSION: u32 = 0;
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

        const VERSION: u32 = 0;
        let message = if need_encryption {
            let session_key = (self.session.secret().clone(), self.session.initialization_vector().unwrap());
            match ApplicationMessage::encrypted_from_unencrypted_data(extension_name, VERSION, message, &session_key) {
                Ok(message) => message,
                Err(err) => {
                    info!("Cannot encrypt message : {:?}", err);
                    return
                },
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
        self.receive_message().and_then(|message| {
            match message {
                None => Ok(false),
                Some(Message::Application(msg)) => {
                    let _ = self.expect_state(State::Established)?;

                    debug_assert!(self.session.is_ready());
                    let session_key = (self.session.secret().clone(), self.session.initialization_vector().unwrap());

                    // FIXME: check version of application
                    callback.on_message(&msg.application_name(), &msg.unencrypted_data(&session_key)?);
                    Ok(true)
                },
                Some(Message::Handshake(msg)) => {
                    info!("handshake message received {:?}", msg);
                    match msg {
                        HandshakeMessage::Sync(_version) => {
                            let _ = self.expect_state(State::New)?;
                            self.enqueue_ack();
                        },
                        HandshakeMessage::Ack(_) => {
                            let _ = self.expect_state(State::Requested)?;
                            self.state = State::Established;
                        },
                    }
                    Ok(true)
                },
                Some(Message::Negotiation(msg)) => {
                    let _ = self.expect_state(State::Established)?;
                    match msg.body() {
                        &NegotiationBody::Request {ref application_name, ..} => {
                            let seq = msg.seq();
                            // FIXME: version negotiation
                            callback.on_connected(&application_name);
                            self.enqueue_negotiation_allowed(seq);
                        },
                        &NegotiationBody::Allowed => {
                            let seq = msg.seq();
                            if let Some(name) = self.requested_negotiation.remove(&seq) {
                                callback.on_connection_allowed(&name);
                            } else {
                                info!("Negotiation::Allowed message received from non requested seq");
                            }
                        },
                        &NegotiationBody::Denied(_) => {
                            // FIXME: Call on_connection_denied
                        },
                    };
                    Ok(true)
                },
            }
        })
    }

    fn receive_message(&mut self) -> Result<Option<Message>> {
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
        if !signed_message.is_valid(&self.session) {
            return Err(Error::InvalidSign)
        }
        let rlp = UntrustedRlp::new(&signed_message.message);
        Ok(Some(rlp.as_val::<Message>()?))
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
}
