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

use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io;
use std::result;

use cio::IoManager;
use mio::deprecated::EventLoop;
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use parking_lot::Mutex;
use rlp::{DecoderError, UntrustedRlp};

use super::super::session::Session;
use super::super::{NodeId, SocketAddr};
use super::message::{HandshakeMessage, Message, Seq, SignedMessage, Version};
use super::stream::{Error as StreamError, SignedStream, Stream};
use super::{ExtensionMessage, NegotiationMessage};

struct EstablishedConnection {
    stream: SignedStream,
    send_queue: VecDeque<Message>,
    next_negotiation_seq: Seq,
    requested_negotiation: HashMap<Seq, String>,
    remote_node_id: NodeId,
}

#[derive(Debug)]
pub enum Error {
    StreamError(StreamError),
    DecoderError(DecoderError),
    UnreadySession,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::StreamError(err) => err.fmt(f),
            Error::DecoderError(err) => err.fmt(f),
            Error::UnreadySession => fmt::Debug::fmt(self, f),
        }
    }
}

impl From<DecoderError> for Error {
    fn from(err: DecoderError) -> Self {
        Error::DecoderError(err)
    }
}

impl From<StreamError> for Error {
    fn from(err: StreamError) -> Self {
        Error::StreamError(err)
    }
}

pub type Result<T> = result::Result<T, Error>;

impl EstablishedConnection {
    fn new(stream: SignedStream, remote_node_id: NodeId) -> Self {
        Self {
            stream,
            send_queue: VecDeque::new(),
            next_negotiation_seq: 0,
            requested_negotiation: HashMap::new(),
            remote_node_id,
        }
    }

    fn disconnect(self) -> DisconnectingConnection {
        DisconnectingConnection::new(self.stream.into())
    }

    fn enqueue(&mut self, message: Message) {
        self.send_queue.push_back(message);
    }

    fn enqueue_negotiation_request(&mut self, name: String, extension_versions: Vec<Version>) {
        let seq = self.next_negotiation_seq;
        self.next_negotiation_seq += 1;
        if let Some(_) = self.requested_negotiation.insert(seq, name.clone()) {
            unreachable!();
        }
        self.enqueue(Message::Negotiation(NegotiationMessage::request(seq, name, extension_versions)));
    }

    fn remove_requested_negotiation(&mut self, seq: &u64) -> Option<String> {
        self.requested_negotiation.remove(seq)
    }

    fn enqueue_negotiation_allowed(&mut self, seq: Seq, version: u64) {
        self.enqueue(Message::Negotiation(NegotiationMessage::allowed(seq, version)));
    }

    fn enqueue_extension_message(&mut self, extension_name: String, need_encryption: bool, message: &[u8]) {
        const VERSION: u64 = 0;
        let message = if need_encryption {
            match ExtensionMessage::encrypted_from_unencrypted_data(
                extension_name,
                VERSION,
                message,
                self.stream.session(),
            ) {
                Ok(message) => message,
                Err(err) => {
                    cdebug!(NETWORK, "Cannot encrypt message : {:?}", err);
                    return
                }
            }
        } else {
            ExtensionMessage::unencrypted(extension_name, VERSION, &message)
        };
        self.enqueue(Message::Extension(message));
    }

    fn stream(&self) -> &SignedStream {
        &self.stream
    }

    fn interest(&self) -> Ready {
        if self.send_queue.is_empty() {
            Ready::readable() | UnixReady::hup()
        } else {
            Ready::writable() | Ready::readable() | UnixReady::hup()
        }
    }

    fn send(&mut self) -> Result<bool> {
        if let Some(message) = self.send_queue.pop_front() {
            self.stream.write(&message)?;
            Ok(false)
        } else {
            self.stream.flush()?;
            Ok(false)
        }
    }

    fn receive(&mut self) -> Result<Option<Message>> {
        Ok(self.stream.read()?)
    }

    fn remote_node_id(&self) -> Option<NodeId> {
        Some(self.remote_node_id)
    }

    fn session(&self) -> Option<Session> {
        Some(*self.stream.session())
    }

    fn register<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.register(self.stream(), reg, self.interest(), PollOpt::edge())
    }

    fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.reregister(self.stream(), reg, self.interest(), PollOpt::edge())
    }

    fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.deregister(self.stream())
    }
}

#[derive(Debug, PartialEq)]
enum WaitState {
    Created,
    Sent,
    Received,
}

struct WaitSyncConnection {
    stream: Stream,
    session: Option<Session>,
    remote_node_id: Option<NodeId>,
    state: WaitState,
}

