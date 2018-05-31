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

use mio::unix::UnixReady;
use mio::Ready;
use rlp::UntrustedRlp;

use super::super::session::Session;
use super::super::NodeId;
use super::super::SocketAddr;
use super::connection::{Connection, Error as ConnectionError, EstablishedConnection, Result as ConnectionResult};
use super::message::{HandshakeMessage, Message, SignedMessage};
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

    pub fn receive(&mut self) -> ConnectionResult<Option<SignedMessage>> {
        if self.state != WaitSyncConnectionState::Created {
            return Ok(None)
        }
        if let Some(signed_message) = self.stream.read::<SignedMessage>()? {
            let message = {
                let rlp = UntrustedRlp::new(&signed_message.message);
                rlp.as_val::<Message>()?
            };

            match &message {
                Message::Handshake(HandshakeMessage::Sync {
                    ..
                }) => Ok(Some(signed_message)),
                _ => Err(ConnectionError::UnreadySession),
            }
        } else {
            Ok(None)
        }
    }

    pub fn ready_session(&mut self, peer_node_id: NodeId, session: Session) {
        debug_assert_eq!(self.state, WaitSyncConnectionState::Created);
        self.peer_node_id = Some(peer_node_id);
        self.session = Some(session);
        self.state = WaitSyncConnectionState::Received;
    }

    pub fn establish(self) -> EstablishedConnection {
        debug_assert_eq!(self.state, WaitSyncConnectionState::Sent);
        let session = self.session.as_ref().expect("Session must exist");
        let peer_node_id = self.peer_node_id.expect("Sync message set peer node id");
        EstablishedConnection::new(SignedStream::new(self.stream, session.clone()), peer_node_id)
    }

    pub fn remote_addr(&self) -> ConnectionResult<SocketAddr> {
        Ok(self.stream.peer_addr()?)
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

    pub fn receive(&mut self) -> ConnectionResult<Option<Message>> {
        if self.state != WaitAckConnectionState::Sent {
            return Ok(None)
        }
        if let Some(message) = self.stream.read()? {
            match message {
                Message::Handshake(HandshakeMessage::Ack(_)) => {
                    self.state = WaitAckConnectionState::Received;
                    Ok(Some(message))
                }
                _ => Err(ConnectionError::UnreadySession),
            }
        } else {
            Ok(None)
        }
    }

    pub fn establish(self) -> EstablishedConnection {
        debug_assert_eq!(WaitAckConnectionState::Received, self.state);
        let peer_node_id = self.peer_node_id;
        EstablishedConnection::new(self.stream, peer_node_id)
    }
}

impl Connection<SignedStream> for WaitAckConnection {
    fn stream(&self) -> &SignedStream {
        &self.stream
    }

    fn interest(&self) -> Ready {
        match self.state {
            WaitAckConnectionState::Created => Ready::writable() | UnixReady::hup(),
            WaitAckConnectionState::Sent => Ready::readable() | UnixReady::hup(),
            WaitAckConnectionState::Received => Ready::empty() | UnixReady::hup(),
        }
    }

    fn send(&mut self) -> ConnectionResult<bool> {
        if self.state != WaitAckConnectionState::Created {
            return Ok(false)
        }

        self.stream.write(&Message::Handshake(HandshakeMessage::sync(self.port, self.local_node_id.clone())))?;
        self.state = WaitAckConnectionState::Sent;
        Ok(false)
    }
}

impl Connection<Stream> for WaitSyncConnection {
    fn stream(&self) -> &Stream {
        &self.stream
    }

    fn interest(&self) -> Ready {
        match self.state {
            WaitSyncConnectionState::Created => Ready::readable() | UnixReady::hup(),
            WaitSyncConnectionState::Received => Ready::writable() | UnixReady::hup(),
            WaitSyncConnectionState::Sent => Ready::empty() | UnixReady::hup(),
        }
    }

    fn send(&mut self) -> ConnectionResult<bool> {
        if self.state != WaitSyncConnectionState::Received {
            return Ok(false)
        }

        let session = self.session.as_ref().expect("Session must exist");
        let message = Message::Handshake(HandshakeMessage::ack());
        let signed_message = SignedMessage::new(&message, session);

        self.stream.write(&signed_message)?;
        self.state = WaitSyncConnectionState::Sent;
        Ok(false)
    }
}
