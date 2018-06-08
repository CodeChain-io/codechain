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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ccore::encoded::Header as EncodedHeader;
use ccore::{
    Block, BlockChainClient, BlockId, BlockImportError, BlockNumber, ChainNotify, Header, ImportError, Seal,
    UnverifiedParcel,
};
use cnetwork::{Api, NetworkExtension, NodeId, TimerToken};
use ctypes::{H256, U256};
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use super::downloader::{BodyDownloader, HeaderDownloader};
use super::message::{Message, RequestMessage, ResponseMessage};

const EXTENSION_NAME: &'static str = "block-propagation";
const SYNC_TIMER_TOKEN: usize = 0;
const SYNC_TIMER_INTERVAL: i64 = 1000;

const SNAPSHOT_PERIOD: u64 = (1 << 14);

pub struct Extension {
    requests: RwLock<HashMap<NodeId, Vec<(u64, RequestMessage)>>>,
    header_downloaders: RwLock<HashMap<NodeId, HeaderDownloader>>,
    body_downloader: Mutex<BodyDownloader>,
    client: Arc<BlockChainClient>,
    api: Mutex<Option<Arc<Api>>>,
    last_request: AtomicUsize,
}

impl Extension {
    pub fn new(client: Arc<BlockChainClient>) -> Arc<Self> {
        Arc::new(Self {
            requests: RwLock::new(HashMap::new()),
            header_downloaders: RwLock::new(HashMap::new()),
            body_downloader: Mutex::new(BodyDownloader::new(Vec::new())),
            client,
            api: Mutex::new(None),
            last_request: AtomicUsize::new(0),
        })
    }

    fn send_message(&self, token: &NodeId, message: Message) {
        self.api.lock().as_ref().map(|api| {
            api.send(token, &message.rlp_bytes().to_vec());
        });
    }

    fn send_request(&self, token: &NodeId, request: RequestMessage) {
        if let Some(requests) = self.requests.write().get_mut(token) {
            let id = self.last_request.fetch_add(1, Ordering::Relaxed) as u64;
            requests.push((id, request.clone()));
            self.send_message(token, Message::Request(id, request));
        }
    }

    fn send_response(&self, token: &NodeId, id: u64, response: ResponseMessage) {
        self.send_message(token, Message::Response(id, response));
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
        self.header_downloaders.write().remove(token);
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
                Message::Request(id, request) => self.on_peer_request(token, id, request),
                Message::Response(id, response) => self.on_peer_response(token, id, response),
            }
        } else {
            cinfo!(SYNC, "Invalid message from peer {}", token);
        }
    }

    fn on_timeout(&self, timer: TimerToken) {
        debug_assert_eq!(timer, SYNC_TIMER_TOKEN);

        let peer_ids: Vec<_> = self.header_downloaders.read().keys().cloned().collect();
        for id in peer_ids {
            if let Some(peer) = self.header_downloaders.write().get_mut(&id) {
                if let Some(request) = peer.create_request() {
                    self.send_request(&id, request);
                }
            }

            // FIXME: invalidate expired body requests
            let have_body_request = {
                if let Some(request_list) = self.requests.read().get(&id) {
                    request_list.iter().any(|r| match r {
                        (_, RequestMessage::Bodies(..)) => true,
                        _ => false,
                    })
                } else {
                    false
                }
            };
            if !have_body_request {
                if let Some(request) = self.body_downloader.lock().create_request() {
                    self.send_request(&id, request);
                }
            }
        }
    }
}

