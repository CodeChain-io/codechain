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

use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use ccore::BlockChainClient;
use cnetwork::{Api, NetworkExtension, NodeId, TimerToken};
use ctypes::H256;
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use super::message::Message;

const EXTENSION_NAME: &'static str = "parcel-propagation";
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

    fn push(&mut self, hash: &H256) {
        if !self.history_set.contains(hash) {
            self.history_set.insert(*hash);
            self.history_queue.push_back(*hash);
            if self.history_queue.len() > MAX_HISTORY_SIZE {
                self.history_queue.pop_front();
            }
        }
    }

    fn contains(&mut self, hash: &H256) -> bool {
        self.history_set.contains(hash)
    }
}

pub struct Extension {
    peers: RwLock<HashMap<NodeId, Peer>>,
    client: Arc<BlockChainClient>,
    api: Mutex<Option<Arc<Api>>>,
}

impl Extension {
    pub fn new(client: Arc<BlockChainClient>) -> Arc<Self> {
        Arc::new(Self {
            peers: RwLock::new(HashMap::new()),
            client,
            api: Mutex::new(None),
        })
    }
}

impl NetworkExtension for Extension {
    fn name(&self) -> String {
        String::from(EXTENSION_NAME)
    }
    fn need_encryption(&self) -> bool {
        false
    }

    fn versions(&self) -> Vec<u64> {
        vec![0]
    }

    fn on_initialize(&self, api: Arc<Api>) {
        api.set_timer(BROADCAST_TIMER_TOKEN, Duration::milliseconds(BROADCAST_TIMER_INTERVAL))
            .expect("Timer set succeeds");
        *self.api.lock() = Some(api);
    }

    fn on_node_added(&self, token: &NodeId, _version: u64) {
        self.peers.write().insert(*token, Peer::new());
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
                    if let Some(peer) = self.peers.write().get_mut(token) {
                        parcels.iter().for_each(|unverified| {
                            peer.push(&unverified.hash());
                        });
                    }
                }
            }
        } else {
            cinfo!(SYNC, "Invalid message from peer {}", token);
        }
    }

    fn on_timeout(&self, timer: TimerToken) {
        match timer {
            BROADCAST_TIMER_TOKEN => self.random_broadcast(),
            _ => debug_assert!(false),
        }
    }
}

impl Extension {
    fn send_message(&self, token: &NodeId, message: Message) {
        self.api.lock().as_ref().map(|api| {
            api.send(token, &message.rlp_bytes().to_vec());
        });
    }

    fn random_broadcast(&self) {
        let parcels = self.client.ready_parcels();
        for (token, peer) in self.peers.write().iter_mut() {
            let unsent: Vec<_> = parcels
                .iter()
                .filter(|parcel| !peer.contains(&parcel.hash()))
                .map(|signed| signed.clone().deconstruct().0)
                .collect();
            for unverified in unsent.iter() {
                peer.push(&unverified.hash());
            }
            self.send_message(token, Message::Parcels(unsent));
        }
    }
}
