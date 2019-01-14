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
use std::fmt;
use std::io;
use std::net;

use mio::deprecated::{TryRead, TryWrite};
use mio::event::Evented;
use mio::net::TcpStream;
use mio::{Poll, PollOpt, Ready, Token};
use rlp::{Decodable, DecoderError, Encodable, UntrustedRlp};

use super::SignedMessage;
use crate::session::Session;
use crate::SocketAddr;

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
        result: Vec<u8>,
    },
    ReadLenOfLen {
        bytes: Vec<u8>,
        read_size: usize,
    },
}

struct TryStream<Stream: TryRead + TryWrite + PeerAddr + Shutdown> {
    stream: Stream,
    read: Option<ReadRetry>,
    write: VecDeque<Vec<u8>>,
}

fn parse_len_of_len(bytes: &[u8]) -> usize {
    debug_assert!(bytes[0] > 0xf7);
    let len_of_len = (bytes[0] - 0xf7) as usize;
    debug_assert!(len_of_len <= 8, "Length of length must be less than 8 but {}({})", len_of_len, bytes[0]);

    len_of_len
}

fn parse_len(bytes: &[u8]) -> (usize, usize) {
    if 0xf8 <= bytes[0] {
        let len_of_len = (bytes[0] - 0xf7) as usize;
        parse_long_len(&bytes, len_of_len)
    } else if 0xc0 <= bytes[0] {
        parse_short_len(&bytes)
    } else {
        unreachable!()
    }
}

fn parse_short_len(bytes: &[u8]) -> (usize, usize) {
    (bytes[0] as usize - 0xc0, 1)
}

fn parse_long_len(bytes: &[u8], len_of_len: usize) -> (usize, usize) {
    let mut total_length: usize = 0;
    for i in &bytes[1..=len_of_len] {
        total_length <<= 8;
        total_length |= *i as usize;
    }
    (total_length, 1 + len_of_len)
}

impl<Stream: TryRead + TryWrite + PeerAddr + Shutdown> TryStream<Stream> {
    fn read_len_of_len(&mut self, mut bytes: Vec<u8>, mut read_size: usize) -> io::Result<Option<Vec<u8>>> {
        debug_assert_eq!(None, self.read);

        let len_of_len = parse_len_of_len(&bytes);
        assert!(read_size < len_of_len, "{} should be less than {}", read_size, len_of_len);

        if let Some(new_read_size) = self.stream.try_read(&mut bytes[(1 + read_size)..=len_of_len])? {
            read_size += new_read_size;
        };
        if len_of_len == read_size {
            return Ok(Some(bytes))
        }

        self.read = Some(ReadRetry::ReadLenOfLen {
            bytes,
            read_size,
        });
        Ok(None)
    }

    fn read_len(&mut self) -> Result<Option<(Vec<u8>)>> {
        debug_assert_eq!(None, self.read);
        let mut bytes: Vec<u8> = vec![0];

        if let Some(read_size) = self.stream.try_read(&mut bytes)? {
            if read_size == 0 {
                return Ok(None)
            }
            debug_assert_eq!(1, read_size);
            if 0xf8 <= bytes[0] {
                let len_of_len = (bytes[0] - 0xf7) as usize;
                bytes.resize(1 + len_of_len, 0);
                return Ok(self.read_len_of_len(bytes, 0)?)
            }
            if 0xc0 <= bytes[0] {
                return Ok(Some(bytes))
            }
            let from_socket = self.peer_addr()?;
            cerror!(NETWORK, "Invalid messages({:?}) from {}", bytes, from_socket);
            self.shutdown()?;
            Ok(None)
        } else {
            Ok(None)
        }
    }

