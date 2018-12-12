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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ccore::encoded::Header as EncodedHeader;
use ccore::{
    Block, BlockChainClient, BlockId, BlockImportError, BlockInfo, ChainInfo, ChainNotify, Client, Header, ImportBlock,
    ImportError, Seal, UnverifiedParcel,
};
use cnetwork::{Api, NetworkExtension, NodeId};
use cstate::FindActionHandler;
use ctimer::{TimeoutHandler, TimerToken};
use ctoken_generator::TokenGenerator;
use ctypes::parcel::Action;
use ctypes::BlockNumber;
use parking_lot::{Mutex, RwLock};
use primitives::{H256, U256};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rlp::{Encodable, UntrustedRlp};
use time::Duration;

use super::super::block::BlockSyncInfo;
use super::downloader::{BodyDownloader, HeaderDownloader};
use super::message::{Message, RequestMessage, ResponseMessage};

const SYNC_TIMER_TOKEN: TimerToken = 0;
const SYNC_EXPIRE_TOKEN_BEGIN: TimerToken = SYNC_TIMER_TOKEN + 1;
const SYNC_EXPIRE_TOKEN_LIMIT: usize = 1000;
const SYNC_EXPIRE_TOKEN_END: TimerToken = SYNC_EXPIRE_TOKEN_BEGIN + SYNC_EXPIRE_TOKEN_LIMIT;

const SYNC_TIMER_INTERVAL: i64 = 1000;
const SYNC_EXPIRE_REQUEST_INTERVAL: i64 = 15000;

const SNAPSHOT_PERIOD: u64 = (1 << 14);

#[derive(Debug, PartialEq)]
pub struct TokenInfo {
    node_id: NodeId,
    request_id: Option<u64>,
}

pub struct Extension {
    requests: RwLock<HashMap<NodeId, Vec<(u64, RequestMessage)>>>,
    header_downloaders: RwLock<HashMap<NodeId, HeaderDownloader>>,
    body_downloader: Mutex<BodyDownloader>,
    tokens: RwLock<HashMap<NodeId, TimerToken>>,
    tokens_info: RwLock<HashMap<TimerToken, TokenInfo>>,
    token_generator: Mutex<TokenGenerator>,
    client: Arc<Client>,
    api: RwLock<Option<Arc<Api>>>,
    last_request: AtomicUsize,
}

impl Extension {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    pub fn new(client: Arc<Client>) -> Arc<Self> {
        Arc::new(Self {
            requests: RwLock::new(HashMap::new()),
            header_downloaders: RwLock::new(HashMap::new()),
            body_downloader: Mutex::new(BodyDownloader::new()),
            tokens: RwLock::new(HashMap::new()),
            tokens_info: RwLock::new(HashMap::new()),
            token_generator: Mutex::new(TokenGenerator::new(SYNC_EXPIRE_TOKEN_BEGIN, SYNC_EXPIRE_TOKEN_END)),
            client,
            api: RwLock::new(None),
            last_request: AtomicUsize::new(0),
        })
    }

    fn send_message(&self, id: &NodeId, message: &Message) {
        let api = self.api.read();
        api.as_ref().expect("Api must exist").send(id, &*message.rlp_bytes());
    }

    fn dismiss_request(&self, id: &NodeId, request_id: u64) {
        if let Some(requests) = self.requests.write().get_mut(id) {
            requests.retain(|(i, _)| *i != request_id);
        }
    }

    fn send_header_request(&self, id: &NodeId, request: RequestMessage) {
        if let Some(requests) = self.requests.write().get_mut(id) {
            let request_id = self.last_request.fetch_add(1, Ordering::Relaxed) as u64;
            requests.push((request_id, request.clone()));
            self.send_message(id, &Message::Request(request_id, request));
        }
    }

    fn send_body_request(&self, id: &NodeId) {
        if let Some(requests) = self.requests.write().get_mut(id) {
            let have_body_request = {
                requests.iter().any(|r| match r {
                    (_, RequestMessage::Bodies(..)) => true,
                    _ => false,
                })
            };
            if have_body_request {
                return
            }

            if let Some(request) = self.body_downloader.lock().create_request() {
                let request_id = self.last_request.fetch_add(1, Ordering::Relaxed) as u64;
                requests.push((request_id, request.clone()));
                self.send_message(id, &Message::Request(request_id, request));

                let tokens = self.tokens.read();
                let mut tokens_info = self.tokens_info.write();

                let token = tokens.get(id).unwrap();
                let token_info = tokens_info.get_mut(token).unwrap();

                let api = self.api.read();
                api.as_ref()
                    .expect("Api must exist")
                    .set_timer_once(*token, Duration::milliseconds(SYNC_EXPIRE_REQUEST_INTERVAL))
                    .expect("Timer set succeeds");
                token_info.request_id = Some(request_id);
            }
        }
    }

