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

use ccore::BlockChainClient;
use cnetwork::{Api, Extension, NodeId};
use ctypes::{H256, U256};
use rlp::Encodable;

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
        Arc::new(Self {
            peers: RwLock::new(HashMap::new()),
            client,
            manager: Mutex::new(DownloadManager::new()),
            api: Mutex::new(None),
        })
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
        let status_message = Message::Status {
            total_score: chain_info.total_score,
            best_hash: chain_info.best_block_hash,
            genesis_hash: chain_info.genesis_hash,
        };
        self.api.lock().as_ref().map(|api| {
            api.send(id, &status_message.rlp_bytes().to_vec());
        });
    }
    fn on_connection_allowed(&self, id: &NodeId) { self.on_connected(id); }

    fn on_message(&self, id: &NodeId, data: &Vec<u8>) {
        match ::rlp::decode(data) {
            Message::Status { total_score, best_hash, genesis_hash } => {
                if genesis_hash == self.client.chain_info().genesis_hash {
                    self.on_peer_status(id, total_score, best_hash);
                } else {
                    info!("BlockSyncExtension: genesis hash mismatch with peer {}", id);
                }
            },
        }
    }

    fn on_close(&self) { *self.api.lock() = None }

    fn on_timeout(&self, timer_id: usize) {
        debug_assert_eq!(timer_id, SYNC_TIMER_ID);
        unimplemented!();
    }
}

impl BlockSyncExtension {
    fn on_peer_status(&self, id: &NodeId, total_score: U256, best_hash: H256) {
        self.peers.write().insert(*id, Peer { total_score, best_hash });
    }
}
