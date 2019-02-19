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
use ckey::Public;
use mio::deprecated::EventLoop;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use primitives::Bytes;

use super::{EstablishedConnection, IncomingMessage, OutgoingMessage, Result};
use crate::session::Session;
use crate::stream::Stream;
use crate::SocketAddr;

pub struct IncomingConnection {
    stream: Stream,
}

impl IncomingConnection {
    pub fn new(stream: Stream) -> Self {
        Self {
            stream,
        }
    }

    pub fn establish(self, session: Session, port: u16) -> Result<EstablishedConnection> {
        let peer_addr = SocketAddr::new(self.stream.peer_addr()?.ip(), port);
        Ok(EstablishedConnection::new(self.stream, session, peer_addr))
    }

    fn interest(&self) -> Ready {
        Ready::writable() | Ready::readable() | UnixReady::hup()
    }

    pub fn send_ack(&mut self, recipient_pub_key: Public, encrypted_nonce: Bytes) {
        self.stream.write(&IncomingMessage::Ack {
            recipient_pub_key,
            encrypted_nonce,
        });
    }

    pub fn send_nack(&mut self) {
        self.stream.write(&IncomingMessage::Nack);
    }

    pub fn flush(&mut self) -> Result<()> {
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Option<OutgoingMessage>> {
        Ok(self.stream.read()?)
    }

    pub fn remote_addr(&self, port: u16) -> Result<SocketAddr> {
        Ok(SocketAddr::new(self.stream.peer_addr()?.ip(), port))
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