    fn send_response(&self, id: &NodeId, request_id: u64, response: ResponseMessage) {
        self.send_message(id, &Message::Response(request_id, response));
    }
}

impl NetworkExtension for Extension {
    fn name(&self) -> &'static str {
        "block-propagation"
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
        api.set_timer(SYNC_TIMER_TOKEN, Duration::milliseconds(SYNC_TIMER_INTERVAL)).expect("Timer set succeeds");
        *api_lock = Some(api);

        let mut header = self.client.best_header();
        let mut hollow_headers = vec![header.decode()];
        while self.client.block_body(&BlockId::Hash(header.hash())).is_none() {
            header = self
                .client
                .block_header(&BlockId::Hash(header.parent_hash()))
                .expect("Every imported header must have parent");
            hollow_headers.push(header.decode());
        }
        for neighbors in hollow_headers.windows(2).rev() {
            let child = &neighbors[0];
            let parent = &neighbors[1];
            cdebug!(SYNC, "Adding block #{} (hash: {}) for initial body download target", child.number(), child.hash());
            self.body_downloader.lock().add_target(child, parent);
        }
        cinfo!(SYNC, "Sync extension initialized");
    }

    fn on_node_added(&self, id: &NodeId, _version: u64) {
        let mut requests = self.requests.write();
        let mut tokens = self.tokens.write();
        let mut tokens_info = self.tokens_info.write();
        let mut token_generator = self.token_generator.lock();

        cinfo!(SYNC, "New peer detected #{}", id);
        let chain_info = self.client.chain_info();
        self.send_message(
            id,
            &Message::Status {
                total_score: chain_info.highest_score,
                best_hash: chain_info.best_block_hash,
                genesis_hash: chain_info.genesis_hash,
            },
        );

        let token = token_generator.gen().expect("Token generator is full");
        let token_info = TokenInfo {
            node_id: *id,
            request_id: None,
        };

        let t = requests.insert(*id, Vec::new());
        debug_assert_eq!(None, t);
        let t = tokens_info.insert(token, token_info);
        debug_assert_eq!(None, t);
        let t = tokens.insert(*id, token);
        debug_assert_eq!(None, t);
        debug_assert!(t.is_none());
    }

    fn on_node_removed(&self, id: &NodeId) {
        let mut requests = self.requests.write();
        let mut header_downloaders = self.header_downloaders.write();
        let mut tokens = self.tokens.write();
        let mut tokens_info = self.tokens_info.write();
        let mut token_generator = self.token_generator.lock();

        cinfo!(SYNC, "Peer removed #{}", id);
        header_downloaders.remove(id);

        requests.remove(id);
        if let Some(token) = tokens.remove(id) {
            tokens_info.remove(&token);
            token_generator.restore(token);
        }
    }

    fn on_message(&self, id: &NodeId, data: &[u8]) {
        if let Ok(received_message) = UntrustedRlp::new(data).as_val() {
            match received_message {
                Message::Status {
                    total_score,
                    best_hash,
                    genesis_hash,
                } => self.on_peer_status(id, total_score, best_hash, genesis_hash),
                Message::Request(request_id, request) => self.on_peer_request(id, request_id, request),
                Message::Response(request_id, response) => self.on_peer_response(id, request_id, response),
            }
        } else {
            cinfo!(SYNC, "Invalid message from peer {}", id);
        }
    }
}

impl TimeoutHandler for Extension {
    fn on_timeout(&self, token: TimerToken) {
        match token {
            SYNC_TIMER_TOKEN => {
                let highest_score = self.client.chain_info().highest_score;
                let mut peer_ids: Vec<_> = self.header_downloaders.read().keys().cloned().collect();
                peer_ids.shuffle(&mut thread_rng());

                for id in peer_ids {
                    if let Some(peer) = self.header_downloaders.write().get_mut(&id) {
                        if let Some(request) = peer.create_request() {
                            self.send_header_request(&id, request);
                        }
                    }

                    let peer_score = if let Some(peer) = self.header_downloaders.read().get(&id) {
                        peer.total_score()
                    } else {
                        U256::zero()
                    };

                    if peer_score > highest_score {
                        self.send_body_request(&id);
                    }
                }
            }
            SYNC_EXPIRE_TOKEN_BEGIN...SYNC_EXPIRE_TOKEN_END => {
                let (id, request_id) = {
                    let mut tokens_info = self.tokens_info.write();
                    let token_info = match tokens_info.get_mut(&token) {
                        Some(info) => info,
                        None => return,
                    };

                    match token_info.request_id {
                        Some(request_id) => {
                            token_info.request_id = None;
                            (token_info.node_id, request_id)
                        }
                        None => return,
                    }
                };

                if let Some(requests) = self.requests.write().get_mut(&id) {
                    let expired_request = requests.iter().find(|(r, _)| *r == request_id);
                    if let Some((_, request)) = expired_request {
                        match request {
                            RequestMessage::Bodies(hashes) => {
                                self.body_downloader.lock().reset_downloading(&hashes);
                            }
                            _ => unreachable!(),
                        }
                    }
                }

                self.dismiss_request(&id, request_id);
            }
            _ => unreachable!(),
        }
    }
}