impl ChainNotify for Extension {
    fn new_blocks(
        &self,
        imported: Vec<H256>,
        invalid: Vec<H256>,
        _enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
        self.body_downloader.lock().remove_target(imported);
        self.body_downloader.lock().remove_target(invalid);


        let chain_info = self.client.chain_info();
        let peer_ids: Vec<_> = self.header_downloaders.read().keys().cloned().collect();
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

    fn new_headers(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
        let peer_ids: Vec<_> = self.header_downloaders.read().keys().cloned().collect();
        for id in peer_ids {
            if let Some(peer) = self.header_downloaders.write().get_mut(&id) {
                peer.mark_as_imported(imported.clone());
            }
        }
        let mut enacted_headers: Vec<_> = enacted
            .into_iter()
            .map(|hash| self.client.block_header(BlockId::Hash(hash)).expect("Enacted header must exist"))
            .collect();
        enacted_headers.sort_unstable_by_key(|header| header.number());

        let body_targets = enacted_headers
            .into_iter()
            .filter(|header| self.client.block_body(BlockId::Hash(header.hash())).is_none())
            .map(|header| (header.hash(), header.parcels_root()))
            .collect();
        self.body_downloader.lock().add_target(body_targets);
        self.body_downloader.lock().remove_target(retracted);
    }
}

impl Extension {
    fn on_peer_status(&self, from: &NodeId, total_score: U256, best_hash: H256, genesis_hash: H256) {
        // Validity check
        if genesis_hash != self.client.chain_info().genesis_hash {
            cinfo!(SYNC, "Genesis hash mismatch with peer {}", from);
            return
        }

        ctrace!(SYNC, "Peer #{} status update: total_score: {}, best_hash: {}", from, total_score, best_hash);

        let mut requests = self.requests.write();
        let mut peers = self.header_downloaders.write();
        if peers.contains_key(from) {
            peers.get_mut(from).unwrap().update(total_score, best_hash);
        } else {
            requests.insert(*from, Vec::new());
            peers.insert(*from, HeaderDownloader::new(self.client.clone(), total_score, best_hash));
        }
    }
}

impl Extension {
    fn on_peer_request(&self, from: &NodeId, id: u64, request: RequestMessage) {
        if !self.header_downloaders.read().contains_key(from) {
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

        self.send_response(from, id, response);
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
            } else {
                bodies.push(Vec::new());
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
    fn on_peer_response(&self, from: &NodeId, id: u64, mut response: ResponseMessage) {
        let last_request = self.requests.read()[from].iter().find(|(i, _)| *i == id).cloned();
        if let Some((_, request)) = last_request {
            match &mut response {
                ResponseMessage::Headers(headers) => {
                    headers.sort_unstable_by_key(|h| h.number());
                }
                _ => {}
            }

            if !self.is_valid_response(&request, &response) {
                return
            }

            match response {
                ResponseMessage::Headers(headers) => self.on_header_response(from, headers),
                ResponseMessage::Bodies(bodies) => {
                    let hashes = match request {
                        RequestMessage::Bodies(hashes) => hashes,
                        _ => unreachable!(),
                    };
                    self.on_body_response(from, hashes, bodies)
                }
                _ => unimplemented!(),
            }
        }
    }

    fn is_valid_response(&self, request: &RequestMessage, response: &ResponseMessage) -> bool {
        match (request, response) {
            (
                RequestMessage::Headers {
                    start_number,
                    ..
                },
                ResponseMessage::Headers(headers),
            ) => {
                // Continuity check
                for neighbors in headers.windows(2) {
                    let parent = &neighbors[0];
                    let child = &neighbors[1];
                    if child.number() != parent.number() + 1 || *child.parent_hash() != parent.hash() {
                        return false
                    }
                }

                headers.first().map(|header| header.number()) == Some(*start_number)
            }
            (RequestMessage::Bodies(..), ResponseMessage::Bodies(..)) => true,
            (RequestMessage::StateHead(..), ResponseMessage::StateHead(..)) => unimplemented!(),
            (
                RequestMessage::StateChunk {
                    ..
                },
                ResponseMessage::StateChunk(..),
            ) => unimplemented!(),
            _ => false,
        }
    }

    fn on_header_response(&self, from: &NodeId, headers: Vec<Header>) {
        let mut completed = if let Some(peer) = self.header_downloaders.write().get_mut(from) {
            let encoded = headers.iter().map(|h| EncodedHeader::new(h.rlp_bytes().to_vec())).collect();
            peer.import_headers(encoded);
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

        if let Some(peer) = self.header_downloaders.write().get_mut(from) {
            peer.mark_as_imported(exists);
            if let Some(request) = peer.create_request() {
                self.send_request(from, request);
            }
        }
    }

    fn on_body_response(&self, from: &NodeId, hashes: Vec<H256>, bodies: Vec<Vec<UnverifiedParcel>>) {
        self.body_downloader.lock().import_bodies(hashes, bodies);
        let completed = self.body_downloader.lock().drain();
        let mut exists = Vec::new();
        for (hash, body) in completed {
            let header = self.client.block_header(BlockId::Hash(hash)).expect("Downloaded body's header must exist");
            let block = Block {
                header: header.decode(),
                parcels: body,
            };
            // FIXME: handle import errors
            match self.client.import_block(block.rlp_bytes(Seal::With)) {
                Err(BlockImportError::Import(ImportError::AlreadyInChain)) => exists.push(hash),
                _ => {}
            }
        }
        self.body_downloader.lock().remove_target(exists);

        if let Some(request) = self.body_downloader.lock().create_request() {
            self.send_request(from, request);
        }
    }
}
