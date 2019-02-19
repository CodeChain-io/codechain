// Copyright 2018-2019 Kodebox, Inc.
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

use std::io;

use cio::IoManager;
use mio::deprecated::EventLoop;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};

use super::super::message::{Message, Version};
use super::super::stream::SignedStream;
use super::super::{ExtensionMessage, NegotiationMessage};
use super::Result;
use crate::session::Session;
use crate::stream::Stream;
use crate::SocketAddr;

pub struct EstablishedConnection {
    stream: SignedStream,
    peer_addr: SocketAddr,
}

impl EstablishedConnection {
    pub fn new(stream: Stream, session: Session, peer_addr: SocketAddr) -> Self {
        Self {
            stream: SignedStream::new(stream, session),
            peer_addr,
        }
    }

    fn write(&mut self, message: &Message) {
        self.stream.write(message);
    }

    pub fn enqueue_negotiation_request(&mut self, name: String, extension_versions: Vec<Version>) {
        self.write(&Message::Negotiation(NegotiationMessage::request(name, extension_versions)));
    }

    pub fn enqueue_negotiation_response(&mut self, name: String, version: u64) {
        self.write(&Message::Negotiation(NegotiationMessage::allowed(name, version)));
    }

    pub fn enqueue_extension_message(
        &mut self,
        extension_name: String,
        need_encryption: bool,
        message: Vec<u8>,
    ) -> Result<()> {
        let message = if need_encryption {
            ExtensionMessage::encrypted_from_unencrypted_data(extension_name, &message, self.stream.session())?
        } else {
            ExtensionMessage::unencrypted(extension_name, message)
        };
        self.write(&Message::Extension(message));
        Ok(())
    }

    fn interest(&self) -> Ready {
        Ready::writable() | Ready::readable() | UnixReady::hup()
    }

    pub fn flush(&mut self) -> Result<()> {
        self.stream.flush()?;
        Ok(())
    }

    pub fn peer_addr(&self) -> &SocketAddr {
        &self.peer_addr
    }

    pub fn receive(&mut self) -> Result<Option<Message>> {
        Ok(self.stream.read()?)
    }

    pub fn session(&self) -> &Session {
        self.stream.session()
    }

    pub fn register<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + 'static, {
        event_loop.register(&self.stream, reg, self.interest(), PollOpt::edge())
    }

    pub fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + 'static, {
        event_loop.reregister(&self.stream, reg, self.interest(), PollOpt::edge())
    }

    pub fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + 'static, {
        event_loop.deregister(&self.stream)
    }
}
