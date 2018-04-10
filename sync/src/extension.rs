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
use std::collections::HashMap;
use std::sync::Arc;

use cbytes::Bytes;
use ccore::{BlockChainClient, BlockId, BlockNumber, ChainNotify, Seal};
use cnetwork::{Api, NetworkExtension, NodeToken, TimerToken};
use ctypes::{H256, U256};
use rlp::{Encodable, UntrustedRlp};

use manager::DownloadManager;
use message::Message;

const EXTENSION_NAME: &'static str = "block-propagation";
const SYNC_TIMER_TOKEN: usize = 0;
const SYNC_TIMER_INTERVAL: u64 = 1000;
const MAX_RETRY: usize = 3;

enum RequestInfo {
    Header(BlockNumber),
    Bodies(Vec<H256>),
}

struct Peer {
    total_score: U256,
    best_hash: H256,
    last_request: Option<RequestInfo>,
    retry: usize,
}

pub struct Extension {
    peers: RwLock<HashMap<NodeToken, Peer>>,
    client: Arc<BlockChainClient>,
    manager: Mutex<DownloadManager>,
    api: Mutex<Option<Arc<Api>>>,
}

impl Extension {
    pub fn new(client: Arc<BlockChainClient>) -> Arc<Self> {
        let best_header = client.block_header(BlockId::Latest).expect("Best block must exist");
        Arc::new(Self {
            peers: RwLock::new(HashMap::new()),
            client,
            manager: Mutex::new(DownloadManager::new(best_header.hash(), best_header.number())),
            api: Mutex::new(None),
        })
    }

    fn retract(&self, length: BlockNumber) {
        let mut best_header = self.client
            .block_header(BlockId::Hash(self.manager.lock().best_hash()))
            .expect("Best block of download manager must exist");
        for _ in 0..length {
            if best_header.parent_hash() == H256::zero() {
                break
            }
            // FIXME: This part can panic if warp-like sync mechanism is introduced
            best_header = self.client
                .block_header(BlockId::Hash(best_header.parent_hash()))
                .expect("Parent block of non-genesis block must exist");
        }
        *self.manager.lock() = DownloadManager::new(best_header.hash(), best_header.number());
        self.peers.write().values_mut().for_each(|peer| {
            peer.last_request = None;
            peer.retry = 0;
        });
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
        self.peers.write().clear();
        api.set_timer(SYNC_TIMER_TOKEN, SYNC_TIMER_INTERVAL);
        *self.api.lock() = Some(api);
    }

    fn on_node_added(&self, token: &NodeToken) {
        self.api.lock().as_ref().map(|api| api.connect(token));
    }
    fn on_node_removed(&self, token: &NodeToken) {
        self.peers.write().remove(token);
    }

    fn on_connected(&self, token: &NodeToken) {
        let chain_info = self.client.chain_info();
        self.send_message(
            token,
            Message::Status {
                total_score: chain_info.total_score,
                best_hash: chain_info.best_block_hash,
                genesis_hash: chain_info.genesis_hash,
            },
        );
    }
    fn on_connection_allowed(&self, token: &NodeToken) {
        self.on_connected(token);
    }