impl WaitSyncConnection {
    fn new(stream: Stream) -> Self {
        Self {
            stream,
            session: None,
            remote_node_id: None,
            state: WaitState::Created,
        }
    }

    fn ready_session(&mut self, remote_node_id: NodeId, session: Session) {
        debug_assert_eq!(self.state, WaitState::Created);
        self.remote_node_id = Some(remote_node_id);
        self.session = Some(session);
        self.state = WaitState::Received;
    }

    fn establish(self) -> EstablishedConnection {
        debug_assert_eq!(self.state, WaitState::Sent);
        let session = self.session.as_ref().expect("Session must exist");
        let remote_node_id = self.remote_node_id.expect("Sync message set peer node id");
        EstablishedConnection::new(SignedStream::new(self.stream, *session), remote_node_id)
    }

    fn disconnect(self) -> DisconnectingConnection {
        DisconnectingConnection::new(self.stream)
    }

    fn remote_addr(&self) -> Result<SocketAddr> {
        Ok(self.stream.peer_addr()?)
    }

    fn stream(&self) -> &Stream {
        &self.stream
    }

    fn interest(&self) -> Ready {
        match self.state {
            WaitState::Created => Ready::readable() | UnixReady::hup(),
            WaitState::Received => Ready::writable() | UnixReady::hup(),
            WaitState::Sent => Ready::empty() | UnixReady::hup(),
        }
    }

    fn send(&mut self) -> Result<bool> {
        if self.state != WaitState::Received {
            return Ok(false)
        }

        let session = self.session.as_ref().expect("Session must exist");
        let message = Message::Handshake(HandshakeMessage::ack());
        let signed_message = SignedMessage::new(&message, session);

        self.stream.write(&signed_message)?;
        self.state = WaitState::Sent;
        Ok(false)
    }

    fn receive(&mut self) -> Result<Option<SignedMessage>> {
        if self.state != WaitState::Created {
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
                _ => Err(Error::UnreadySession),
            }
        } else {
            Ok(None)
        }
    }

    fn remote_node_id(&self) -> Option<NodeId> {
        self.remote_node_id
    }

    fn register<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.register(self.stream(), reg, self.interest(), PollOpt::edge())
    }

    fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.reregister(self.stream(), reg, self.interest(), PollOpt::edge())
    }

    fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.deregister(self.stream())
    }
}

struct WaitAckConnection {
    stream: SignedStream,
    port: u16,
    local_node_id: NodeId,
    remote_node_id: NodeId,
    state: WaitState,
}

impl WaitAckConnection {
    fn new(stream: Stream, session: Session, port: u16, local_node_id: NodeId, remote_node_id: NodeId) -> Self {
        Self {
            stream: SignedStream::new(stream, session),
            port,
            local_node_id,
            remote_node_id,
            state: WaitState::Created,
        }
    }

    fn establish(self) -> EstablishedConnection {
        debug_assert_eq!(WaitState::Received, self.state);
        let remote_node_id = self.remote_node_id;
        EstablishedConnection::new(self.stream, remote_node_id)
    }

    fn disconnect(self) -> DisconnectingConnection {
        DisconnectingConnection::new(self.stream.into())
    }

    fn stream(&self) -> &SignedStream {
        &self.stream
    }

    fn interest(&self) -> Ready {
        match self.state {
            WaitState::Created => Ready::writable() | UnixReady::hup(),
            WaitState::Sent => Ready::readable() | UnixReady::hup(),
            WaitState::Received => Ready::empty() | UnixReady::hup(),
        }
    }

    fn send(&mut self) -> Result<bool> {
        if self.state != WaitState::Created {
            return Ok(false)
        }

        self.stream.write(&Message::Handshake(HandshakeMessage::sync(self.port, self.local_node_id)))?;
        self.state = WaitState::Sent;
        Ok(false)
    }

    fn receive(&mut self) -> Result<Option<HandshakeMessage>> {
        if self.state != WaitState::Sent {
            return Ok(None)
        }
        if let Some(message) = self.stream.read()? {
            match message {
                Message::Handshake(HandshakeMessage::Ack(version)) => {
                    self.state = WaitState::Received;
                    Ok(Some(HandshakeMessage::Ack(version)))
                }
                _ => Err(Error::UnreadySession),
            }
        } else {
            Ok(None)
        }
    }

    fn remote_node_id(&self) -> Option<NodeId> {
        Some(self.remote_node_id)
    }

