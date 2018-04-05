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
use std::sync::Arc;
use parking_lot::{RwLock, Mutex};

use ccore::{BlockChainClient, BlockId};
use cnetwork::{Api, Extension, NodeId};
use ctypes::{H256, U256};
use rlp::{Encodable, UntrustedRlp};

use manager::DownloadManager;
use message::Message;

const EXTENSION_NAME: &'static str = "block-propagation";
const SYNC_TIMER_ID: usize = 0;
const SYNC_TIMER_INTERVAL: u64 = 1000;

struct Peer {
    total_score: U256,
    best_hash: H256,
}

pub struct BlockSyncExtension {
    peers: RwLock<HashMap<NodeId, Peer>>,
    client: Arc<BlockChainClient>,
    manager: Mutex<DownloadManager>,
    api: Mutex<Option<Arc<Api>>>,
}

impl BlockSyncExtension {
    pub fn new(client: Arc<BlockChainClient>) -> Arc<Self> {
        let best_block = client.block(BlockId::Latest)
            .expect("BlockSyncExtension: Best block should exist")
            .decode();
        Arc::new(Self {
            peers: RwLock::new(HashMap::new()),
            client,
            manager: Mutex::new(DownloadManager::new(best_block)),
            api: Mutex::new(None),
        })
    }

    fn send(&self, id: &NodeId, message: Message) {
        self.api.lock().as_ref().map(|api| {
            api.send(id, &message.rlp_bytes().to_vec());
        });
    }
}

impl Extension for BlockSyncExtension {
    fn name(&self) -> String { String::from(EXTENSION_NAME) }
    fn need_encryption(&self) -> bool { false }

    fn on_initialize(&self, api: Arc<Api>) {
        self.peers.write().clear();
        api.set_timer(SYNC_TIMER_ID, SYNC_TIMER_INTERVAL);
        *self.api.lock() = Some(api);
    }

    fn on_node_added(&self, id: &NodeId) {
        self.api.lock().as_ref().map(|api| api.connect(id));
    }
    fn on_node_removed(&self, id: &NodeId) { self.peers.write().remove(id); }

    fn on_connected(&self, id: &NodeId) {
        let chain_info = self.client.chain_info();
        self.send(id, Message::Status {
            total_score: chain_info.total_score,
            best_hash: chain_info.best_block_hash,
            genesis_hash: chain_info.genesis_hash,
        });
    }
    fn on_connection_allowed(&self, id: &NodeId) { self.on_connected(id); }

    fn on_message(&self, id: &NodeId, data: &Vec<u8>) {
        if let Ok(received_message) = UntrustedRlp::new(data).as_val() {
            if !self.is_valid_message(id, &received_message) {
                return;
            }
            self.apply_message(id, &received_message);

            let next_message = match received_message {
                Message::RequestHeaders { start_hash, max_count } => {
                    Some(self.create_headers_message(start_hash, max_count))
                },
                Message::RequestBodies(hashes) => {
                    Some(self.create_bodies_message(hashes))
                },
                _ => {
                    let total_score = self.client
                        .block_total_score(BlockId::Hash(self.manager.lock().best_hash()))
                        .expect("Best block of download manager should exist in chain");
                    // FIXME: Check if this statement holds mutex lock of `peers`
                    let peer_total_score = self.peers.read()
                        .get(id)
                        .expect("Peer should exist for valid message")
                        .total_score;
                    if peer_total_score > total_score {
                        self.manager.lock().create_request()
                    } else {
                        None
                    }
                },
            };
            if let Some(message) = next_message {
                self.send(id, message);
            }
        } else {
            info!("BlockSyncExtension: invalid message from peer {}", id);
        }
    }

    fn on_close(&self) { *self.api.lock() = None }

    fn on_timeout(&self, timer_id: usize) {
        debug_assert_eq!(timer_id, SYNC_TIMER_ID);
        unimplemented!();
    }
}

impl BlockSyncExtension {
    fn is_valid_message(&self, id: &NodeId, message: &Message) -> bool {
        match message {
            &Message::Status { genesis_hash, .. } => {
                if genesis_hash != self.client.chain_info().genesis_hash {
                    info!("BlockSyncExtension: genesis hash mismatch with peer {}", id);
                    false
                } else {
                    true
                }
            },
            _ => {
                if !self.peers.read().contains_key(id) {
                    info!("BlockSyncExtension: message from unexpected peer {}", id);
                    return false;
                }
                // FIXME: check if response matches requested data
                true
            }
        }
    }

    fn apply_message(&self, id: &NodeId, message: &Message) {
        match message {
            &Message::Status { total_score, best_hash, .. } => {
                self.peers.write().insert(*id, Peer { total_score, best_hash });
            },
            &Message::Headers(ref headers) => self.manager.lock().import_headers(headers),
            &Message::Bodies(ref bodies) => self.manager.lock().import_bodies(bodies),
            _ => {},
        };
        // FIXME: Import fully downloaded blocks to client
    }

    fn create_headers_message(&self, start_hash: H256, max_count: u64) -> Message {
        let mut headers = Vec::new();
        let mut block_id = BlockId::Hash(start_hash);
        for _ in 0..max_count {
            if let Some(header) = self.client.block_header(block_id) {
                headers.push(header.decode());
                block_id = BlockId::Number(header.number() + 1);
            } else {
                break;
            }
        }
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
