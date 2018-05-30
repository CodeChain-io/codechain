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

use cio::IoManager;
use mio::deprecated::EventLoop;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use rlp::UntrustedRlp;
use unexpected::Mismatch;

use super::super::addr::convert_to_node_id;
use super::super::session::{Nonce, Session};
use super::super::NodeId;
use super::connection::{Error as ConnectionError, EstablishedConnection, Result as ConnectionResult};
use super::message::{HandshakeMessage, Message, SignedMessage};
use super::session_candidate::SessionCandidate;
use super::stream::{SignedStream, Stream};

#[derive(Debug, PartialEq)]
enum WaitSyncConnectionState {
    Created,
    Received,
    Sent,
}

pub struct WaitSyncConnection {
    stream: Stream,
    session: Option<Session>,
    peer_node_id: Option<NodeId>,
    state: WaitSyncConnectionState,
}

impl WaitSyncConnection {
    pub fn new(stream: Stream) -> Self {
        Self {
            stream,
            session: None,
            peer_node_id: None,
            state: WaitSyncConnectionState::Created,
        }
    }

    pub fn send(&mut self) -> ConnectionResult<()> {
        if self.state != WaitSyncConnectionState::Received {
            return Ok(())
        }

        let session = self.session.as_ref().expect("Session must exist");
        let message = Message::Handshake(HandshakeMessage::ack());
        let signed_message = SignedMessage::new(&message, session);

        self.stream.write(&signed_message)?;
        self.state = WaitSyncConnectionState::Sent;
        Ok(())
    }

    pub fn receive(&mut self, registered_sessions: &SessionCandidate) -> ConnectionResult<Option<Nonce>> {
        if self.state != WaitSyncConnectionState::Created {
            return Ok(None)
        }
        if let Some(signed_message) = self.stream.read::<SignedMessage>()? {
            let rlp = UntrustedRlp::new(&signed_message.message);
            match rlp.as_val::<Message>()? {
                Message::Handshake(HandshakeMessage::Sync {
                    port,
                    node_id,
                    ..
                }) => {
                    let peer_addr = self.stream.peer_addr()?;
                    let peer_node_id = convert_to_node_id(&peer_addr.ip(), port);

                    if peer_node_id != node_id {
                        return Err(ConnectionError::UnexpectedNodeId(Mismatch {
                            expected: peer_node_id,
                            found: node_id,
                        }))
                    }
                    let &(ref session, _) =
                        registered_sessions.get(&peer_node_id).ok_or(ConnectionError::UnreadySession)?;
                    if !signed_message.is_valid(&session) {
                        return Err(ConnectionError::InvalidSign)
                    }
                    self.peer_node_id = Some(peer_node_id);
                    self.session = Some(session.clone());
                    self.state = WaitSyncConnectionState::Received;
                    Ok(Some(session.id().clone()))
                }
                _ => Err(ConnectionError::UnreadySession),
            }
        } else {
            Ok(None)
        }
    }

    pub fn establish(self) -> EstablishedConnection {
        debug_assert_eq!(self.state, WaitSyncConnectionState::Sent);
        let session = self.session.as_ref().expect("Session must exist");
        let peer_node_id = self.peer_node_id.expect("Sync message set peer node id");
        EstablishedConnection::new(SignedStream::new(self.stream, session.clone()), peer_node_id)
    }

    fn interest(&self) -> Ready {
        match self.state {
            WaitSyncConnectionState::Created => Ready::readable() | UnixReady::hup(),
            WaitSyncConnectionState::Received => Ready::writable() | UnixReady::hup(),
            WaitSyncConnectionState::Sent => Ready::empty() | UnixReady::hup(),
        }
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

#[derive(Debug, PartialEq)]
enum WaitAckConnectionState {
    Created,
    Sent,
    Received,
}

pub struct WaitAckConnection {
    stream: SignedStream,
    port: u16,
    local_node_id: NodeId,
    peer_node_id: NodeId,
    state: WaitAckConnectionState,
}

impl WaitAckConnection {
    pub fn new(stream: Stream, session: Session, port: u16, local_node_id: NodeId, peer_node_id: NodeId) -> Self {
        Self {
            stream: SignedStream::new(stream, session),
            port,
            local_node_id,
            peer_node_id,
            state: WaitAckConnectionState::Created,
        }
    }

    pub fn send(&mut self) -> ConnectionResult<()> {
        if self.state == WaitAckConnectionState::Created {
            self.stream.write(&Message::Handshake(HandshakeMessage::sync(self.port, self.local_node_id.clone())))?;
            self.state = WaitAckConnectionState::Sent;
        }
        Ok(())
    }

    pub fn receive(&mut self) -> ConnectionResult<bool> {
        if self.state != WaitAckConnectionState::Sent {
            return Ok(false)
        }
        if let Some(message) = self.stream.read()? {
            match message {
                Message::Handshake(HandshakeMessage::Ack(_)) => {
                    self.state = WaitAckConnectionState::Received;
                    Ok(true)
                }
                _ => Err(ConnectionError::UnreadySession),
            }
        } else {
            Ok(false)
        }
    }

    pub fn establish(self) -> EstablishedConnection {
        debug_assert_eq!(WaitAckConnectionState::Received, self.state);
        let peer_node_id = self.peer_node_id;
        EstablishedConnection::new(self.stream, peer_node_id)
    }

    fn interest(&self) -> Ready {
        match self.state {
            WaitAckConnectionState::Created => Ready::writable() | UnixReady::hup(),
            WaitAckConnectionState::Sent => Ready::readable() | UnixReady::hup(),
            WaitAckConnectionState::Received => Ready::empty() | UnixReady::hup(),
        }
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