    fn register<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.register(self.stream(), reg, self.interest(), PollOpt::edge())
    }

    fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.reregister(self.stream(), reg, self.interest(), PollOpt::edge())
    }

    fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.deregister(self.stream())
    }
}

struct DisconnectingConnection {
    stream: Stream,
}

impl DisconnectingConnection {
    fn new(mut stream: Stream) -> Self {
        stream.clear();
        Self {
            stream,
        }
    }

    fn reregister<Message>(&self, reg: Token, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.reregister(&self.stream, reg, Ready::empty(), PollOpt::edge())
    }

    fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<()>
    where
        Message: Send + Sync + Clone + 'static, {
        event_loop.deregister(&self.stream)
    }
}

#[derive(Debug, PartialEq)]
pub enum ConnectionType {
    None,
    AckWaiting,
    SyncWaiting,
    Established,
    Disconnecting,
}

enum State {
    WaitSync(WaitSyncConnection),
    WaitAck(WaitAckConnection),
    Established(EstablishedConnection),
    Disconnecting(DisconnectingConnection),
    Intermediate, // An intermediate state before established
}

pub struct Connection {
    state: Mutex<Cell<State>>,
}

impl Connection {
    pub fn connect(
        stream: Stream,
        session: Session,
        local_port: u16,
        local_node_id: NodeId,
        remote_node_id: NodeId,
    ) -> Self {
        let connection = WaitAckConnection::new(stream, session, local_port, local_node_id, remote_node_id);
        Self {
            state: Mutex::new(Cell::new(State::WaitAck(connection))),
        }
    }

    pub fn accept(stream: Stream) -> Self {
        let connection = WaitSyncConnection::new(stream);
        Self {
            state: Mutex::new(Cell::new(State::WaitSync(connection))),
        }
    }

