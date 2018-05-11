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

use std::collections::VecDeque;
use std::error::Error as StdError;
use std::fmt;
use std::io;

use cio::IoManager;
use mio::deprecated::EventLoop;
use mio::{PollOpt, Ready, Token};

use super::super::SocketAddr;
use super::message::Message;
use super::socket::{Error as SocketError, Socket};

#[derive(Debug)]
pub enum Error {
    Socket(SocketError),
    Io(io::Error),
    QueueOverflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Socket(err) => err.fmt(f),
            Error::Io(err) => err.fmt(f),
            Error::QueueOverflow => fmt::Debug::fmt(&self, f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::Socket(err) => err.description(),
            Error::Io(err) => err.description(),
            Error::QueueOverflow => "Queue overflow",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self {
            Error::Socket(err) => Some(err),
            Error::Io(err) => Some(err),
            Error::QueueOverflow => None,
        }
    }
}

impl From<SocketError> for Error {
    fn from(err: SocketError) -> Self {
        Error::Socket(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub struct Server {
    socket: Socket,
    queue: VecDeque<(Message, SocketAddr)>,
}

impl Server {
    pub fn bind(socket_address: &SocketAddr) -> Result<Server> {
        let socket = Socket::bind(socket_address)?;
        Ok(Self {
            socket,
            queue: VecDeque::new(),
        })
    }

    pub fn enqueue(&mut self, message: Message, target: SocketAddr) -> Result<()> {
        const MAX_QUEUE_SIZE: usize = 100;

        if self.queue.len() >= MAX_QUEUE_SIZE {
            Err(Error::QueueOverflow)
        } else {
            self.queue.push_back((message, target));
            Ok(())
        }
    }

    // return false if there is no message to be sent
    pub fn send(&mut self) -> Result<bool> {
        if let Some((message, target)) = self.queue.pop_front() {
            self.socket.write(&message, &target)?;
            Ok(!self.queue.is_empty())
        } else {
            Ok(false)
        }
    }

    pub fn receive(&self) -> Result<Option<(Message, SocketAddr)>> {
        Ok(self.socket.read()?)
    }

    fn interest(&self) -> Ready {
        if self.queue.is_empty() {
            Ready::readable()
        } else {
            Ready::readable() | Ready::writable()
        }
    }

    pub fn register<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        Ok(event_loop.register(&self.socket, reg, self.interest(), PollOpt::edge())?)
    }

    pub fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        Ok(event_loop.reregister(&self.socket, reg, self.interest(), PollOpt::edge())?)
    }
}
