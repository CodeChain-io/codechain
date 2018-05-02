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
use rand::{thread_rng, Rng};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ccore::BlockChainClient;
use cnetwork::{Api, NetworkExtension, NodeToken, TimerToken};
use ctypes::H256;
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use super::message::Message;

const EXTENSION_NAME: &'static str = "transaction-propagation";
const BROADCAST_TIMER_TOKEN: TimerToken = 0;
const BROADCAST_TIMER_INTERVAL: i64 = 1000;
const RESET_TIMER_TOKEN: TimerToken = 1;
const RESET_TIMER_INTERVAL: i64 = 1000;

struct Peer {
    transaction_history: HashSet<H256>,
}

pub struct Extension {
    peers: RwLock<HashMap<NodeToken, Peer>>,
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

    fn on_initialize(&self, api: Arc<Api>) {
        api.set_timer(BROADCAST_TIMER_TOKEN, Duration::milliseconds(BROADCAST_TIMER_INTERVAL))
            .expect("Timer set succeeds");
        api.set_timer(RESET_TIMER_TOKEN, Duration::milliseconds(RESET_TIMER_INTERVAL)).expect("Timer set succeeds");
        *self.api.lock() = Some(api);
    }

    fn on_node_added(&self, token: &NodeToken) {
        self.api.lock().as_ref().map(|api| api.negotiate(token));
    }
    fn on_node_removed(&self, token: &NodeToken) {
        self.peers.write().remove(token);
    }

    fn on_negotiated(&self, token: &NodeToken) {
        self.peers.write().insert(
            *token,
            Peer {
                transaction_history: HashSet::new(),
            },
        );
    }
    fn on_negotiation_allowed(&self, token: &NodeToken) {
        self.on_negotiated(token);
    }

    fn on_message(&self, token: &NodeToken, data: &[u8]) {
        if let Ok(received_message) = UntrustedRlp::new(data).as_val() {
            match received_message {
                Message::Transactions(transactions) => {
                    self.client
                        .queue_transactions(transactions.iter().map(|tx| tx.rlp_bytes().to_vec()).collect(), *token);
                    if let Some(peer) = self.peers.write().get_mut(token) {
                        transactions.iter().for_each(|tx| {
                            peer.transaction_history.insert(tx.hash());
                        });
                    }
                }
            }
        } else {
            info!(target: "sync", "invalid message from peer {}", token);
        }
    }

    fn on_timeout(&self, timer: TimerToken) {
        match timer {
            BROADCAST_TIMER_TOKEN => self.random_broadcast(),
            RESET_TIMER_TOKEN => self.random_reset(),
            _ => debug_assert!(false),
        }
    }
}

impl Extension {
    fn send_message(&self, token: &NodeToken, message: Message) {
        self.api.lock().as_ref().map(|api| {
            api.send(token, &message.rlp_bytes().to_vec());
        });
    }

    fn random_broadcast(&self) {
        let transactions = self.client.ready_transactions();
        for (token, peer) in self.peers.write().iter_mut() {
            if thread_rng().gen() {
                let unsent: Vec<_> = transactions
                    .iter()
                    .filter(|tx| !peer.transaction_history.contains(&tx.hash()))
                    .map(|tx| tx.clone().deconstruct().0)
                    .collect();
                peer.transaction_history.extend(unsent.iter().map(|tx| tx.hash()));
                self.send_message(token, Message::Transactions(unsent));
            }
        }
    }

    fn random_reset(&self) {
        let mut peers = self.peers.write();
        if peers.is_empty() {
            return
        }

        let lucky_index = thread_rng().gen_range(0, peers.len());
        if let Some(peer) = peers.values_mut().nth(lucky_index) {
            peer.transaction_history.clear();
        }
    }
}