    pub fn shutdown(&self) -> io::Result<()> {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(connection) => connection.stream.shutdown(),
            State::WaitSync(connection) => connection.stream.shutdown(),
            State::Established(connection) => connection.stream.shutdown(),
            _ => unreachable!(),
        }
    }

    pub fn set_disconnecting(&self) {
        let state = self.state.lock();
        let old_state = state.replace(State::Intermediate);
        let connection = match old_state {
            State::WaitAck(connection) => connection.disconnect(),
            State::WaitSync(connection) => connection.disconnect(),
            State::Established(connection) => connection.disconnect(),
            _ => unreachable!(),
        };
        state.set(State::Disconnecting(connection));
    }

    pub fn is_disconnecting(&self) -> bool {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::Disconnecting(_) => true,
            _ => false,
        }
    }

    pub fn establish(&self) -> bool {
        let state = self.state.lock();
        let old_state = state.replace(State::Intermediate);
        match old_state {
            State::WaitAck(connection) => {
                state.replace(State::Established(connection.establish()));
                true
            }
            State::WaitSync(connection) => {
                state.replace(State::Established(connection.establish()));
                true
            }
            State::Established(_) => false,
            _ => unreachable!(),
        }
    }

    pub fn register<Message>(
        &self,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> io::Result<ConnectionType>
    where
        Message: Send + Sync + Clone + 'static, {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(connection) => {
                connection.register(reg, event_loop)?;
                Ok(ConnectionType::AckWaiting)
            }
            State::WaitSync(connection) => {
                connection.register(reg, event_loop)?;
                Ok(ConnectionType::SyncWaiting)
            }
            State::Established(connection) => {
                connection.register(reg, event_loop)?;
                Ok(ConnectionType::Established)
            }
            _ => unreachable!(),
        }
    }

    pub fn reregister<Message>(
        &self,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> io::Result<ConnectionType>
    where
        Message: Send + Sync + Clone + 'static, {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(connection) => {
                connection.reregister(reg, event_loop)?;
                Ok(ConnectionType::AckWaiting)
            }
            State::WaitSync(connection) => {
                connection.reregister(reg, event_loop)?;
                Ok(ConnectionType::SyncWaiting)
            }
            State::Established(connection) => {
                connection.reregister(reg, event_loop)?;
                Ok(ConnectionType::Established)
            }
            State::Disconnecting(connection) => {
                ctrace!(NETWORK, "Packet received while disconnecting");
                connection.reregister(reg, event_loop)?;
                Ok(ConnectionType::Disconnecting)
            }
            _ => unreachable!(),
        }
    }

    pub fn deregister<Message>(&self, event_loop: &mut EventLoop<IoManager<Message>>) -> io::Result<ConnectionType>
    where
        Message: Send + Sync + Clone + 'static, {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(connection) => {
                connection.deregister(event_loop)?;
                Ok(ConnectionType::AckWaiting)
            }
            State::WaitSync(connection) => {
                connection.deregister(event_loop)?;
                Ok(ConnectionType::SyncWaiting)
            }
            State::Established(connection) => {
                connection.deregister(event_loop)?;
                Ok(ConnectionType::Established)
            }
            State::Disconnecting(connection) => {
                connection.deregister(event_loop)?;
                Ok(ConnectionType::Disconnecting)
            }
            _ => unreachable!(),
        }
    }

    pub fn send(&self) -> Result<(ConnectionType, bool)> {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(connection) => {
                let remain = connection.send()?;
                Ok((ConnectionType::AckWaiting, remain))
            }
            State::WaitSync(connection) => {
                let remain = connection.send()?;
                Ok((ConnectionType::SyncWaiting, remain))
            }
            State::Established(connection) => {
                let remain = connection.send()?;
                Ok((ConnectionType::Established, remain))
            }
            _ => unreachable!(),
        }
    }

    pub fn receive(&self) -> Result<Option<ReceivedMessage>> {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(connection) => Ok(connection.receive()?.map(|message| match message {
                HandshakeMessage::Ack(version) => ReceivedMessage::Ack {
                    version,
                },
                _ => unreachable!(),
            })),
            State::WaitSync(connection) => Ok(connection.receive()?.map(ReceivedMessage::Sync)),
            State::Established(connection) => Ok(connection.receive()?.map(|message| match message {
                Message::Negotiation(msg) => ReceivedMessage::Negotiation(msg),
                Message::Extension(msg) => ReceivedMessage::Extension(msg),
                _ => unreachable!(),
            })),
            State::Disconnecting(_) => Ok(None),
            _ => unreachable!(),
        }
    }

    pub fn ready_session(&self, remote_node_id: NodeId, session: Session) -> bool {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(_) => false,
            State::WaitSync(connection) => {
                connection.ready_session(remote_node_id, session);
                true
            }
            State::Established(_) => false,
            _ => unreachable!(),
        }
    }

    pub fn enqueue_negotiation_request(&self, name: String, versions: Vec<Version>) -> bool {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(_) => false,
            State::WaitSync(_) => false,
            State::Established(connection) => {
                connection.enqueue_negotiation_request(name, versions);
                true
            }
            _ => unreachable!(),
        }
    }

    pub fn enqueue_negotiation_allowed(&self, seq: u64, version: u64) -> bool {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(_) => false,
            State::WaitSync(_) => false,
            State::Established(connection) => {
                connection.enqueue_negotiation_allowed(seq, version);
                true
            }
            _ => unreachable!(),
        }
    }

    pub fn enqueue_extension_message(&self, extension_name: &String, need_encryption: bool, data: &[u8]) -> bool {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(_) => false,
            State::WaitSync(_) => false,
            State::Established(connection) => {
                connection.enqueue_extension_message(extension_name.clone(), need_encryption, &data);
                true
            }
            _ => unreachable!(),
        }
    }

    pub fn remove_requested_negotiation(&self, seq: &u64) -> Option<String> {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(_) => None,
            State::WaitSync(_) => None,
            State::Established(connection) => connection.remove_requested_negotiation(seq),
            _ => unreachable!(),
        }
    }

    pub fn remote_addr_of_waiting_sync(&self) -> Option<SocketAddr> {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(_) => None,
            State::WaitSync(connection) => connection.remote_addr().ok(),
            State::Established(_) => None,
            _ => unreachable!(),
        }
    }

    pub fn remote_node_id(&self) -> Option<NodeId> {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(connection) => connection.remote_node_id(),
            State::WaitSync(connection) => connection.remote_node_id(),
            State::Established(connection) => connection.remote_node_id(),
            _ => unreachable!(),
        }
    }

    pub fn established_session(&self) -> Option<Session> {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::WaitAck(_) => None,
            State::WaitSync(_) => None,
            State::Established(connection) => connection.session(),
            _ => unreachable!(),
        }
    }

    pub fn is_established(&self) -> bool {
        let mut state = self.state.lock();
        match state.get_mut() {
            State::Intermediate => unreachable!(),
            State::Established(_) => true,
            _ => false,
        }
    }
}

pub enum ReceivedMessage {
    Ack {
        version: u64,
    },
    Sync(SignedMessage),
    Extension(ExtensionMessage),
    Negotiation(NegotiationMessage),
}
