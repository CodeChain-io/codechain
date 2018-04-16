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

use std::error::Error as StdError;
use std::fmt;
use std::io;

use mio::event::Evented;
use mio::net::UdpSocket;
use mio::{Poll, PollOpt, Ready, Token};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};

use super::super::SocketAddr;

#[derive(Debug)]
pub enum Error {
    Decoder(DecoderError),
    Io(io::Error),
    InsufficientSent {
        message_length: usize,
        sent_length: usize,
    },
}

pub type Result<T> = ::std::result::Result<T, Error>;

pub struct Socket {
    socket: UdpSocket,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Io(ref err) => err.fmt(f),
            &Error::Decoder(ref err) => err.fmt(f),
            &Error::InsufficientSent {
                ..
            } => fmt::Debug::fmt(&self, f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            &Error::Io(ref err) => err.description(),
            &Error::Decoder(ref err) => err.description(),
            &Error::InsufficientSent {
                ..
            } => "insufficient sent",
        }
    }
    fn cause(&self) -> Option<&StdError> {
        match self {
            &Error::Io(ref err) => Some(err),
            &Error::Decoder(ref err) => Some(err),
            &Error::InsufficientSent {
                ..
            } => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<DecoderError> for Error {
    fn from(err: DecoderError) -> Self {
        Error::Decoder(err)
    }
}
const MAX_PACKET_SIZE: usize = 1024;
impl Socket {
    pub fn bind(socket_address: &SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(socket_address.into())?;
        Ok(Self {
            socket,
        })
    }

    fn write_bytes(&self, message: &[u8], target: &SocketAddr) -> Result<usize> {
        Ok(self.socket.send_to(&message, target.into())?)
    }

    fn read_bytes(&self) -> Result<Option<(Vec<u8>, SocketAddr)>> {
        let mut buf: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];
        let result = match self.socket.recv_from(&mut buf) {
            Ok((received_size, socket_address)) => {
                let socket_address = From::from(socket_address);
                let mut result: Vec<u8> = Vec::new();
                result.extend_from_slice(&buf[..received_size]);
                debug_assert_ne!(0, result.len());
                Ok(Some((result, socket_address)))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }?;
        Ok(result)
    }

    pub fn write<M>(&self, message: &M, target: &SocketAddr) -> Result<()>
    where
        M: Encodable, {
        let bytes = message.rlp_bytes();
        let message_length = bytes.len();
        debug_assert!(message_length < MAX_PACKET_SIZE);

        let sent_length = self.write_bytes(&bytes, &target)?;
        if sent_length == message_length {
            Ok(())
        } else {
            // FIXME: Repeat sent remains when the socket is writable again
            Err(Error::InsufficientSent {
                message_length,
                sent_length,
            })
        }
    }

    pub fn read<M>(&self) -> Result<Option<(M, SocketAddr)>>
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

impl From<UdpSocket> for Socket {
    fn from(socket: UdpSocket) -> Self {
        Self {
            socket,
        }
    }
}

impl Into<UdpSocket> for Socket {
    fn into(self) -> UdpSocket {
        self.socket
    }
}

impl Evented for Socket {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.socket.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.socket.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.socket.deregister(poll)
    }
}
