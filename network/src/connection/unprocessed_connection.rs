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

use std::collections::HashMap;
use std::io;

use cio::IoManager;
use mio::deprecated::EventLoop;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use rlp::UntrustedRlp;

use super::super::session::{Nonce, Session};
use super::connection::{Connection, Error as ConnectionError, Result as ConnectionResult};
use super::message::{HandshakeMessage, Message, SignedMessage};
use super::stream::Stream;

pub struct UnprocessedConnection {
    stream: Stream,
    session: Option<Session>,
    ack: VecDeque<Message>,
}

impl UnprocessedConnection {
    pub fn new(stream: Stream) -> Self {
        Self {
            stream,
            session: None,
            ack: VecDeque::new(),
        }
    }

    pub fn receive(&mut self, registered_sessions: &HashMap<Nonce, Session>) -> ConnectionResult<Option<Nonce>> {
        if let Some(signed_message) = self.stream.read::<SignedMessage>()? {
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

    pub fn process(self) -> Connection {
        let session = self.session.as_ref().expect("Session must exist");
        Connection::new(self.stream, *session.secret(), session.nonce().clone())
    }

    pub fn interest(&self) -> Ready {
        Ready::readable() | UnixReady::hup()
    }

    pub fn register<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.register(&self.stream, reg, self.interest(), PollOpt::edge())
    }

    pub fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.reregister(&self.stream, reg, self.interest(), PollOpt::edge())
    }

    pub fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.deregister(&self.stream)
    }
}
