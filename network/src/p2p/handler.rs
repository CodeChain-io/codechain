// Copyright 2018-2019 Kodebox, Inc.
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

use std::collections::{BTreeSet, HashMap};
use std::iter::FromIterator;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ccrypto::aes::SymmetricCipherError;
use cio::{IoChannel, IoContext, IoHandler, IoHandlerResult, IoManager, StreamToken, TimerToken};
use ckey::NetworkId;
use finally::finally;
use mio::deprecated::EventLoop;
use mio::{PollOpt, Ready, Token};
use parking_lot::{Mutex, RwLock};
use rand::prelude::SliceRandom;
use rand::rngs::OsRng;
use rand::Rng;
use token_generator::TokenGenerator;

use super::connection::{
    EstablishedConnection, IncomingConnection, IncomingMessage, OutgoingConnection, OutgoingMessage,
};
use super::listener::Listener;
use super::{NegotiationMessage, NetworkMessage};
use crate::client::Client;
use crate::session::Session;
use crate::stream::Stream;
use crate::{FiltersControl, NodeId, RoutingTable, SocketAddr};

pub const MAX_INBOUND_CONNECTIONS: usize = 1000;
pub const MAX_OUTBOUND_CONNECTIONS: usize = 1000;
pub const MAX_OUTGOING_CONNECTIONS: usize = 50;
pub const MAX_INCOMING_CONNECTIONS: usize = 10;

const ACCEPT: StreamToken = 0;

const FIRST_INBOUND: StreamToken = ACCEPT + 1000;
const LAST_INBOUND: StreamToken = FIRST_INBOUND + MAX_INBOUND_CONNECTIONS - 1;

const FIRST_OUTBOUND: StreamToken = FIRST_INBOUND + 1000;
const LAST_OUTBOUND: StreamToken = FIRST_OUTBOUND + MAX_OUTBOUND_CONNECTIONS - 1;

const FIRST_INCOMING: StreamToken = FIRST_OUTBOUND + 1000;
const LAST_INCOMING: StreamToken = FIRST_INCOMING + MAX_INCOMING_CONNECTIONS - 1;

const FIRST_OUTGOING: StreamToken = FIRST_INCOMING + 1000;
const LAST_OUTGOING: StreamToken = FIRST_OUTGOING + MAX_OUTGOING_CONNECTIONS - 1;

const CREATE_CONNECTIONS: TimerToken = 0;

const FIRST_WAIT_SYNC: TimerToken = FIRST_INCOMING;
const LAST_WAIT_SYNC: TimerToken = LAST_INCOMING;

const FIRST_WAIT_ACK: TimerToken = FIRST_OUTGOING;
const LAST_WAIT_ACK: TimerToken = LAST_OUTGOING;

const FIRST_TRY_SYNC: TimerToken = FIRST_OUTGOING + 1000;
const LAST_TRY_SYNC: TimerToken = LAST_OUTGOING + 1000;

const CREATE_CONNECTION_INTERVAL: Duration = Duration::from_secs(3);

const RETRY_SYNC_MAX: Duration = Duration::from_secs(10); // T1
const RTT: Duration = Duration::from_secs(10); // T2
const WAIT_SYNC: Duration = Duration::from_secs(30); // T3 >> T1 + RTT

pub struct Handler {
    connecting_lock: Mutex<()>,
    channel: IoChannel<Message>,

    network_id: NetworkId,
    socket_address: SocketAddr,
    listener: Listener,

    inbound_connections: RwLock<HashMap<StreamToken, EstablishedConnection>>,
    outbound_connections: RwLock<HashMap<StreamToken, EstablishedConnection>>,
    incoming_connections: RwLock<HashMap<StreamToken, IncomingConnection>>,
    outgoing_connections: RwLock<HashMap<StreamToken, OutgoingConnection>>,

    inbound_tokens: Mutex<TokenGenerator>,
    outbound_tokens: Mutex<TokenGenerator>,
    incoming_tokens: Mutex<TokenGenerator>,
    outgoing_tokens: Mutex<TokenGenerator>,

    establishing_incoming_session: Mutex<HashMap<StreamToken, (u16, Session)>>,
    establishing_outgoing_session: Mutex<HashMap<StreamToken, Session>>,

    routing_table: Arc<RoutingTable>,
    filters: Arc<FiltersControl>,

    remote_node_ids: Mutex<HashMap<StreamToken, NodeId>>,
    remote_node_ids_reverse: Mutex<HashMap<NodeId, StreamToken>>,

    client: Arc<Client>,

    min_peers: usize,
    max_peers: usize,

    rng: Mutex<OsRng>,
}

