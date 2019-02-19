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
use ckey::{NetworkId, Public};
use mio::deprecated::EventLoop;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};

use super::{EstablishedConnection, IncomingMessage, OutgoingMessage, Result};
use crate::session::Session;
use crate::stream::Stream;
use crate::SocketAddr;

pub struct OutgoingConnection {
    stream: Stream,
    initiator_pub_key: Public,
    network_id: NetworkId,
    initiator_port: u16,
    peer_addr: SocketAddr,
}

impl OutgoingConnection {
    pub fn new(stream: Stream, initiator_pub_key: Public, network_id: NetworkId, initiator_port: u16) -> Result<Self> {
        let peer_addr = stream.peer_addr()?;
        Ok(Self {
            stream,
            initiator_pub_key,
            network_id,
            initiator_port,
            peer_addr,
        })
    }

    fn interest(&self) -> Ready {
        Ready::writable() | Ready::readable() | UnixReady::hup()
    }

    pub fn send_sync(&mut self, recipient_pub_key: Option<Public>) {
        if let Some(recipient_pub_key) = recipient_pub_key {
            self.stream.write(&OutgoingMessage::Sync2 {
                initiator_pub_key: self.initiator_pub_key,
                network_id: self.network_id,
                initiator_port: self.initiator_port,
                recipient_pub_key,
            });
        } else {
            self.stream.write(&OutgoingMessage::Sync1 {
                initiator_pub_key: self.initiator_pub_key,
                network_id: self.network_id,
                initiator_port: self.initiator_port,
            });
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Option<IncomingMessage>> {
        Ok(self.stream.read()?)
    }

    pub fn peer_addr(&self) -> &SocketAddr {
        &self.peer_addr
    }

    pub fn establish(self, session: Session) -> Result<EstablishedConnection> {
        let peer_addr = self.stream.peer_addr()?;
        Ok(EstablishedConnection::new(self.stream, session, peer_addr))
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
