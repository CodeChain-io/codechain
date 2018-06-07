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
use std::collections::HashMap;
use std::sync::Arc;

use ccore::encoded::Header as EncodedHeader;
use ccore::{BlockChainClient, BlockId, BlockImportError, BlockNumber, ChainNotify, Header, ImportError};
use cnetwork::{Api, NetworkExtension, NodeId, TimerToken};
use ctypes::{H256, U256};
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use super::message::{Message, RequestMessage, ResponseMessage};
use super::peer::Peer;

const EXTENSION_NAME: &'static str = "block-propagation";
const SYNC_TIMER_TOKEN: usize = 0;
const SYNC_TIMER_INTERVAL: i64 = 1000;

const SNAPSHOT_PERIOD: u64 = (1 << 14);

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

    fn send_message(&self, token: &NodeId, message: Message) {
        self.api.lock().as_ref().map(|api| {
            api.send(token, &message.rlp_bytes().to_vec());
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
        api.set_timer(SYNC_TIMER_TOKEN, Duration::milliseconds(SYNC_TIMER_INTERVAL)).expect("Timer set succeeds");
        *self.api.lock() = Some(api);
        cinfo!(SYNC, "Sync extension initialized");
    }

    fn on_node_added(&self, token: &NodeId) {
        cinfo!(SYNC, "New peer detected #{}", token);
        self.api.lock().as_ref().map(|api| api.negotiate(token));
    }
    fn on_node_removed(&self, token: &NodeId) {
        self.peers.write().remove(token);
        cinfo!(SYNC, "Peer removed #{}", token);
    }

    fn on_negotiated(&self, token: &NodeId) {
        ctrace!(SYNC, "New peer negotiated #{}", token);
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
    fn on_negotiation_allowed(&self, token: &NodeId) {
        self.on_negotiated(token);
    }

    fn on_message(&self, token: &NodeId, data: &[u8]) {
        if let Ok(received_message) = UntrustedRlp::new(data).as_val() {
            match received_message {
                Message::Status {
                    total_score,
                    best_hash,
                    genesis_hash,
                } => {
                    self.on_peer_status(token, total_score, best_hash, genesis_hash);
                }
                Message::Request(_, request) => self.on_peer_request(token, request),
                Message::Response(_, response) => match response {
                    ResponseMessage::Headers(headers) => self.on_header_response(token, headers),
                    _ => unimplemented!(),
                },
            }
        } else {
            cinfo!(SYNC, "Invalid message from peer {}", token);
        }
    }

    fn on_timeout(&self, timer: TimerToken) {
        debug_assert_eq!(timer, SYNC_TIMER_TOKEN);

        let peer_ids: Vec<_> = self.peers.read().keys().cloned().collect();
        for id in peer_ids {
            if let Some(peer) = self.peers.write().get_mut(&id) {
                if let Some(request) = peer.create_request() {
                    self.send_message(&id, Message::Request(0, request));
                }
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
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
    }

    fn new_headers(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        _enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
        let peer_ids: Vec<_> = self.peers.read().keys().cloned().collect();
        for id in peer_ids {
            if let Some(peer) = self.peers.write().get_mut(&id) {
                peer.mark_as_imported(imported.clone());
            }
        }
    }
}

impl Extension {
    fn on_peer_status(&self, from: &NodeId, total_score: U256, best_hash: H256, genesis_hash: H256) {
        // Validity check
        if genesis_hash != self.client.chain_info().genesis_hash {
            cinfo!(SYNC, "Genesis hash mismatch with peer {}", from);
            return
        }

        let mut peers = self.peers.write();
        if peers.contains_key(from) {
            peers.get_mut(from).unwrap().update(total_score, best_hash);
        } else {
            peers.insert(*from, Peer::new(self.client.clone(), total_score, best_hash));
        }
    }
}

impl Extension {
    fn on_peer_request(&self, from: &NodeId, request: RequestMessage) {
        if !self.peers.read().contains_key(from) {
            cinfo!(SYNC, "Request from invalid peer #{} received", from);
            return
        }

        if !self.is_valid_request(&request) {
            cinfo!(SYNC, "Invalid request received from peer #{}", from);
            return
        }

        let response = match request {
            RequestMessage::Headers {
                start_number,
                max_count,
            } => self.create_headers_response(start_number, max_count),
            RequestMessage::Bodies(hashes) => self.create_bodies_response(hashes),
            RequestMessage::StateHead(hash) => self.create_state_head_response(hash),
            RequestMessage::StateChunk {
                block_hash,
                tree_root,
            } => self.create_state_chunk_response(block_hash, tree_root),
        };

        // FIXME: assign request id
        self.send_message(from, Message::Response(0, response));
    }

    fn is_valid_request(&self, request: &RequestMessage) -> bool {
        match request {
            RequestMessage::Headers {
                ..
            } => true,
            RequestMessage::Bodies(hashes) => hashes.len() != 0,
            RequestMessage::StateHead(hash) => match self.client.block_number(BlockId::Hash(*hash)) {
                Some(number) if number % SNAPSHOT_PERIOD == 0 => true,
                _ => false,
            },
            RequestMessage::StateChunk {
                block_hash,
                tree_root: _tree_root,
            } => {
                let _is_checkpoint = match self.client.block_number(BlockId::Hash(*block_hash)) {
                    Some(number) if number % SNAPSHOT_PERIOD == 0 => true,
                    _ => false,
                };
                // FIXME:  check tree_root
                unimplemented!()
            }
        }
    }

    fn create_headers_response(&self, start_number: BlockNumber, max_count: u64) -> ResponseMessage {
        let headers = (0..max_count)
            .map(|number| self.client.block(BlockId::Number(start_number + number)))
            .take_while(|block| block.is_some())
            .map(|block| block.expect("take_while guarantees existance of item").header().decode())
            .collect();
        ResponseMessage::Headers(headers)
    }

    fn create_bodies_response(&self, hashes: Vec<H256>) -> ResponseMessage {
        let mut bodies = Vec::new();
        for hash in hashes {
            if let Some(body) = self.client.block_body(BlockId::Hash(hash)) {
                bodies.push(body.parcels());
            }
        }
        ResponseMessage::Bodies(bodies)
    }

    fn create_state_head_response(&self, _hash: H256) -> ResponseMessage {
        unimplemented!()
    }

    fn create_state_chunk_response(&self, _hash: H256, _tree_root: H256) -> ResponseMessage {
        unimplemented!()
    }
}

impl Extension {
    fn on_header_response(&self, from: &NodeId, headers: Vec<Header>) {
        let mut completed = if let Some(peer) = self.peers.write().get_mut(from) {
            let mut encoded: Vec<_> = headers.iter().map(|h| EncodedHeader::new(h.rlp_bytes().to_vec())).collect();
            encoded.sort_unstable_by_key(|header| header.number());

            // Continuity check
            for neighbors in encoded.windows(2) {
                let parent = &neighbors[0];
                let child = &neighbors[1];
                if child.number() != parent.number() + 1 || child.parent_hash() != parent.hash() {
                    cinfo!(SYNC, "Headers are not continuous");
                }
            }

            if encoded.len() != 0 && encoded.first().map(|header| header.number()) == peer.last_request_number() {
                peer.import_headers(encoded);
            }
            peer.downloaded()
        } else {
            Vec::new()
        };
        completed.sort_unstable_by_key(|header| header.number());

        let mut exists = Vec::new();
        for header in completed {
            let hash = header.hash();
            // FIXME: handle import errors
            match self.client.import_header(header.into_inner()) {
                Err(BlockImportError::Import(ImportError::AlreadyInChain)) => exists.push(hash),
                _ => {}
            }
        }

        if let Some(peer) = self.peers.write().get_mut(from) {
            peer.mark_as_imported(exists);
            if let Some(request) = peer.create_request() {
                self.send_message(from, Message::Request(0, request));
            }
        }
    }
}
