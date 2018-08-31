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
use std::io::{self, Write};
use std::net;

use mio::deprecated::TryRead;
use mio::event::Evented;
use mio::net::TcpStream;
use mio::{Poll, PollOpt, Ready, Token};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};

use super::super::session::Session;
use super::super::SocketAddr;
use super::SignedMessage;

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    DecoderError(DecoderError),
    InvalidSign,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IoError(err) => err.fmt(f),
            Error::DecoderError(err) => err.fmt(f),
            Error::InvalidSign => fmt::Debug::fmt(&self, f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::IoError(err) => err.description(),
            Error::DecoderError(err) => err.description(),
            Error::InvalidSign => "invalid sign",
        }
    }
    fn cause(&self) -> Option<&StdError> {
        match self {
            Error::IoError(err) => Some(err),
            Error::DecoderError(err) => Some(err),
            Error::InvalidSign => None,
        }
    }
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

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug, PartialEq)]
enum ReadRetry {
    ReadBytes {
        total_length: usize,
        result: Vec<u8>,
    },
    ReadLenOfLen {
        bytes: Vec<u8>,
    },
}

struct TryStream {
    stream: TcpStream,
    read: Option<ReadRetry>,
}

impl TryStream {
    fn read_len_of_len(&mut self, mut bytes: Vec<u8>) -> io::Result<(usize, Vec<u8>)> {
        debug_assert_eq!(None, self.read);
        debug_assert_eq!(1, bytes.len());
        debug_assert!(bytes[0] >= 0xf7);
        let len_of_len = (bytes[0] - 0xf7) as usize;
        debug_assert!(len_of_len <= 8);
        bytes.resize(1 + len_of_len, 0);

        if let Some(read_size) = self.stream.try_read(&mut bytes[1..(1 + len_of_len)])? {
            debug_assert_eq!(len_of_len, read_size);
            let mut total_length: usize = 0;
            for i in &bytes[1..(1 + len_of_len)] {
                total_length <<= 8;
                total_length |= *i as usize;
            }
            Ok((total_length, bytes))
        } else {
            let from_socket = self.peer_addr().unwrap();
            cdebug!(NETWORK, "Cannot read length from socket({}).", from_socket);
            self.read = Some(ReadRetry::ReadLenOfLen {
                bytes,
            });
            Ok((0, vec![]))
        }
    }

    fn read_len(&mut self) -> io::Result<(usize, Vec<u8>)> {
        debug_assert_eq!(None, self.read);
        let mut bytes: Vec<u8> = vec![0];

        if let Some(read_size) = self.stream.try_read(&mut bytes)? {
            debug_assert_eq!(1, read_size);
            if bytes[0] >= 0xf7 {
                return self.read_len_of_len(bytes)
            }
            if bytes[0] >= 0xc0 {
                return Ok(((bytes[0] - 0xc0) as usize, bytes))
            }
            Ok((0, vec![]))
        } else {
            Ok((0, vec![]))
        }
    }

    fn read_bytes(&mut self) -> io::Result<Option<Vec<u8>>> {
        let from_socket = self.peer_addr().unwrap();

        let (mut total_length, mut result) = {
            let mut retry_job = None;
            ::std::mem::swap(&mut retry_job, &mut self.read);
            match retry_job {
                None => self.read_len()?,
                Some(ReadRetry::ReadBytes {
                    total_length,
                    result,
                }) => {
                    cdebug!(NETWORK, "Retry the previous job from {}. {} bytes remain.", from_socket, total_length);
                    (total_length, result)
                }
                Some(ReadRetry::ReadLenOfLen {
                    bytes,
                }) => {
                    cdebug!(NETWORK, "Retry the previous job from {}.", from_socket);
                    self.read_len_of_len(bytes)?
                }
            }
        };

        if total_length == 0 {
            return Ok(Some(result))
        }
        let mut bytes: [u8; 1024] = [0; 1024];

        ctrace!(NETWORK, "Read {} bytes from {}", total_length, from_socket);
        while total_length != 0 {
            let to_be_read = ::std::cmp::min(total_length, 1024);
            if let Some(read_size) = self.stream.try_read(&mut bytes[0..to_be_read])? {
                result.extend_from_slice(&bytes[..read_size]);
                debug_assert!(total_length >= read_size);
                total_length -= read_size;
            } else {
                debug_assert_eq!(None, self.read);
                self.read = Some(ReadRetry::ReadBytes {
                    total_length,
                    result,
                });
                cdebug!(NETWORK, "Cannot read data from {}, {} bytes remain.", from_socket, total_length);
                return Ok(None)
            }
        }
        Ok(Some(result))
    }