impl Handler {
    pub fn try_new(
        channel: IoChannel<Message>,
        network_id: NetworkId,
        socket_address: SocketAddr,
        client: Arc<Client>,
        routing_table: Arc<RoutingTable>,
        filters: Arc<FiltersControl>,
        min_peers: usize,
        max_peers: usize,
    ) -> ::std::result::Result<Self, String> {
        if MAX_INBOUND_CONNECTIONS + MAX_OUTBOUND_CONNECTIONS < max_peers {
            return Err(format!("Max peers must be less than {}", MAX_INBOUND_CONNECTIONS + MAX_OUTBOUND_CONNECTIONS))
        }
        Ok(Self {
            connecting_lock: Default::default(),
            channel,

            network_id,
            socket_address,
            listener: Listener::bind(&socket_address).expect("Cannot listen TCP port"),

            inbound_tokens: Mutex::new(TokenGenerator::new(FIRST_INBOUND, MAX_INBOUND_CONNECTIONS)),
            outbound_tokens: Mutex::new(TokenGenerator::new(FIRST_OUTBOUND, MAX_OUTBOUND_CONNECTIONS)),
            incoming_tokens: Mutex::new(TokenGenerator::new(FIRST_INCOMING, MAX_INCOMING_CONNECTIONS)),
            outgoing_tokens: Mutex::new(TokenGenerator::new(FIRST_OUTGOING, MAX_OUTGOING_CONNECTIONS)),

            inbound_connections: Default::default(),
            outbound_connections: Default::default(),
            incoming_connections: Default::default(),
            outgoing_connections: Default::default(),

            establishing_incoming_session: Default::default(),
            establishing_outgoing_session: Default::default(),

            routing_table,
            filters,

            remote_node_ids: Default::default(),
            remote_node_ids_reverse: Default::default(),

            client,

            min_peers,
            max_peers,

            rng: Mutex::new(OsRng::new().unwrap()),
        })
    }

    pub fn get_port(&self) -> u16 {
        self.socket_address.port()
    }

    pub fn get_peer_count(&self) -> usize {
        let inbound_connections = self.inbound_connections.read();
        let outbound_connections = self.outbound_connections.read();
        let incoming_count = inbound_connections.len();
        let outgoing_count = outbound_connections.len();
        debug_assert!(
            std::usize::MAX - incoming_count >= outgoing_count,
            "incoming: {} outgoing: {}",
            incoming_count,
            outgoing_count
        );
        incoming_count + outgoing_count
    }

    pub fn established_peers(&self) -> Vec<SocketAddr> {
        self.routing_table.established_addresses()
    }

    fn connect(&self, io: &IoContext<Message>, socket_address: SocketAddr) -> IoHandlerResult<()> {
        let ip = socket_address.ip();
        if !self.filters.is_allowed(&ip) {
            self.routing_table.remove(&socket_address);
            return Err(format!("New connection to {} is requested. But it's not allowed", ip).into())
        }

        let initiator_pub_key = if let Some(initiator_pub_key) = self.routing_table.local_public(socket_address) {
            initiator_pub_key
        } else {
            return Ok(())
        };

        if let Some(stream) = Stream::connect(&socket_address)? {
            let mut outgoing_tokens = self.outgoing_tokens.lock();
            let mut outgoing_connections = self.outgoing_connections.write();
            // Please make sure there is no early return after it.
            let initiator_port = self.socket_address.port();
            let con = OutgoingConnection::new(stream, initiator_pub_key, self.network_id, initiator_port)?;
            let token = outgoing_tokens.gen().ok_or("Too many outgoing connections")?;
            let t = outgoing_connections.insert(token, con);
            assert!(t.is_none());
            io.register_stream(token);
            cinfo!(NETWORK, "New connection to {}({})", socket_address, token);
        } else {
            cwarn!(NETWORK, "Cannot create a connection to {}", socket_address);
        }
        Ok(())
    }
}

fn retry_sync_timer(stream: StreamToken) -> TimerToken {
    assert!(FIRST_OUTGOING <= stream && stream <= LAST_OUTGOING, "{} < {} < {}", FIRST_OUTGOING, stream, LAST_OUTGOING);
    stream - FIRST_OUTGOING + FIRST_TRY_SYNC
}

fn retry_sync_stream(timer: TimerToken) -> StreamToken {
    assert!(FIRST_TRY_SYNC <= timer && timer <= LAST_TRY_SYNC, "{} < {} < {}", FIRST_TRY_SYNC, timer, LAST_TRY_SYNC);
    timer - FIRST_TRY_SYNC + FIRST_OUTGOING
}

fn wait_sync_timer(stream: StreamToken) -> TimerToken {
    assert!(FIRST_INCOMING <= stream && stream <= LAST_INCOMING, "{} < {} < {}", FIRST_INCOMING, stream, LAST_INCOMING);
    stream - FIRST_INCOMING + FIRST_WAIT_SYNC
}

fn wait_sync_stream(timer: TimerToken) -> StreamToken {
    assert!(
        FIRST_WAIT_SYNC <= timer && timer <= LAST_WAIT_SYNC,
        "{} < {} < {}",
        FIRST_WAIT_SYNC,
        timer,
        LAST_WAIT_SYNC
    );
    timer - FIRST_WAIT_SYNC + FIRST_INCOMING
}

fn wait_ack_timer(stream: StreamToken) -> TimerToken {
    assert!(FIRST_OUTGOING <= stream && stream <= LAST_OUTGOING, "{} < {} < {}", FIRST_OUTGOING, stream, LAST_OUTGOING);
    stream - FIRST_OUTGOING + FIRST_WAIT_ACK
}

