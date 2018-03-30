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
use std::io::{Write, self};
use std::result;

use mio::deprecated::TryRead;
use mio::net::TcpStream;
use rlp::{Encodable, DecoderError, UntrustedRlp};

use super::{HandshakeMessage, Message};
use super::SignedMessage;
use super::super::session::Session;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum State {
    New, // create socket
    Requested, // send sync
    Established, // send ack or receive ack
}

pub struct Connection {
    stream: TcpStream,
    session: Session,
    state: State,
    send_queue: VecDeque<Message>,
}

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    DecoderError(DecoderError),
    InvalidSign,
    InvalidState {
        expected: State,
        actual: State,
    },
    UnreadySession,
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

type Result<T> = result::Result<T, Error>;

impl Connection {
    pub fn new(stream: TcpStream, session: Session) -> Result<Self> {
        if !session.is_ready() {
            info!("Try to connect with unready session");
            return Err(Error::UnreadySession)
        }
        Ok(Self {
            stream,
            session,
            state: State::New,
            send_queue: VecDeque::new(),
        })
    }

    pub fn send(&mut self) -> Result<bool> {
        if let Some(message) = self.send_queue.pop_front() {
            if let Some(signed) = SignedMessage::new(message, &self.session) {
                let bytes_to_send = signed.rlp_bytes();

                let _ = self.stream.set_nodelay(true)?;

                self.stream.write_all(&bytes_to_send)?;
            } else {
                info!("Cannot sign the message");
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn enqueue(&mut self, message: Message) {
        self.send_queue.push_back(message);
    }

    pub fn enqueue_sync(&mut self) {
        const VERSION: u32 = 0;
        self.enqueue(Message::Handshake(HandshakeMessage::Sync(VERSION)));
    }

    pub fn enqueue_ack(&mut self) {
        const VERSION: u32 = 0;
        self.enqueue(Message::Handshake(HandshakeMessage::Ack(VERSION)));
    }

    pub fn receive(&mut self) -> bool {
        self.receive_internal().unwrap_or_else(|err| {
            info!("Cannot receive message {:?}", err);
            false
        })
    }

    fn receive_internal(&mut self) -> Result<bool> {
        self.receive_message().and_then(|message| {
            match message {
                None => Ok(false),
                Some(Message::Application(msg)) => {
                    unimplemented!();
                },
                Some(Message::Handshake(msg)) => {
                    info!("handshake message received {:?}", msg);
                    match msg {
                        HandshakeMessage::Sync(_version) => {
                            let _ = self.expect_state(State::New)?;
                            self.state = State::Requested;
                            self.enqueue_ack();
                        },
                        HandshakeMessage::Ack(_) => {
                            let _ = self.expect_state(State::Requested)?;
                            self.state = State::Established;
                        },
                    }
                    Ok(true)
                },
                Some(Message::Negotiation(msg)) => {
                    unimplemented!();
                },
            }
        })
    }

    fn receive_message(&mut self) -> Result<Option<Message>> {
        let mut result: Vec<u8> = Vec::new();
        let mut bytes: [u8; 1024] = [0; 1024];

        loop {
            if let Some(read_size) = self.stream.try_read(&mut bytes)? {
                result.extend_from_slice(&bytes[..read_size]);
            } else {
                break
            }
        }

        if result.len() == 0 {
            return Ok(None)
        }
        let rlp = UntrustedRlp::new(&result);
        let signed_message = rlp.as_val::<SignedMessage>()?;
        if !signed_message.is_valid(&self.session) {
            return Err(Error::InvalidSign)
        }
        let rlp = UntrustedRlp::new(&signed_message.message);
        Ok(Some(rlp.as_val::<Message>()?))
    }

    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }

    fn expect_state(&self, expected: State) -> Result<()> {
        if self.state != expected {
            Err(Error::InvalidState {
                expected,
                actual: self.state.clone(),
            })
        } else {
            Ok(())
        }
    }
}
