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

use cio::IoManager;
use mio::deprecated::EventLoop;
use mio::event::Evented;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use rlp::DecoderError;


use super::super::session::Session;
use super::super::NodeId;
use super::message::{Message, Seq, Version};
use super::stream::{Error as StreamError, SignedStream};
use super::{ExtensionMessage, NegotiationMessage};

pub struct EstablishedConnection {
    stream: SignedStream,
    send_queue: VecDeque<Message>,
    next_negotiation_seq: Seq,
    requested_negotiation: HashMap<Seq, String>,
    remote_node_id: NodeId,
}

#[derive(Debug)]
pub enum Error {
    StreamError(StreamError),
    DecoderError(DecoderError),
    UnreadySession,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::StreamError(err) => err.fmt(f),
            Error::DecoderError(err) => err.fmt(f),
            Error::UnreadySession => fmt::Debug::fmt(self, f),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::StreamError(err) => err.description(),
            Error::DecoderError(err) => err.description(),
            Error::UnreadySession => "Session is not ready",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            Error::StreamError(err) => Some(err),
            Error::DecoderError(err) => Some(err),
            Error::UnreadySession => None,
        }
    }
}

impl From<DecoderError> for Error {
    fn from(err: DecoderError) -> Self {
        Error::DecoderError(err)
    }
}

impl From<StreamError> for Error {
    fn from(err: StreamError) -> Self {
        Error::StreamError(err)
    }
}

pub type Result<T> = result::Result<T, Error>;

impl EstablishedConnection {
    pub fn new(stream: SignedStream, remote_node_id: NodeId) -> Self {
        Self {
            stream,
            send_queue: VecDeque::new(),
            next_negotiation_seq: 0,
            requested_negotiation: HashMap::new(),
            remote_node_id,
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

    pub fn remove_requested_negotiation(&mut self, seq: &u64) -> Option<String> {
        self.requested_negotiation.remove(seq)
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
}

pub trait Connection<S: Sized, M: Sized>
where
    S: Evented, {
    fn stream(&self) -> &S;
    fn interest(&self) -> Ready;

    fn send(&mut self) -> Result<bool>;
    fn receive(&mut self) -> Result<Option<M>>;

    fn remote_node_id(&self) -> Option<NodeId>;
    fn session(&self) -> Option<Session>;

    fn register<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.register(self.stream(), reg, self.interest(), PollOpt::edge())
    }

    fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.reregister(self.stream(), reg, self.interest(), PollOpt::edge())
    }

    fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.deregister(self.stream())
    }
}

impl Connection<SignedStream, Message> for EstablishedConnection {
    fn stream(&self) -> &SignedStream {
        &self.stream
    }

    fn interest(&self) -> Ready {
        if self.send_queue.is_empty() {
            Ready::readable() | UnixReady::hup()
        } else {
            Ready::writable() | Ready::readable() | UnixReady::hup()
        }
    }

    fn send(&mut self) -> Result<bool> {
        if let Some(message) = self.send_queue.pop_front() {
            self.stream.write(&message)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn receive(&mut self) -> Result<Option<Message>> {
        Ok(self.stream.read()?)
    }

    fn remote_node_id(&self) -> Option<NodeId> {
        Some(self.remote_node_id.clone())
    }

    fn session(&self) -> Option<Session> {
        Some(self.stream.session().clone())
    }
}