fn wait_ack_stream(timer: TimerToken) -> StreamToken {
    assert!(FIRST_WAIT_ACK <= timer && timer <= LAST_WAIT_ACK, "{} < {} < {}", FIRST_WAIT_ACK, timer, LAST_WAIT_ACK);
    timer - FIRST_WAIT_ACK + FIRST_OUTGOING
}

impl IoHandler<Message> for Handler {
    fn initialize(&self, io: &IoContext<Message>) -> IoHandlerResult<()> {
        io.register_stream(ACCEPT);
        io.register_timer_once(CREATE_CONNECTIONS, CREATE_CONNECTION_INTERVAL);
        Ok(())
    }

    fn timeout(&self, io: &IoContext<Message>, timer: TimerToken) -> IoHandlerResult<()> {
        match timer {
            CREATE_CONNECTIONS => {
                let _l = self.connecting_lock.lock();
                let current_connections = {
                    let inbound_connections = self.inbound_connections.read();
                    let outbound_connections = self.outbound_connections.read();
                    let outgoing_connections = self.outgoing_connections.read();
                    let incoming_connections = self.incoming_connections.read();
                    let current_connections = outbound_connections.len()
                        + inbound_connections.len()
                        + incoming_connections.len()
                        + outgoing_connections.len();
                    if current_connections >= self.min_peers {
                        return Ok(())
                    }
                    current_connections
                };

                let mut candidates = self.routing_table.candidates();
                candidates.shuffle(&mut *self.rng.lock());
                for addr in candidates.into_iter().take(self.min_peers - current_connections) {
                    if let Err(err) = self.connect(io, addr) {
                        cwarn!(NETWORK, "Cannot connect to {}: {:?}", addr, err);
                    }
                }

                io.register_timer_once(CREATE_CONNECTIONS, CREATE_CONNECTION_INTERVAL);
            }
            FIRST_WAIT_SYNC...LAST_WAIT_SYNC => {
                cwarn!(NETWORK, "No sync message from {}", timer);
                io.deregister_stream(wait_sync_stream(timer));
            }
            FIRST_WAIT_ACK...LAST_WAIT_ACK => {
                cwarn!(NETWORK, "No ack message from {}", timer);
                io.deregister_stream(wait_ack_stream(timer));
            }
            FIRST_TRY_SYNC...LAST_TRY_SYNC => {
                let stream = retry_sync_stream(timer);
                let mut outgoing_connections = self.outgoing_connections.write();
                if let Some(con) = outgoing_connections.get_mut(&stream) {
                    let target = *con.peer_addr();
                    let maybe_remote_public = self.routing_table.try_establish(target)?;
                    con.send_sync(maybe_remote_public);
                    io.register_timer_once(wait_ack_timer(stream), RTT);
                    io.update_registration(stream);
                } else {
                    cdebug!(NETWORK, "Cannot retry {} sync", timer);
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    #[allow(clippy::cyclomatic_complexity)]
    fn message(&self, io: &IoContext<Message>, message: Message) -> IoHandlerResult<()> {
        match message {
            Message::RequestConnection(socket_address) => {
                let _l = self.connecting_lock.lock();
                if self.routing_table.is_establishing_or_established(&socket_address) {
                    return Ok(())
                }

                if self.routing_table.unban(socket_address) {
                    cinfo!(NETWORK, "{} is unbanned because a connection is requested", socket_address);
                }

                ctrace!(NETWORK, "Connecting to {}", socket_address);
                self.connect(io, socket_address)?;
            }
            Message::SendExtensionMessage {
                node_id,
                extension_name,
                need_encryption,
                data,
            } => {
                let stream =
                    *self.remote_node_ids_reverse.lock().get(&node_id).ok_or_else(|| Error::InvalidNode(node_id))?;
                match stream {
                    FIRST_OUTBOUND...LAST_OUTBOUND => {
                        let mut outbound_connections = self.outbound_connections.write();
                        if let Some(con) = outbound_connections.get_mut(&stream) {
                            let _f = finally(|| {
                                io.update_registration(stream);
                            });
                            con.enqueue_extension_message(extension_name, need_encryption, data)?;
                        } else {
                            return Err(format!("{} is an invalid stream", stream).into())
                        }
                    }
                    FIRST_INBOUND...LAST_INBOUND => {
                        let mut inbound_connections = self.inbound_connections.write();
                        if let Some(con) = inbound_connections.get_mut(&stream) {
                            let _f = finally(|| {
                                io.update_registration(stream);
                            });
                            con.enqueue_extension_message(extension_name, need_encryption, data)?;
                        } else {
                            return Err(format!("{} is an invalid stream", stream).into())
                        }
                    }
                    _ => unreachable!("{} is an invalid stream", stream),
                }
            }
            Message::Disconnect(socket_address) => {
                if let Some(stream) = self.remote_node_ids_reverse.lock().get(&socket_address.into()) {
                    io.deregister_stream(*stream);
                    cinfo!(NETWORK, "Disconnect {}:{}", socket_address, stream);
                } else {
                    cwarn!(NETWORK, "Cannot disconnect {} because it's already disconnected", socket_address);
                }
                self.routing_table.ban(socket_address);
            }
            Message::ApplyFilters => {
                for addr in self.routing_table.established_addresses() {
                    if !self.filters.is_allowed(&addr.ip()) {
                        if let Some(stream) = self.remote_node_ids_reverse.lock().get(&addr.into()) {
                            io.deregister_stream(*stream);
                            cinfo!(NETWORK, "Filter disconnects {}:{}", addr, stream);
                        } else {
                            cwarn!(NETWORK, "{} is already disconnected", addr);
                        }
                    }
                }
            }
            Message::Established {
                connection,
                is_inbound: true,
            } => {
                let mut inbound_connections = self.inbound_connections.write();
                let mut inbound_tokens = self.inbound_tokens.lock();
                if let Some(token) = inbound_tokens.gen() {
                    let remote_node_id = connection.peer_addr().into();
                    assert_eq!(None, self.remote_node_ids.lock().insert(token, remote_node_id));
                    assert_eq!(None, self.remote_node_ids_reverse.lock().insert(remote_node_id, token));

                    let t = inbound_connections.insert(token, connection);
                    assert!(t.is_none());
                    io.register_stream(token);
                } else {
                    cwarn!(NETWORK, "Cannot establish an inbound connection");
                }
            }
            Message::Established {
                mut connection,
                is_inbound: false,
            } => {
                let mut outbound_tokens = self.outbound_tokens.lock();
                let mut outbound_connections = self.outbound_connections.write();
                if let Some(token) = outbound_tokens.gen() {
                    let peer_addr = *connection.peer_addr();
                    let remote_node_id = peer_addr.into();
                    assert_eq!(None, self.remote_node_ids.lock().insert(token, remote_node_id));
                    assert_eq!(None, self.remote_node_ids_reverse.lock().insert(remote_node_id, token));

                    for (name, versions) in self.client.extension_versions() {
                        connection.enqueue_negotiation_request(name.clone(), versions);
                    }
                    let t = outbound_connections.insert(token, connection);
                    assert!(t.is_none());
                    io.register_stream(token);
                } else {
                    cwarn!(NETWORK, "Cannot establish an outbound connection");
                }
            }
            Message::RegisterTryAck {
                timer,
                timeout,
            } => {
                io.register_timer_once(timer, timeout);
            }
            Message::StartConnect => {
                io.clear_timer(CREATE_CONNECTIONS);
                io.register_timer_once(CREATE_CONNECTIONS, CREATE_CONNECTION_INTERVAL);
            }
        }
        Ok(())
    }

    fn stream_hup(&self, io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            FIRST_INBOUND...LAST_INBOUND => {
                cinfo!(NETWORK, "Hang-up inbound stream({})", stream);
                io.deregister_stream(stream);
            }
            FIRST_OUTBOUND...LAST_OUTBOUND => {
                cinfo!(NETWORK, "Hang-up outbound stream({})", stream);
                io.deregister_stream(stream);
            }
            FIRST_INCOMING...LAST_INCOMING => {
                cinfo!(NETWORK, "Hang-up incoming stream({})", stream);
                io.deregister_stream(stream);
            }
            FIRST_OUTGOING...LAST_OUTGOING => {
                cinfo!(NETWORK, "Hang-up outgoing stream({})", stream);
                io.deregister_stream(stream);
            }
            _ => unreachable!("Unexpected stream on hup: {}", stream),
        }
        Ok(())
    }

    fn stream_readable(&self, io: &IoContext<Message>, stream_token: StreamToken) -> IoHandlerResult<()> {
        match stream_token {
            ACCEPT => {
                let _f = finally(|| {
                    io.update_registration(stream_token);
                });
                while let Some((stream, socket_address)) = self.listener.accept()? {
                    let (mut incoming_connections, mut incoming_tokens) = {
                        let inbound_connections = self.inbound_connections.read();
                        let outbound_connections = self.outbound_connections.read();
                        let incoming_connections = self.incoming_connections.write();
                        let outgoing_connections = self.outgoing_connections.read();
                        let incoming_tokens = self.incoming_tokens.lock();

                        let current_connections = outbound_connections.len()
                            + inbound_connections.len()
                            + incoming_connections.len()
                            + outgoing_connections.len();

                        if self.max_peers < current_connections {
                            cinfo!(
                                NETWORK,
                                "New connection from {} is dropped because there are too many connections({} < {})",
                                socket_address,
                                self.max_peers,
                                current_connections
                            );
                            return Ok(())
                        }
                        (incoming_connections, incoming_tokens)
                    };
                    let ip = socket_address.ip();
                    if !self.filters.is_allowed(&ip) {
                        cwarn!(NETWORK, "P2P connection request from {} is received. But it's not allowed", ip);
                        return Ok(())
                    }
                    let token = incoming_tokens.gen().ok_or("Too many incomming connections")?;
                    // Please make sure there is no early return after it.
                    let t = incoming_connections.insert(token, IncomingConnection::new(stream));
                    assert!(t.is_none());
                    cinfo!(NETWORK, "New connection from {}({})", socket_address, token);
                    io.register_stream(token);
                    io.register_timer_once(wait_sync_timer(token), WAIT_SYNC);
                }
            }
            FIRST_INBOUND...LAST_INBOUND => {
                let mut inbound_connections = self.inbound_connections.write();
                if let Some(con) = inbound_connections.get_mut(&stream_token) {
                    let should_update = AtomicBool::new(true);
                    let _f = finally(|| {
                        if should_update.load(Ordering::SeqCst) {
                            io.update_registration(stream_token);
                        }
                    });
                    match con.receive()? {
                        Some(NetworkMessage::Extension(msg)) => {
                            let remote_node_id = *self.remote_node_ids.lock().get(&stream_token).unwrap_or_else(|| {
                                unreachable!("Node id for {}:{} must exist", stream_token, con.peer_addr())
                            });
                            let unencrypted = msg.unencrypted_data(con.session()).map_err(|e| format!("{:?}", e))?;
                            self.client.on_message(msg.extension_name(), &remote_node_id, &unencrypted);
                        }
                        Some(NetworkMessage::Negotiation(NegotiationMessage::Request {
                            extension_name,
                            extension_versions,
                        })) => {
                            let versions = self
                                .client
                                .extension_versions()
                                .into_iter()
                                .find(|(name, _)| name == &extension_name)
                                .map(|(_, versions)| versions)
                                .ok_or_else(|| format!("{} is not a valid extension name", extension_name))?;
                            let valid_versions: BTreeSet<u64> = BTreeSet::from_iter(versions.into_iter())
                                .intersection(&BTreeSet::from_iter(extension_versions.into_iter()))
                                .cloned()
                                .collect();
                            let version = valid_versions
                                .into_iter()
                                .last()
                                .ok_or_else(|| format!("There is no valid version for {}", extension_name))?;

                            let remote_node_id = *self.remote_node_ids.lock().get(&stream_token).unwrap_or_else(|| {
                                unreachable!("Node id for {}:{} must exist", stream_token, con.peer_addr())
                            });
                            self.client.on_node_added(&extension_name, &remote_node_id, version);
                            con.enqueue_negotiation_response(extension_name, version);
                        }
                        Some(NetworkMessage::Negotiation(NegotiationMessage::Response {
                            ..
                        })) => {
                            should_update.store(false, Ordering::SeqCst);
                            io.deregister_stream(stream_token);
                            return Err(format!(
                                "Inbound connection from {} received a negotiation response message",
                                con.peer_addr()
                            )
                            .into())
                        }
                        None => {
                            should_update.store(false, Ordering::SeqCst);
                        }
                    }
                } else {
                    cdebug!(NETWORK, "Invalid inbound token({}) on read", stream_token);
                }
            }
            FIRST_OUTBOUND...LAST_OUTBOUND => {
                let mut outbound_connections = self.outbound_connections.write();
                if let Some(con) = outbound_connections.get_mut(&stream_token) {
                    let should_update = AtomicBool::new(true);
                    let _f = finally(|| {
                        if should_update.load(Ordering::SeqCst) {
                            io.update_registration(stream_token);
                        }
                    });
                    match con.receive()? {
                        Some(NetworkMessage::Extension(msg)) => {
                            let remote_node_id = *self.remote_node_ids.lock().get(&stream_token).unwrap_or_else(|| {
                                unreachable!("Node id for {}:{} must exist", stream_token, con.peer_addr())
                            });
                            let unencrypted = msg.unencrypted_data(con.session()).map_err(|e| format!("{:?}", e))?;
                            self.client.on_message(msg.extension_name(), &remote_node_id, &unencrypted);
                        }
                        Some(NetworkMessage::Negotiation(NegotiationMessage::Request {
                            ..
                        })) => {
                            should_update.store(false, Ordering::SeqCst);
                            io.deregister_stream(stream_token);
                            return Err(format!(
                                "Outbound connection from {} received a negotiation request message",
                                con.peer_addr()
                            )
                            .into())
                        }
                        Some(NetworkMessage::Negotiation(NegotiationMessage::Response {
                            extension_name,
                            allowed_version,
                        })) => {
                            let remote_node_id = *self.remote_node_ids.lock().get(&stream_token).unwrap_or_else(|| {
                                unreachable!("Node id for {}:{} must exist", stream_token, con.peer_addr())
                            });
                            self.client.on_node_added(&extension_name, &remote_node_id, allowed_version);
                        }
                        None => {
                            should_update.store(false, Ordering::SeqCst);
                        }
                    }
                } else {
                    cdebug!(NETWORK, "Invalid outbound token({}) on read", stream_token);
                }
            }
            FIRST_INCOMING...LAST_INCOMING => {
                let mut incoming_connections = self.incoming_connections.write();
                if let Some(con) = incoming_connections.get_mut(&stream_token) {
                    let should_update = AtomicBool::new(true);
                    let _f = finally(|| {
                        if should_update.load(Ordering::SeqCst) {
                            io.update_registration(stream_token);
                        }
                    });
                    match con.receive()? {
                        Some(OutgoingMessage::Sync1 {
                            initiator_pub_key,
                            network_id,
                            initiator_port,
                        }) => {
                            let from = con.remote_addr(initiator_port)?;
                            if network_id != self.network_id {
                                io.deregister_stream(stream_token);
                                should_update.store(false, Ordering::SeqCst);
                                return Err(format!("An invalid network id({}) from {}", network_id, from).into())
                            }
                            if let Some((encrypted_nonce, local_public, session)) =
                                self.routing_table.set_recipient_establish1(from, initiator_pub_key)?
                            {
                                cinfo!(NETWORK, "Send ack to {}", from);
                                con.send_ack(local_public, encrypted_nonce);
                                let t = self
                                    .establishing_incoming_session
                                    .lock()
                                    .insert(stream_token, (initiator_port, session));
                                assert_eq!(None, t, "Cannot establish {}", initiator_port);
                                io.clear_timer(wait_sync_timer(stream_token));
                                should_update.store(false, Ordering::SeqCst);
                                io.deregister_stream(stream_token);
                            } else {
                                cinfo!(NETWORK, "Send nack to {}", from);
                                con.send_nack();
                                io.clear_timer(wait_sync_timer(stream_token));
                            }
                        }
                        Some(OutgoingMessage::Sync2 {
                            initiator_pub_key,
                            recipient_pub_key,
                            network_id,
                            initiator_port,
                        }) => {
                            let from = con.remote_addr(initiator_port)?;
                            if network_id != self.network_id {
                                should_update.store(false, Ordering::SeqCst);
                                io.deregister_stream(stream_token);
                                return Err(format!("An invalid network id({}) from {}", network_id, from).into())
                            }
                            if let Some((encrypted_nonce, local_public, session)) = self
                                .routing_table
                                .set_recipient_establish2(from, recipient_pub_key, initiator_pub_key)?
                            {
                                con.send_ack(local_public, encrypted_nonce);
                                let t = self
                                    .establishing_incoming_session
                                    .lock()
                                    .insert(stream_token, (initiator_port, session));
                                assert_eq!(None, t, "Cannot establish {}", initiator_port);
                                io.clear_timer(wait_sync_timer(stream_token));
                                should_update.store(false, Ordering::SeqCst);
                                io.deregister_stream(stream_token);
                            } else {
                                io.clear_timer(wait_sync_timer(stream_token));
                                con.send_nack();
                            }
                        }
                        None => {
                            should_update.store(false, Ordering::SeqCst);
                        }
                    }
                } else {
                    cdebug!(NETWORK, "Invalid incoming token({}) on read", stream_token);
                }
            }
            FIRST_OUTGOING...LAST_OUTGOING => {
                let mut outgoing_connections = self.outgoing_connections.write();
                if let Some(con) = outgoing_connections.get_mut(&stream_token) {
                    let should_update = AtomicBool::new(true);
                    let _f = finally(|| {
                        if should_update.load(Ordering::SeqCst) {
                            io.update_registration(stream_token);
                        }
                    });
                    let from = *con.peer_addr();
                    match con.receive()? {
                        Some(IncomingMessage::Ack {
                            recipient_pub_key,
                            encrypted_nonce,
                        }) => {
                            let session = self.routing_table.set_initiator_establish(
                                from,
                                recipient_pub_key,
                                &encrypted_nonce,
                            )?;
                            let t = self.establishing_outgoing_session.lock().insert(stream_token, session);
                            assert_eq!(None, t);
                            io.clear_timer(wait_ack_timer(stream_token));
                            io.clear_timer(retry_sync_timer(stream_token));
                            should_update.store(false, Ordering::SeqCst);
                            io.deregister_stream(stream_token);
                        }
                        Some(IncomingMessage::Nack) => {
                            cinfo!(NETWORK, "Nack from {}", from);
                            self.routing_table.reset_initiator_establish(from)?;
                            let timeout = self.rng.lock().gen_range(Duration::from_millis(1), RETRY_SYNC_MAX);
                            io.clear_timer(wait_ack_timer(stream_token));
                            let timer_token = retry_sync_timer(stream_token);
                            io.clear_timer(timer_token);
                            io.register_timer_once(timer_token, timeout);
                        }
                        None => {
                            should_update.store(false, Ordering::SeqCst);
                        }
                    }
                } else {
                    cdebug!(NETWORK, "Invalid outgoing token({}) on read", stream_token);
                }
            }
            _ => unreachable!("Unexpected stream on read: {}", stream_token),
        }
        Ok(())
    }

    fn stream_writable(&self, _io: &IoContext<Message>, stream: StreamToken) -> IoHandlerResult<()> {
        match stream {
            FIRST_INBOUND...LAST_INBOUND => {
                if let Some(con) = self.inbound_connections.write().get_mut(&stream) {
                    con.flush()?;
                } else {
                    cdebug!(NETWORK, "Invalid inbound token({}) on write", stream);
                }
            }
            FIRST_OUTBOUND...LAST_OUTBOUND => {
                if let Some(con) = self.outbound_connections.write().get_mut(&stream) {
                    con.flush()?;
                } else {
                    cdebug!(NETWORK, "Invalid outbound token({}) on write", stream);
                }
            }
            FIRST_INCOMING...LAST_INCOMING => {
                if let Some(con) = self.incoming_connections.write().get_mut(&stream) {
                    con.flush()?;
                } else {
                    cdebug!(NETWORK, "Invalid incoming token({}) on write", stream);
                }
            }
            FIRST_OUTGOING...LAST_OUTGOING => {
                if let Some(con) = self.outgoing_connections.write().get_mut(&stream) {
                    con.flush()?;
                } else {
                    cdebug!(NETWORK, "Invalid outgoing token({}) on write", stream);
                }
            }
            _ => unreachable!("Unexpected stream on write: {}", stream),
        }
        Ok(())
    }

    fn register_stream(
        &self,
        stream: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        match stream {
            ACCEPT => {
                event_loop.register(&self.listener, reg, Ready::readable(), PollOpt::edge())?;
                ctrace!(NETWORK, "TCP connection starts for {}", self.socket_address);
            }
            FIRST_INBOUND...LAST_INBOUND => {
                if let Some(con) = self.inbound_connections.read().get(&stream) {
                    con.register(reg, event_loop)?;
                    ctrace!(NETWORK, "Inbound connect({}) registered", stream);
                } else {
                    cdebug!(NETWORK, "Invalid inbound token({}) on register", stream);
                }
            }
            FIRST_OUTBOUND...LAST_OUTBOUND => {
                if let Some(con) = self.outbound_connections.read().get(&stream) {
                    con.register(reg, event_loop)?;
                    ctrace!(NETWORK, "Outbound connect({}) registered", stream);
                } else {
                    cdebug!(NETWORK, "Invalid outbound token({}) on register", stream);
                }
            }
            FIRST_INCOMING...LAST_INCOMING => {
                if let Some(con) = self.incoming_connections.read().get(&stream) {
                    con.register(reg, event_loop)?;
                    ctrace!(NETWORK, "Incoming connect({}) registered", stream);
                } else {
                    cdebug!(NETWORK, "Invalid incoming token({}) on register", stream);
                }
            }
            FIRST_OUTGOING...LAST_OUTGOING => {
                if let Some(con) = self.outgoing_connections.read().get(&stream) {
                    con.register(reg, event_loop)?;
                    ctrace!(NETWORK, "Outgoing connect({}) registered", stream);

                    self.channel.send(Message::RegisterTryAck {
                        timer: retry_sync_timer(reg.0),
                        timeout: Duration::from_secs(0),
                    })?;
                } else {
                    cdebug!(NETWORK, "Invalid outgoing token({}) on register", stream);
                }
            }
            _ => unreachable!("Unexpected stream on register: {}", stream),
        }
        Ok(())
    }

    fn update_stream(
        &self,
        stream: StreamToken,
        reg: Token,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        match stream {
            ACCEPT => {
                event_loop.reregister(&self.listener, reg, Ready::readable(), PollOpt::edge())?;
            }
            FIRST_INBOUND...LAST_INBOUND => {
                if let Some(con) = self.inbound_connections.read().get(&stream) {
                    con.reregister(reg, event_loop)?;
                    ctrace!(NETWORK, "Inbound connect({}) updated", stream);
                } else {
                    cdebug!(NETWORK, "Invalid inbound token({}) on update", stream);
                }
            }
            FIRST_OUTBOUND...LAST_OUTBOUND => {
                if let Some(con) = self.outbound_connections.read().get(&stream) {
                    con.reregister(reg, event_loop)?;
                    ctrace!(NETWORK, "Outbound connect({}) updated", stream);
                } else {
                    cdebug!(NETWORK, "Invalid outbound token({}) on update", stream);
                }
            }
            FIRST_INCOMING...LAST_INCOMING => {
                if let Some(con) = self.incoming_connections.read().get(&stream) {
                    con.reregister(reg, event_loop)?;
                    ctrace!(NETWORK, "Incoming connect({}) updated", stream);
                } else {
                    cdebug!(NETWORK, "Invalid incoming token({}) on update", stream);
                }
            }
            FIRST_OUTGOING...LAST_OUTGOING => {
                if let Some(con) = self.outgoing_connections.read().get(&stream) {
                    con.reregister(reg, event_loop)?;
                    ctrace!(NETWORK, "Outgoing connect({}) updated", stream);
                } else {
                    cdebug!(NETWORK, "Invalid outgoing token({}) on update", stream);
                }
            }
            _ => unreachable!("Unexpected stream on update: {}", stream),
        }
        Ok(())
    }

    fn deregister_stream(
        &self,
        stream: StreamToken,
        event_loop: &mut EventLoop<IoManager<Message>>,
    ) -> IoHandlerResult<()> {
        self.channel.send(Message::StartConnect)?;
        match stream {
            FIRST_INBOUND...LAST_INBOUND => {
                let mut inbound_connections = self.inbound_connections.write();
                let mut inbound_tokens = self.inbound_tokens.lock();
                if let Some(con) = inbound_connections.remove(&stream) {
                    if let Some(node_id) = self.remote_node_ids.lock().remove(&stream) {
                        assert_ne!(None, self.remote_node_ids_reverse.lock().remove(&node_id));
                        self.client.on_node_removed(&node_id);
                    } else {
                        unreachable!("{} has no node id", stream);
                    }
                    con.deregister(event_loop)?;
                    self.routing_table.remove(con.peer_addr());
                    inbound_tokens.restore(stream);
                    ctrace!(NETWORK, "Inbound connect({}) removed", stream);
                } else {
                    cdebug!(NETWORK, "Invalid inbound token({}) on deregister", stream);
                }
            }
            FIRST_OUTBOUND...LAST_OUTBOUND => {
                let mut outbound_connections = self.outbound_connections.write();
                let mut outbound_tokens = self.outbound_tokens.lock();
                if let Some(con) = outbound_connections.remove(&stream) {
                    if let Some(node_id) = self.remote_node_ids.lock().remove(&stream) {
                        assert_ne!(None, self.remote_node_ids_reverse.lock().remove(&node_id));
                        self.client.on_node_removed(&node_id);
                    } else {
                        unreachable!("{} has no node id", stream);
                    }
                    con.deregister(event_loop)?;
                    self.routing_table.remove(con.peer_addr());
                    outbound_tokens.restore(stream);
                    ctrace!(NETWORK, "Outbound connect({}) removed", stream);
                } else {
                    cdebug!(NETWORK, "Invalid outbound token({}) on deregister", stream);
                }
            }
            FIRST_INCOMING...LAST_INCOMING => {
                let mut incoming_connections = self.incoming_connections.write();
                let mut incoming_tokens = self.incoming_tokens.lock();
                let mut establishing_incoming_session = self.establishing_incoming_session.lock();
                if let Some(con) = incoming_connections.remove(&stream) {
                    con.deregister(event_loop)?;
                    incoming_tokens.restore(stream);
                    if let Some((port, session)) = establishing_incoming_session.remove(&stream) {
                        let connection = con.establish(session, port)?;
                        {
                            let peer_addr = connection.peer_addr();
                            if !self.filters.is_allowed(&peer_addr.ip()) {
                                return Err(format!(
                                    "Incoming connection from {} cannot be established because of filter",
                                    peer_addr
                                )
                                .into())
                            }
                        }
                        self.channel.send(Message::Established {
                            connection,
                            is_inbound: true,
                        })?;
                        ctrace!(NETWORK, "Incoming connect({}) established", stream);
                    } else {
                        ctrace!(NETWORK, "Incoming connect({}) removed", stream);
                    }
                } else {
                    cdebug!(NETWORK, "Invalid incoming token({}) on deregister", stream);
                }
            }
            FIRST_OUTGOING...LAST_OUTGOING => {
                let mut outgoing_connections = self.outgoing_connections.write();
                let mut outgoing_tokens = self.outgoing_tokens.lock();
                let mut establishing_outgoing_session = self.establishing_outgoing_session.lock();
                if let Some(con) = outgoing_connections.remove(&stream) {
                    con.deregister(event_loop)?;
                    outgoing_tokens.restore(stream);
                    if let Some(session) = establishing_outgoing_session.remove(&stream) {
                        let connection = con.establish(session)?;
                        {
                            let peer_addr = connection.peer_addr();
                            if !self.filters.is_allowed(&peer_addr.ip()) {
                                return Err(format!(
                                    "Outgoing connection to {} cannot be established because of filter",
                                    peer_addr
                                )
                                .into())
                            }
                        }
                        self.channel.send(Message::Established {
                            connection,
                            is_inbound: false,
                        })?;
                        ctrace!(NETWORK, "Outgoing connect({}) established", stream);
                    } else {
                        self.routing_table.remove(con.peer_addr());
                        ctrace!(NETWORK, "Outgoing connect({}) removed", stream);
                    }
                } else {
                    cdebug!(NETWORK, "Invalid outgoing token({}) on deregister", stream);
                }
            }
            _ => unreachable!("Unexpected stream on deregister: {}", stream),
        }
        Ok(())
    }
}

impl From<SymmetricCipherError> for Error {
    fn from(err: SymmetricCipherError) -> Self {
        Error::SymmetricCipher(err)
    }
}

pub enum Message {
    RequestConnection(SocketAddr),
    SendExtensionMessage {
        node_id: NodeId,
        extension_name: String,
        need_encryption: bool,
        data: Vec<u8>,
    },
    Disconnect(SocketAddr),
    ApplyFilters,
    Established {
        connection: EstablishedConnection,
        is_inbound: bool,
    },
    RegisterTryAck {
        timer: TimerToken,
        timeout: Duration,
    },
    StartConnect,
}

#[derive(Debug)]
enum Error {
    InvalidNode(NodeId),
    SymmetricCipher(SymmetricCipherError),
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            Error::InvalidNode(_) => ::std::fmt::Debug::fmt(self, f),
            Error::SymmetricCipher(err) => ::std::fmt::Debug::fmt(&err, f),
        }
    }
}