impl ChainNotify for Extension {
    fn new_headers(
        &self,
        imported: Vec<H256>,
        _invalid: Vec<H256>,
        enacted: Vec<H256>,
        retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
        _new_highest_header: Option<H256>,
    ) {
        let peer_ids: Vec<_> = self.header_downloaders.read().keys().cloned().collect();
        for id in peer_ids {
            if let Some(peer) = self.header_downloaders.write().get_mut(&id) {
                peer.mark_as_imported(imported.clone());
            }
        }
        let mut headers_to_download: Vec<_> = enacted
            .into_iter()
            .map(|hash| self.client.block_header(&BlockId::Hash(hash)).expect("Enacted header must exist"))
            .collect();
        headers_to_download.sort_unstable_by_key(|header| header.number());
        headers_to_download.dedup_by_key(|header| header.hash());

        headers_to_download
            .into_iter()
            .filter(|header| self.client.block_body(&BlockId::Hash(header.hash())).is_none())
            .for_each(|header| {
                let parent = self
                    .client
                    .block_header(&BlockId::Hash(header.parent_hash()))
                    .expect("Enacted header must have parent");
                self.body_downloader.lock().add_target(&header.decode(), &parent.decode());
            });
        self.body_downloader.lock().remove_target(&retracted);
    }

