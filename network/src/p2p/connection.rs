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
use mio::deprecated::EventLoop;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use rlp::DecoderError;
use unexpected::Mismatch;


use super::super::client::Client;
use super::super::extension::{Error as ExtensionError, NodeToken};
use super::super::NodeId;
use super::message::{Message, Seq, Version};
use super::stream::{Error as StreamError, SignedStream};
use super::{ExtensionMessage, NegotiationBody, NegotiationMessage};

pub struct Connection {
    stream: SignedStream,
    send_queue: VecDeque<Message>,
    next_negotiation_seq: Seq,
    requested_negotiation: HashMap<Seq, String>,
    peer_node_id: NodeId,
}

#[derive(Debug)]
pub enum Error {
    StreamError(StreamError),
    DecoderError(DecoderError),
    InvalidSign,
    UnreadySession,
    UnexpectedNodeId(Mismatch<NodeId>),
    SymmetricCipherError(SymmetricCipherError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::StreamError(err) => err.fmt(f),
            Error::DecoderError(err) => err.fmt(f),
            Error::InvalidSign => fmt::Debug::fmt(&self, f),
            Error::UnreadySession => fmt::Debug::fmt(&self, f),
            Error::UnexpectedNodeId(_) => fmt::Debug::fmt(&self, f),
            Error::SymmetricCipherError(err) => fmt::Debug::fmt(&err, f),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::StreamError(err) => err.description(),
            Error::DecoderError(err) => err.description(),
            Error::InvalidSign => "Invalid sign",
            Error::UnreadySession => "Unready session",
            Error::UnexpectedNodeId(_) => "Unexpected node id",
            Error::SymmetricCipherError(_) => "Symmetric cipher",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            Error::StreamError(err) => Some(err),
            Error::DecoderError(err) => Some(err),
            Error::InvalidSign => None,
            Error::UnreadySession => None,
            Error::UnexpectedNodeId(_) => None,
            Error::SymmetricCipherError(_) => None,
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
    pub fn new(stream: SignedStream, peer_node_id: NodeId) -> Self {
        Self {
            stream,
            send_queue: VecDeque::new(),
            next_negotiation_seq: 0,
            requested_negotiation: HashMap::new(),
            peer_node_id,
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

    fn enqueue(&mut self, message: Message) {
        self.send_queue.push_back(message);
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

    pub fn enqueue_extension_message(&mut self, extension_name: String, need_encryption: bool, message: &[u8]) {
        const VERSION: u64 = 0;
        let message = if need_encryption {
            match ExtensionMessage::encrypted_from_unencrypted_data(
                extension_name,
                VERSION,
                message,
                self.stream.session(),
            ) {
                Ok(message) => message,
                Err(err) => {
                    cdebug!(NET, "Cannot encrypt message : {:?}", err);
                    return
                }
            }
        } else {
            ExtensionMessage::unencrypted(extension_name, VERSION, &message)
        };
        self.enqueue(Message::Extension(message));
    }

    pub fn receive(&mut self, callback: &ExtensionCallback) -> bool {
        self.receive_internal(&callback).unwrap_or_else(|err| {
            cdebug!(NET, "Cannot receive message {:?}", err);
            false
        })
    }

    fn receive_internal(&mut self, callback: &ExtensionCallback) -> Result<bool> {
        if let Some(message) = self.stream.read()? {
            match message {
                Message::Extension(msg) => {
                    let session = self.stream.session();

                    // FIXME: check version of extension
                    callback.on_message(&msg.extension_name(), &msg.unencrypted_data(session)?);
                    Ok(true)
                }
                Message::Handshake(msg) => {
                    ctrace!(NET, "handshake message received {:?}", msg);
                    unreachable!();
                }
                Message::Negotiation(msg) => {
                    match msg.body() {
                        NegotiationBody::Request {
                            ref extension_name,
                            ..
                        } => {
                            let seq = msg.seq();
                            // FIXME: version negotiation
                            callback.on_negotiated(&extension_name);
                            self.enqueue_negotiation_allowed(seq);
                        }
                        NegotiationBody::Allowed => {
                            let seq = msg.seq();
                            if let Some(name) = self.requested_negotiation.remove(&seq) {
                                callback.on_negotiation_allowed(&name);
                            } else {
                                ctrace!(NET, "Negotiation::Allowed message received from non requested seq");
                            }
                        }
                        NegotiationBody::Denied(_) => {
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

    pub fn peer_node_id(&self) -> &NodeId {
        &self.peer_node_id
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

    fn on_negotiated(&self, name: &String) {
        self.client.on_negotiated(&name, &self.id);
    }

    fn on_negotiation_allowed(&self, name: &String) {
        self.client.on_negotiation_allowed(&name, &self.id);
    }

    #[allow(dead_code)]
    fn on_negotiation_denied(&self, name: &String, error: ExtensionError) {
        self.client.on_negotiation_denied(&name, &self.id, error);
    }

    fn on_message(&self, name: &String, data: &[u8]) {
        self.client.on_message(&name, &self.id, &data);
    }
}
