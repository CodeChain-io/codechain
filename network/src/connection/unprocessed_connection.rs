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

use std::collections::{HashMap, VecDeque};

use mio::deprecated::TryRead;
use mio::net::TcpStream;
use rlp::UntrustedRlp;

use super::super::session::{Nonce, Session};
use super::connection::{Connection, Error as ConnectionError, Result as ConnectionResult};
use super::message::{HandshakeMessage, Message, SignedMessage};

pub struct UnprocessedConnection {
    stream: TcpStream,
    session: Option<Session>,
    ack: VecDeque<Message>,
}

impl UnprocessedConnection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            session: None,
            ack: VecDeque::new(),
        }
    }

    pub fn receive(&mut self, registered_sessions: &HashMap<Nonce, Session>) -> ConnectionResult<Option<Nonce>> {
        if let Some(signed_message) = self.receive_signed_message()? {
            let rlp = UntrustedRlp::new(&signed_message.message);
            match rlp.as_val::<Message>()? {
                Message::Handshake(HandshakeMessage::Sync(_version, nonce)) => {
                    let session = registered_sessions.get(&nonce).ok_or(ConnectionError::UnreadySession)?;
                    if !signed_message.is_valid(&session) {
                        return Err(ConnectionError::InvalidSign)
                    }
                    self.session = Some(session.clone());
                    Ok(Some(nonce))
                }
                _ => Err(ConnectionError::UnreadySession),
            }
        } else {
            Ok(None)
        }
    }

    fn receive_signed_message(&mut self) -> ConnectionResult<Option<SignedMessage>> {
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
        Ok(Some(signed_message))
    }

    pub fn process(self) -> Connection {
        let session = self.session.as_ref().expect("Session must exist");
        Connection::new(self.stream, *session.secret(), session.nonce().expect("Session must exist"))
    }

    pub fn session(&self) -> &Option<Session> {
        &self.session
    }

    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }
}