    fn read_bytes(&mut self) -> Result<Option<Vec<u8>>> {
        let from_socket = self.peer_addr()?;

        let mut retry_job = None;
        ::std::mem::swap(&mut retry_job, &mut self.read);
        let mut result = match match retry_job {
            None => self.read_len()?,
            Some(ReadRetry::ReadBytes {
                result,
            }) => {
                cdebug!(NETWORK, "Retry the previous reading body from {}.", from_socket);
                Some(result)
            }
            Some(ReadRetry::ReadLenOfLen {
                bytes,
                read_size,
            }) => {
                cdebug!(NETWORK, "Retry the previous reading length from {}.", from_socket);
                self.read_len_of_len(bytes, read_size)?
            }
        } {
            None => return Ok(None),
            Some(result) => (result),
        };

        let (total_length, len_of_len) = parse_len(&result);
        if total_length == 0 {
            return Ok(None)
        }
        let mut remain_length = total_length + len_of_len - result.len();
        let mut bytes: [u8; 1024] = [0; 1024];

        ctrace!(NETWORK, "Read {} bytes from {}", total_length, from_socket);
        while remain_length != 0 {
            let to_be_read = ::std::cmp::min(remain_length, 1024);
            if let Some(read_size) = self.stream.try_read(&mut bytes[0..to_be_read])? {
                result.extend_from_slice(&bytes[..read_size]);
                debug_assert!(remain_length >= read_size);
                remain_length -= read_size;
            } else {
                debug_assert_eq!(None, self.read);
                self.read = Some(ReadRetry::ReadBytes {
                    result,
                });
                cdebug!(NETWORK, "Cannot read data from {}, {} bytes remain.", from_socket, remain_length);
                return Ok(None)
            }
        }
        Ok(Some(result))
    }

    fn write(&mut self) -> Result<bool> {
        debug_assert!(!self.write.is_empty());
        let peer_socket = self.peer_addr()?;
        let mut job = self.write.pop_front().unwrap();
        match self.stream.try_write(&job) {
            Ok(Some(ref n)) if n == &job.len() => {
                ctrace!(NETWORK, "{} bytes sent to {}", n, peer_socket);
                Ok(true)
            }
            Ok(Some(n)) => {
                debug_assert!(n < job.len());
                let sent: Vec<_> = job.drain(..n).collect();
                debug_assert_eq!(n, sent.len());
                ctrace!(NETWORK, "{} bytes sent to {}, {} bytes remain", n, peer_socket, job.len());
                self.write.push_front(job);
                Ok(false)
            }
            Ok(None) => {
                ctrace!(NETWORK, "Cannot send a message to {}, {} bytes remain", peer_socket, job.len());
                self.write.push_front(job);
                Ok(false)
            }
            Err(err) => {
                cdebug!(NETWORK, "Cannot send a message to {}, {} bytes remain : {:?}", peer_socket, job.len(), err);
                self.write.push_front(job);
                Err(err.into())
            }
        }
    }

    fn write_bytes(&mut self, bytes_to_send: Vec<u8>) -> Result<()> {
        self.write.push_back(bytes_to_send);
        self.flush()?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        while !self.write.is_empty() {
            if !self.write()? {
                break
            }
        }
        Ok(())
    }

    fn peer_addr(&self) -> Result<SocketAddr> {
        self.stream.peer_addr()
    }

    fn shutdown(&self) -> io::Result<()> {
        self.stream.shutdown()
    }
}

trait PeerAddr {
    fn peer_addr(&self) -> Result<SocketAddr>;
}

trait Shutdown {
    fn shutdown(&self) -> io::Result<()>;
}

impl PeerAddr for TcpStream {
    fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(self.peer_addr()?.into())
    }
}

impl Shutdown for TcpStream {
    fn shutdown(&self) -> io::Result<()> {
        self.shutdown(net::Shutdown::Both)
    }
}

pub struct Stream {
    try_stream: TryStream<TcpStream>,
}