    fn stream(&self) -> &TcpStream {
        &self.stream
    }

    fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(self.stream.peer_addr()?.into())
    }

    fn shutdown(&self) -> io::Result<()> {
        self.stream.shutdown(net::Shutdown::Both)
    }
}

pub struct Stream {
    try_stream: TryStream,
}

impl Stream {
    pub fn connect<'a, S: Into<&'a net::SocketAddr>>(socket_address: S) -> Result<Option<Self>> {
        Ok(match TcpStream::connect(socket_address.into()) {
            Ok(stream) => Some(Self::from(stream)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(e) => Err(e)?,
        })
    }

    pub fn read<M>(&mut self) -> Result<Option<M>>
    where
        M: ?Sized + Decodable, {
        match self.read_bytes()? {
            None => Ok(None),
            Some(ref bytes) if bytes.is_empty() => Ok(None),
            Some(bytes) => {
                let rlp = UntrustedRlp::new(&bytes);
                Ok(Some(rlp.as_val::<M>()?))
            }
        }
    }

    pub fn write<M>(&mut self, message: &M) -> Result<()>
    where
        M: Encodable, {
        let bytes = message.rlp_bytes();
        Ok(self.write_bytes(&bytes)?)
    }

    fn read_bytes(&mut self) -> io::Result<Option<Vec<u8>>> {
        self.try_stream.read_bytes()
    }

    fn write_bytes(&mut self, bytes_to_send: &[u8]) -> io::Result<()> {
        self.try_stream.stream.write_all(&bytes_to_send)
    }

    pub fn stream(&self) -> &TcpStream {
        &self.try_stream.stream()
    }

    pub fn peer_addr(&self) -> Result<SocketAddr> {
        self.try_stream.peer_addr()
    }

    pub fn shutdown(&self) -> io::Result<()> {
        self.try_stream.shutdown()
    }
}

pub struct SignedStream {
    stream: Stream,
    session: Session,
}

impl SignedStream {
    pub fn new(stream: Stream, session: Session) -> Self {
        Self {
            stream,
            session,
        }
    }

    pub fn read<M>(&mut self) -> Result<Option<M>>
    where
        M: ?Sized + Decodable, {
        if let Some(signed) = self.stream.read::<SignedMessage>()? {
            if !signed.is_valid(&self.session) {
                return Err(Error::InvalidSign)
            }
            let rlp = UntrustedRlp::new(&signed.message);
            Ok(Some(rlp.as_val::<M>()?))
        } else {
            Ok(None)
        }
    }

    pub fn write<M>(&mut self, message: &M) -> Result<()>
    where
        M: Encodable, {
        let signed_message = SignedMessage::new(message, &self.session);
        self.stream.write(&signed_message)
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn shutdown(&self) -> io::Result<()> {
        self.stream.shutdown()
    }
}

impl From<TcpStream> for Stream {
    fn from(stream: TcpStream) -> Self {
        Self {
            try_stream: TryStream {
                stream,
                read: None,
            },
        }
    }
}

impl Into<TcpStream> for Stream {
    fn into(self) -> TcpStream {
        self.try_stream.stream
    }
}

impl<'a> Into<&'a TcpStream> for &'a Stream {
    fn into(self) -> &'a TcpStream {
        &self.try_stream.stream()
    }
}

impl Into<Stream> for SignedStream {
    fn into(self) -> Stream {
        self.stream
    }
}

impl Evented for Stream {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.try_stream.stream.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.try_stream.stream.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.try_stream.stream.deregister(poll)
    }
}

impl Evented for SignedStream {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.stream.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.stream.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.stream.deregister(poll)
    }
}