    fn on_message(&self, token: &NodeToken, data: &Vec<u8>) {
        if let Ok(received_message) = UntrustedRlp::new(data).as_val() {
            if !self.is_valid_message(token, &received_message) {
                return
            }
            self.apply_message(token, &received_message);

            // Do nothing and return if status message is received
            if received_message.is_status() {
                return
            }

            self.manager.lock().drain().iter().for_each(|block| {
                // FIXME: Handle block import errors
                match self.client.import_block(block.rlp_bytes(Seal::With)) {
                    Ok(_) => {}
                    Err(error) => {
                        info!(target: "BlockSyncExtension", "block import failed with error({:?})", error);
                    }
                }
            });

            // Create next message for peer
            let next_message = match received_message {
                Message::RequestHeaders {
                    start_number,
                    max_count,
                } => Some(self.create_headers_message(start_number, max_count)),
                Message::RequestBodies(hashes) => Some(self.create_bodies_message(hashes)),
                _ => {
                    let total_score = self.client
                        .block_total_score(BlockId::Hash(self.manager.lock().best_hash()))
                        .expect("Best block of download manager must exist in chain");
                    // FIXME: Check if this statement really needs `clone`
                    let peer_info = self.peers.read().get(token).map(|peer| (peer.total_score.clone(), peer.retry));
                    match peer_info {
                        Some((peer_total_score, peer_retry)) => {
                            if peer_retry < MAX_RETRY && peer_total_score > total_score {
                                self.manager.lock().create_request()
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                }
            };

            self.record_last_request(token, &next_message);

            if let Some(message) = next_message {
                self.send_message(token, message);
            }
        } else {
            info!(target: "sync", "invalid message from peer {}", token);
        }
    }

    fn on_close(&self) {
        *self.api.lock() = None
    }

    fn on_timeout(&self, timer: TimerToken) {
        debug_assert_eq!(timer, SYNC_TIMER_TOKEN);
        {
            let peers = self.peers.read();
            if peers.len() != 0 && peers.values().all(|peer| peer.retry >= MAX_RETRY) {
                // FIXME: Increase retracting step for each round
                self.retract(1);
            }
        }

        let mut peer_ids: Vec<_> = self.peers
            .read()
            .iter()
            .filter(|&(_, peer)| peer.last_request.is_none() && peer.retry < MAX_RETRY)
            .map(|(id, _)| id)
            .cloned()
            .collect();
        // Shuffle peers to avoid requesting messages in deterministic order
        thread_rng().shuffle(peer_ids.as_mut_slice());
        for id in peer_ids {
            let next_message = self.manager.lock().create_request();
            self.record_last_request(&id, &next_message);
            if let Some(message) = next_message {
                self.send_message(&id, message);
            }
        }
    }
}

impl ChainNotify for Extension {
    fn new_blocks(
        &self,
        _imported: Vec<H256>,
        _invalid: Vec<H256>,
        _enacted: Vec<H256>,
        retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _proposed: Vec<Bytes>,
        _duration: u64,
    ) {
        if retracted.len() != 0 {
            // FIXME: Increase retracting step for each round
            self.retract(1);
        } else {
            // FIXME: Send status message only when block is imported
            let chain_info = self.client.chain_info();
            let peer_ids: Vec<_> = self.peers.read().keys().cloned().collect();
            for id in peer_ids {
                self.send_message(
                    &id,
                    Message::Status {
                        total_score: chain_info.total_score,
                        best_hash: chain_info.best_block_hash,
                        genesis_hash: chain_info.genesis_hash,
                    },
                );
            }
        }
    }
}

impl Extension {
    fn is_valid_message(&self, token: &NodeToken, message: &Message) -> bool {
        match message {
            &Message::Status {
                genesis_hash,
                ..
            } => {
                if genesis_hash != self.client.chain_info().genesis_hash {
                    info!(target: "sync", "genesis hash mismatch with peer {}", token);
                    return false
                } else {
                    return true
                }
            }
            _ => {}
        }

        if let Some(last_request) = self.peers.read().get(token).map(|peer| &peer.last_request) {
            match (message, last_request) {
                (&Message::RequestBodies(ref hashes), _) => hashes.len() != 0,
                (&Message::Headers(ref headers), &Some(RequestInfo::Header(start_number))) => {
                    if headers.len() == 0 {
                        true
                    } else {
                        headers.first().expect("Response is not empty").number() == start_number
                    }
                }
                (&Message::Bodies(..), &Some(RequestInfo::Bodies(..))) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    fn apply_message(&self, token: &NodeToken, message: &Message) {
        match message {
            &Message::Status {
                total_score,
                best_hash,
                ..
            } => {
                let mut peers = self.peers.write();
                if peers.contains_key(token) {
                    let peer = peers.get_mut(token).expect("Peer list must contain peer for `token`");
                    peer.total_score = total_score;
                    peer.best_hash = best_hash;
                } else {
                    peers.insert(
                        *token,
                        Peer {
                            total_score,
                            best_hash,
                            last_request: None,
                            retry: 0,
                        },
                    );
                }
            }
            &Message::Headers(ref headers) => {
                let import_success = self.manager.lock().import_headers(headers);
                if let Some(peer) = self.peers.write().get_mut(token) {
                    peer.retry = if import_success {
                        0
                    } else {
                        peer.retry + 1
                    };
                }
            }
            &Message::Bodies(ref bodies) => {
                let import_success = self.manager.lock().import_bodies(bodies);
                if let Some(peer) = self.peers.write().get_mut(token) {
                    peer.retry = if import_success {
                        0
                    } else {
                        peer.retry + 1
                    };
                }
            }
            _ => {}
        }
    }

    fn record_last_request(&self, token: &NodeToken, message: &Option<Message>) {
        let mut peers = self.peers.write();
        if let Some(peer) = peers.get_mut(token) {
            match message {
                &Some(Message::RequestHeaders {
                    start_number,
                    ..
                }) => {
                    peer.last_request = Some(RequestInfo::Header(start_number));
                }
                &Some(Message::RequestBodies(ref hashes)) => {
                    peer.last_request = Some(RequestInfo::Bodies(hashes.clone()));
                }
                &None => {
                    peer.last_request = None;
                }
                _ => {}
            };
        }
    }

    fn send_message(&self, token: &NodeToken, message: Message) {
        self.api.lock().as_ref().map(|api| {
            api.send(token, &message.rlp_bytes().to_vec());
        });
    }

    fn create_headers_message(&self, start_number: BlockNumber, max_count: u64) -> Message {
        let headers = (0..max_count)
            .map(|number| self.client.block_header(BlockId::Number(start_number + number)))
            .take_while(|header| header.is_some())
            .map(|header| header.expect("take_while guarantees existance of item").decode())
            .collect();
        Message::Headers(headers)
    }

    fn create_bodies_message(&self, hashes: Vec<H256>) -> Message {
        let mut bodies = Vec::new();
        for hash in hashes {
            if let Some(body) = self.client.block_body(BlockId::Hash(hash)) {
                bodies.push(body.transactions());
            }
        }
        Message::Bodies(bodies)
    }
}
