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

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use ccore::BlockChainClient;
use cnetwork::{Api, NetworkExtension, NodeId, TimeoutHandler, TimerToken};
use parking_lot::RwLock;
use primitives::H256;
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use super::message::Message;

const BROADCAST_TIMER_TOKEN: TimerToken = 0;
const BROADCAST_TIMER_INTERVAL: i64 = 1000;
const MAX_HISTORY_SIZE: usize = 100;

struct Peer {
    history_set: HashSet<H256>,
    history_queue: VecDeque<H256>,
}

impl Peer {
    fn new() -> Self {
        Self {
            history_set: HashSet::new(),
            history_queue: VecDeque::new(),
        }
    }

    fn push(&mut self, hash: H256) {
        debug_assert!(!self.history_set.contains(&hash));
        self.history_set.insert(hash);
        self.history_queue.push_back(hash);
        if self.history_queue.len() > MAX_HISTORY_SIZE {
            self.history_queue.pop_front();
        }
    }

    fn contains(&mut self, hash: &H256) -> bool {
        self.history_set.contains(hash)
    }
}

pub struct Extension {
    peers: RwLock<HashMap<NodeId, RwLock<Peer>>>,
    client: Arc<BlockChainClient>,
    api: RwLock<Option<Arc<Api>>>,
}

impl Extension {
    pub fn new(client: Arc<BlockChainClient>) -> Arc<Self> {
        Arc::new(Self {
            peers: RwLock::new(HashMap::new()),
            client,
            api: RwLock::new(None),
        })
    }
}

impl NetworkExtension for Extension {
    fn name(&self) -> &'static str {
        "parcel-propagation"
    }
    fn need_encryption(&self) -> bool {
        false
    }

    fn versions(&self) -> &[u64] {
        const VERSIONS: &[u64] = &[0];
        &VERSIONS
    }

    fn on_initialize(&self, api: Arc<Api>) {
        let mut api_lock = self.api.write();
        api.set_timer(BROADCAST_TIMER_TOKEN, Duration::milliseconds(BROADCAST_TIMER_INTERVAL))
            .expect("Timer set succeeds");
        *api_lock = Some(api);
    }

    fn on_node_added(&self, token: &NodeId, _version: u64) {
        self.peers.write().insert(*token, RwLock::new(Peer::new()));
    }
    fn on_node_removed(&self, token: &NodeId) {
        self.peers.write().remove(token);
    }

    fn on_message(&self, token: &NodeId, data: &[u8]) {
        if let Ok(received_message) = UntrustedRlp::new(data).as_val() {
            match received_message {
                Message::Parcels(parcels) => {
                    self.client.queue_parcels(
                        parcels.iter().map(|unverified| unverified.rlp_bytes().to_vec()).collect(),
                        *token,
                    );
                    let peers = self.peers.read();
                    if let Some(peer) = peers.get(token) {
                        let mut peer = peer.write();
                        let parcels: Vec<_> = parcels
                            .iter()
                            .map(|parcel| parcel.hash())
                            .filter(|parcel| !peer.contains(parcel))
                            .collect();
                        for unverified in parcels.iter() {
                            peer.push(*unverified);
                        }
                        cdebug!(SYNC_PARCEL, "Receive {} parcels from {}", parcels.len(), token);
                        ctrace!(SYNC_PARCEL, "Receive {:?}", parcels);
                    } else {
                        cwarn!(SYNC_PARCEL, "Message from {} but it's already removed", token);
                    }
                }
            }
        } else {
            cwarn!(SYNC_PARCEL, "Invalid message from peer {}", token);
        }
    }
}

impl TimeoutHandler for Extension {
    fn on_timeout(&self, timer: TimerToken) {
        match timer {
            BROADCAST_TIMER_TOKEN => self.random_broadcast(),
            _ => unreachable!(),
        }
    }
}

impl Extension {
    fn send_message(&self, token: &NodeId, message: Message) {
        let api = self.api.read();
        api.as_ref().expect("Api must exist").send(token, &message.rlp_bytes());
    }

    fn random_broadcast(&self) {
        let parcels = self.client.ready_parcels();
        if parcels.is_empty() {
            ctrace!(SYNC_PARCEL, "No parcels to propagate");
            return
        }
        for (token, peer) in self.peers.read().iter() {
            let mut peer = peer.write();
            let unsent: Vec<_> = parcels
                .iter()
                .filter(|parcel| !peer.contains(&parcel.hash()))
                .map(|signed| signed.clone().deconstruct().0)
                .collect();
            if unsent.is_empty() {
                continue
            }
            let unsent_hashes = unsent.iter().map(|p| p.hash()).collect::<Vec<_>>();
            for h in unsent_hashes.iter() {
                peer.push(*h);
            }
            cdebug!(SYNC_PARCEL, "Send {} parcels to {}", unsent.len(), token);
            ctrace!(SYNC_PARCEL, "Send {:?}", unsent_hashes);
            self.send_message(token, Message::Parcels(unsent));
        }
    }
}
