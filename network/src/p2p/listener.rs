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

use std::io;

use mio::event::Evented;
use mio::net::TcpListener;
use mio::{Poll, PollOpt, Ready, Token};

use super::super::SocketAddr;
use super::stream::Stream;

pub struct Listener {
    listener: TcpListener,
}

impl Listener {
    pub fn bind(socket_address: &SocketAddr) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(socket_address)?,
        })
    }

    pub fn accept(&self) -> io::Result<Option<(Stream, SocketAddr)>> {
        Ok(match self.listener.accept() {
            Ok((stream, socket_address)) => Some((From::from(stream), From::from(socket_address))),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(e) => Err(e)?,
        })
    }
}

impl Evented for Listener {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.listener.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.listener.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.listener.deregister(poll)
    }
}