impl Stream {
    pub fn connect(socket_address: &net::SocketAddr) -> Result<Option<Self>> {
        Ok(match TcpStream::connect(socket_address) {
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
        self.try_stream.write_bytes(bytes.to_vec())?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.try_stream.flush()?;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.try_stream.write.clear();
    }

    fn read_bytes(&mut self) -> Result<Option<Vec<u8>>> {
        self.try_stream.read_bytes()
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

    pub fn flush(&mut self) -> Result<()> {
        self.stream.flush()?;
        Ok(())
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
                write: VecDeque::default(),
            },
        }
    }
}

impl From<Stream> for TcpStream {
    fn from(stream: Stream) -> Self {
        stream.try_stream.stream
    }
}

impl From<SignedStream> for Stream {
    fn from(stream: SignedStream) -> Self {
        stream.stream
    }
}

impl Evented for Stream {
    fn register(&self, poll: &Poll, token: Token, mut interest: Ready, opts: PollOpt) -> io::Result<()> {
        if !self.try_stream.write.is_empty() {
            interest |= Ready::writable();
        }
        self.try_stream.stream.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, mut interest: Ready, opts: PollOpt) -> io::Result<()> {
        if !self.try_stream.write.is_empty() {
            interest |= Ready::writable();
        }
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

#[cfg(test)]
mod test_stream {
    use super::*;

    pub struct TestStream {
        peer_addr: SocketAddr,
        read_stream: VecDeque<Option<Vec<u8>>>,
    }

    impl TestStream {
        pub fn new(peer_addr: SocketAddr) -> Self {
            Self {
                peer_addr,
                read_stream: Default::default(),
            }
        }

        pub fn append(&mut self, bytes: Vec<u8>) {
            self.read_stream.push_back(Some(bytes));
        }

        pub fn append_blank(&mut self) {
            self.read_stream.push_back(None);
        }
    }

    impl TryRead for TestStream {
        fn try_read(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
            let mut values = match self.read_stream.pop_front() {
                Some(Some(values)) => values,
                _ => return Ok(None),
            };
            let len_of_values = values.len();
            let len_of_buffer = buf.len();
            if len_of_values <= len_of_buffer {
                buf[0..len_of_values].copy_from_slice(&values);
                return Ok(Some(len_of_values))
            }

            buf.copy_from_slice(&values[0..len_of_buffer]);
            values.drain(0..len_of_buffer);
            assert_ne!(0, values.len());
            self.read_stream.push_front(Some(values));
            Ok(Some(len_of_buffer))
        }
    }
    impl TryWrite for TestStream {
        fn try_write(&mut self, _buf: &[u8]) -> io::Result<Option<usize>> {
            unimplemented!()
        }
    }
    impl PeerAddr for TestStream {
        fn peer_addr(&self) -> Result<SocketAddr> {
            Ok(self.peer_addr)
        }
    }
    impl Shutdown for TestStream {
        fn shutdown(&self) -> io::Result<()> {
            Ok(())
        }
    }

    pub fn short_message() -> Vec<u8> {
        let message = vec![vec![1]; 10];
        let encoded = message.rlp_bytes();
        encoded.to_vec()
    }

    pub fn long_message() -> Vec<u8> {
        let message = vec![vec![1]; 300];
        let encoded = message.rlp_bytes();
        encoded.to_vec()
    }

    pub fn fill_at_once(stream: &mut TestStream, bytes: Vec<u8>) {
        stream.append(bytes);
    }

    pub fn fill_one_by_one(stream: &mut TestStream, bytes: Vec<u8>) {
        for i in bytes {
            stream.append(vec![i]);
        }
    }

    pub fn fill_one_by_one_with_blank(stream: &mut TestStream, bytes: Vec<u8>) {
        for i in bytes {
            stream.append_blank();
            stream.append(vec![i]);
        }
    }

    #[test]
    fn short_all_at_once() {
        let encoded = short_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_at_once(&mut stream, encoded.clone());
        let mut received = Vec::new();
        received.resize(encoded.len(), 0);
        assert_eq!(Some(encoded.len()), stream.try_read(&mut received).unwrap());
        assert_eq!(&encoded, &received);
    }

    #[test]
    fn long_all_at_once() {
        let encoded = long_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_at_once(&mut stream, encoded.clone());
        stream.append(encoded.to_vec());
        let mut received = Vec::new();
        received.resize(encoded.len(), 0);
        assert_eq!(Some(encoded.len()), stream.try_read(&mut received).unwrap());
        assert_eq!(&encoded, &received);
    }

    #[test]
    fn short_one_by_one() {
        let encoded = short_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_one_by_one(&mut stream, encoded.clone());
        for i in encoded.to_vec() {
            let mut received = vec![0; 1];
            assert_eq!(Some(1), stream.try_read(&mut received).unwrap());
            assert_eq!(received, vec![i]);
        }
    }

    #[test]
    fn long_one_by_one() {
        let encoded = long_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_one_by_one(&mut stream, encoded.clone());
        for i in encoded.to_vec() {
            let mut received = vec![0; 1];
            assert_eq!(Some(1), stream.try_read(&mut received).unwrap());
            assert_eq!(received, vec![i]);
        }
    }

    #[test]
    fn short_one_by_one_with_blank() {
        let encoded = short_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_one_by_one_with_blank(&mut stream, encoded.clone());
        for i in encoded.to_vec() {
            let mut received = vec![0; 1];
            assert_eq!(None, stream.try_read(&mut received).unwrap());
            assert_eq!(Some(1), stream.try_read(&mut received).unwrap());
            assert_eq!(received, vec![i]);
        }
    }

    #[test]
    fn long_one_by_one_with_blank() {
        let encoded = long_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_one_by_one_with_blank(&mut stream, encoded.clone());
        for i in encoded.to_vec() {
            let mut received = vec![0; 1];
            assert_eq!(None, stream.try_read(&mut received).unwrap());
            assert_eq!(Some(1), stream.try_read(&mut received).unwrap());
            assert_eq!(received, vec![i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_stream::*;
    use super::*;

    #[test]
    fn short_message_at_once() {
        let encoded = short_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_at_once(&mut stream, encoded.clone());
        let mut stream = TryStream {
            stream,
            read: None,
            write: VecDeque::default(),
        };
        assert_eq!(Some(encoded), stream.read_bytes().unwrap());
    }

    #[test]
    fn long_message_at_once() {
        let encoded = long_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_at_once(&mut stream, encoded.clone());
        let mut stream = TryStream {
            stream,
            read: None,
            write: VecDeque::default(),
        };
        assert_eq!(Some(encoded), stream.read_bytes().unwrap());
    }

    #[test]
    fn short_message_one_by_one() {
        let encoded = short_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_one_by_one(&mut stream, encoded.clone());
        let mut stream = TryStream {
            stream,
            read: None,
            write: VecDeque::default(),
        };
        assert_eq!(Some(encoded), stream.read_bytes().unwrap());
    }

    #[test]
    fn long_message_one_by_one() {
        let encoded = long_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_one_by_one(&mut stream, encoded.clone());
        let mut stream = TryStream {
            stream,
            read: None,
            write: VecDeque::default(),
        };
        assert_eq!(None, stream.read_bytes().unwrap());
        assert_eq!(Some(encoded), stream.read_bytes().unwrap());
    }

    #[test]
    fn short_message_one_by_one_with_blank() {
        let encoded = short_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_one_by_one_with_blank(&mut stream, encoded.clone());
        let mut stream = TryStream {
            stream,
            read: None,
            write: VecDeque::default(),
        };
        for i in 0..(encoded.len()) {
            assert_eq!(None, stream.read_bytes().unwrap(), "unexpected result in {}th try", i);
        }
        assert_eq!(Some(encoded), stream.read_bytes().unwrap());
    }

    #[test]
    fn long_message_one_by_one_with_blank() {
        let encoded = long_message();
        let mut stream = TestStream::new(SocketAddr::v4(1, 2, 3, 4, 5678));
        fill_one_by_one_with_blank(&mut stream, encoded.clone());
        let mut stream = TryStream {
            stream,
            read: None,
            write: VecDeque::default(),
        };
        for i in 0..=encoded.len() {
            assert_eq!(None, stream.read_bytes().unwrap(), "unexpected result in {}th try", i);
        }
        assert_eq!(Some(encoded), stream.read_bytes().unwrap());
    }
}