    fn new_blocks(
        &self,
        imported: Vec<H256>,
        invalid: Vec<H256>,
        _enacted: Vec<H256>,
        _retracted: Vec<H256>,
        _sealed: Vec<H256>,
        _duration: u64,
    ) {
        self.body_downloader.lock().remove_target(&imported);
        self.body_downloader.lock().remove_target(&invalid);


        let chain_info = self.client.chain_info();
        let peer_ids = self.header_downloaders.read();
        for id in peer_ids.keys() {
            self.send_message(
                id,
                &Message::Status {
                    total_score: chain_info.highest_score,
                    best_hash: chain_info.best_block_hash,
                    genesis_hash: chain_info.genesis_hash,
                },
            );
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

        cinfo!(SYNC, "Peer #{} status update: total_score: {}, best_hash: {}", from, total_score, best_hash);

        let mut peers = self.header_downloaders.write();
        if peers.contains_key(from) {
            peers.get_mut(from).unwrap().update(total_score, best_hash);
        } else {
            peers.insert(*from, HeaderDownloader::new(self.client.clone(), total_score, best_hash));
        }
    }

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
            RequestMessage::Bodies(hashes) => !hashes.is_empty(),
            RequestMessage::StateHead(hash) => match self.client.block_number(&BlockId::Hash(*hash)) {
                Some(number) if number % SNAPSHOT_PERIOD == 0 => true,
                _ => false,
            },
            RequestMessage::StateChunk {
                block_hash,
                ..
            } => {
                let _is_checkpoint = match self.client.block_number(&BlockId::Hash(*block_hash)) {
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
            .map(|number| self.client.block(&BlockId::Number(start_number + number)))
            .take_while(|block| block.is_some())
            .map(|block| block.expect("take_while guarantees existance of item").header().decode())
            .collect();
        ResponseMessage::Headers(headers)
    }

    fn create_bodies_response(&self, hashes: Vec<H256>) -> ResponseMessage {
        let bodies = hashes
            .into_iter()
            .map(|hash| self.client.block_body(&BlockId::Hash(hash)).map(|body| body.parcels()).unwrap_or_default())
            .collect();
        ResponseMessage::Bodies(bodies)
    }

    fn create_state_head_response(&self, _hash: H256) -> ResponseMessage {
        unimplemented!()
    }

    fn create_state_chunk_response(&self, _hash: H256, _tree_root: H256) -> ResponseMessage {
        unimplemented!()
    }

    fn on_peer_response(&self, from: &NodeId, id: u64, mut response: ResponseMessage) {
        let last_request = self.requests.read()[from].iter().find(|(i, _)| *i == id).cloned();
        if let Some((_, request)) = last_request {
            if let ResponseMessage::Headers(headers) = &mut response {
                headers.sort_unstable_by_key(|h| h.number());
            }

            if !self.is_valid_response(&request, &response) {
                return
            }

            match response {
                ResponseMessage::Headers(headers) => {
                    self.dismiss_request(from, id);
                    self.on_header_response(from, &headers)
                }
                ResponseMessage::Bodies(bodies) => {
                    let hashes = match request {
                        RequestMessage::Bodies(hashes) => hashes,
                        _ => unreachable!(),
                    };
                    assert_eq!(bodies.len(), hashes.len());
                    if let Some(token) = self.tokens.read().get(from) {
                        if let Some(token_info) = self.tokens_info.write().get_mut(token) {
                            if token_info.request_id.is_none() {
                                ctrace!(SYNC, "Expired before handling response");
                                return
                            }
                            let api = self.api.read();
                            api.as_ref().expect("Api must exist").clear_timer(*token).expect("Timer clear succeed");
                            token_info.request_id = None;
                        }
                    }
                    self.dismiss_request(from, id);
                    self.on_body_response(hashes, bodies);
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
            (RequestMessage::Bodies(hashes), ResponseMessage::Bodies(bodies)) => {
                if hashes.len() != bodies.len() {
                    return false
                }
                for body in bodies {
                    for parcel in body {
                        let is_valid = match &parcel.action {
                            Action::Custom {
                                handler_id,
                                ..
                            } => self.client.find_action_handler_for(*handler_id).is_some(),
                            _ => true,
                        };
                        if !is_valid {
                            return false
                        }
                    }
                }
                true
            }
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

    fn on_header_response(&self, from: &NodeId, headers: &[Header]) {
        let mut completed = if let Some(peer) = self.header_downloaders.write().get_mut(from) {
            let encoded: Vec<_> = headers.iter().map(|h| EncodedHeader::new(h.rlp_bytes().to_vec())).collect();
            peer.import_headers(&encoded);
            peer.downloaded()
        } else {
            Vec::new()
        };
        completed.sort_unstable_by_key(|header| header.number());

        let mut exists = Vec::new();
        for header in completed {
            match self.client.import_header(header.clone().into_inner()) {
                Err(BlockImportError::Import(ImportError::AlreadyInChain)) => exists.push(header.hash()),
                // FIXME: handle import errors
                Err(err) => {
                    cwarn!(SYNC, "Cannot import header({}): {:?}", header.hash(), err);
                    break
                }
                _ => {}
            }
        }

        if let Some(peer) = self.header_downloaders.write().get_mut(from) {
            peer.mark_as_imported(exists);
            if let Some(request) = peer.create_request() {
                self.send_header_request(from, request);
            }
        }
    }

    fn on_body_response(&self, hashes: Vec<H256>, bodies: Vec<Vec<UnverifiedParcel>>) {
        {
            let mut body_downloader = self.body_downloader.lock();
            body_downloader.import_bodies(hashes, bodies);
            let completed = body_downloader.drain();
            for (hash, parcels) in completed {
                let header = self
                    .client
                    .block_header(&BlockId::Hash(hash))
                    .expect("Downloaded body's header must exist")
                    .decode();
                let block = Block {
                    header,
                    parcels,
                };
                cdebug!(SYNC, "Body download completed for #{}({})", block.header.number(), hash);
                match self.client.import_block(block.rlp_bytes(&Seal::With)) {
                    Err(BlockImportError::Import(ImportError::AlreadyInChain)) => {
                        cwarn!(SYNC, "Downloaded already existing block({})", hash)
                    }
                    Err(err) => {
                        // FIXME: handle import errors
                        cwarn!(SYNC, "Cannot import block({}): {:?}", hash, err);
                        break
                    }
                    _ => {}
                }
            }
        }

        let total_score = self.client.chain_info().highest_score;
        let mut peer_ids: Vec<_> = self.header_downloaders.read().keys().cloned().collect();
        peer_ids.shuffle(&mut thread_rng());

        for id in peer_ids {
            let peer_score = if let Some(peer) = self.header_downloaders.read().get(&id) {
                peer.total_score()
            } else {
                U256::zero()
            };

            if peer_score > total_score {
                self.send_body_request(&id);
            }
        }
    }
}

impl BlockSyncInfo for Extension {
    fn get_peers(&self) -> Vec<NodeId> {
        self.header_downloaders.read().keys().cloned().collect()
    }
}
