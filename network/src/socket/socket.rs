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
use std::io;

use cio::IoManager;
use mio::deprecated::EventLoop;
use mio::net::UdpSocket;
use mio::{PollOpt, Ready, Token};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};

use super::message::Message;
use crate::SocketAddr;

#[derive(Debug)]
pub enum Error {
    Decoder(DecoderError),
    Io(io::Error),
    QueueOverflow,
}

impl From<DecoderError> for Error {
    fn from(err: DecoderError) -> Self {
        Error::Decoder(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub struct Socket {
    socket: UdpSocket,
    queue: VecDeque<(Message, SocketAddr)>,
}

const MAX_PACKET_SIZE: usize = 1024;
impl Socket {
    pub fn bind(socket_address: &SocketAddr) -> io::Result<Socket> {
        let socket = UdpSocket::bind(socket_address)?;
        Ok(Self {
            socket,
            queue: VecDeque::new(),
        })
    }

    pub fn send(&mut self, message: Message, target: SocketAddr) -> Result<()> {
        const MAX_QUEUE_SIZE: usize = 100;

        if self.queue.len() >= MAX_QUEUE_SIZE {
            Err(Error::QueueOverflow)
        } else {
            self.queue.push_back((message, target));
            Ok(())
        }
    }

    pub fn flush(&mut self) -> io::Result<()> {
        while let Some((message, target)) = self.queue.pop_front() {
            let result = self.write(&message, &target);
            if let Ok(true) = result {
                continue
            }
            self.queue.push_front((message, target));
            result?;
            break
        }
        Ok(())
    }

    pub fn receive(&self) -> Result<Option<(Message, SocketAddr)>> {
        Ok(self.read()?)
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
        event_loop.register(&self.socket, reg, self.interest(), PollOpt::edge())?;
        Ok(())
    }

    pub fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.reregister(&self.socket, reg, self.interest(), PollOpt::edge())?;
        Ok(())
    }

    fn write_bytes(&self, message: &[u8], target: &SocketAddr) -> io::Result<usize> {
        Ok(self.socket.send_to(&message, target)?)
    }

    fn read_bytes(&self) -> io::Result<Option<(Vec<u8>, SocketAddr)>> {
        let mut buf: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];
        let result = match self.socket.recv_from(&mut buf) {
            Ok((received_size, socket_address)) => {
                let socket_address = From::from(socket_address);
                let mut result: Vec<u8> = Vec::new();
                result.extend_from_slice(&buf[..received_size]);
                debug_assert_ne!(0, result.len());
                Some((result, socket_address))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(e) => return Err(e),
        };
        Ok(result)
    }

    fn write<M>(&self, message: &M, target: &SocketAddr) -> io::Result<bool>
    where
        M: Encodable, {
        let bytes = message.rlp_bytes();
        let message_length = bytes.len();
        debug_assert!(message_length < MAX_PACKET_SIZE);

        let sent_length = self.write_bytes(&bytes, &target)?;
        Ok(sent_length == message_length)
    }

    fn read<M>(&self) -> Result<Option<(M, SocketAddr)>>
    where
        M: ?Sized + Decodable, {
        let result = self.read_bytes()?;
        match result {
            None => Ok(None),
            Some((bytes, target)) => {
                let rlp = UntrustedRlp::new(&bytes);
                Ok(Some((rlp.as_val::<M>()?, target)))
            }
        }
    }
}
