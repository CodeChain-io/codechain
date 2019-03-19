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

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use ccore::BlockChainClient;
use cnetwork::{Api, NetworkExtension, NodeId};
use ctimer::TimerToken;
use never::Never;
use parking_lot::RwLock;
use primitives::H256;
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use super::message::Message;

const BROADCAST_TIMER_TOKEN: TimerToken = 0;
const BROADCAST_TIMER_INTERVAL: i64 = 1000;
const MAX_HISTORY_SIZE: usize = 100_000;

struct KnownTxs {
    history_set: HashSet<H256>,
    history_queue: VecDeque<H256>,
}

impl KnownTxs {
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
    peers: HashMap<NodeId, RwLock<KnownTxs>>,
    client: Arc<BlockChainClient>,
    api: Box<Api>,
}

impl Extension {
    pub fn new(client: Arc<BlockChainClient>, api: Box<Api>) -> Self {
        api.set_timer(BROADCAST_TIMER_TOKEN, Duration::milliseconds(BROADCAST_TIMER_INTERVAL))
            .expect("Timer set succeeds");
        Extension {
            peers: Default::default(),
            client,
            api,
        }
    }
}

impl NetworkExtension<Never> for Extension {
    fn name() -> &'static str {
        "transaction-propagation"
    }
    fn need_encryption() -> bool {
        false
    }

    fn versions() -> &'static [u64] {
        const VERSIONS: &[u64] = &[0];
        &VERSIONS
    }

    fn on_node_added(&mut self, token: &NodeId, _version: u64) {
        self.peers.insert(*token, RwLock::new(KnownTxs::new()));
    }
    fn on_node_removed(&mut self, token: &NodeId) {
        self.peers.remove(token);
    }

    fn on_message(&mut self, token: &NodeId, data: &[u8]) {
        if let Ok(received_message) = UntrustedRlp::new(data).as_val() {
            match received_message {
                Message::Transactions(transactions) => {
                    self.client.queue_transactions(
                        transactions.iter().map(|unverified| unverified.rlp_bytes().to_vec()).collect(),
                        *token,
                    );
                    if let Some(peer) = self.peers.get(token) {
                        let mut peer = peer.write();
                        let transactions: Vec<_> =
                            transactions.iter().map(|tx| tx.hash()).filter(|tx_hash| !peer.contains(tx_hash)).collect();
                        for unverified in transactions.iter() {
                            peer.push(*unverified);
                        }
                        cdebug!(SYNC_TX, "Receive {} transactions from {}", transactions.len(), token);
                        ctrace!(SYNC_TX, "Receive {:?}", transactions);
                    } else {
                        cwarn!(SYNC_TX, "Message from {} but it's already removed", token);
                    }
                }
            }
        } else {
            cwarn!(SYNC_TX, "Invalid message from peer {}", token);
        }
    }

    fn on_timeout(&mut self, timer: TimerToken) {
        match timer {
            BROADCAST_TIMER_TOKEN => self.random_broadcast(),
            _ => unreachable!(),
        }
    }
}

impl Extension {
    fn random_broadcast(&self) {
        let transactions = self.client.ready_transactions(0..(::std::u64::MAX)).transactions;
        if transactions.is_empty() {
            ctrace!(SYNC_TX, "No transactions to propagate");
            return
        }
        for (token, peer) in &self.peers {
            let mut peer = peer.write();
            let unsent: Vec<_> = transactions
                .iter()
                .filter(|tx| !peer.contains(&tx.hash()))
                .map(|signed| signed.clone().deconstruct().0)
                .collect();
            if unsent.is_empty() {
                continue
            }
            let unsent_hashes = unsent.iter().map(|p| p.hash()).collect::<Vec<_>>();
            for h in unsent_hashes.iter() {
                peer.push(*h);
            }
            cdebug!(SYNC_TX, "Send {} transactions to {}", unsent.len(), token);
            ctrace!(SYNC_TX, "Send {:?}", unsent_hashes);
            self.api.send(token, &Message::Transactions(unsent).rlp_bytes());
        }
    }
}
