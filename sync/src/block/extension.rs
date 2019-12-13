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

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::mem::discriminant;
use std::sync::Arc;
use std::time::Duration;

use ccore::encoded::Header as EncodedHeader;
use ccore::{
    Block, BlockChainClient, BlockChainTrait, BlockId, BlockImportError, BlockStatus, ChainNotify, Client, ImportBlock,
    ImportError, UnverifiedTransaction,
};
use cmerkle::snapshot::ChunkDecompressor;
use cmerkle::snapshot::Restore as SnapshotRestore;
use cmerkle::{skewed_merkle_root, TrieFactory};
use cnetwork::{Api, EventSender, NetworkExtension, NodeId};
use cstate::FindActionHandler;
use ctimer::TimerToken;
use ctypes::header::{Header, Seal};
use ctypes::transaction::Action;
use ctypes::{BlockHash, BlockNumber};
use hashdb::AsHashDB;
use kvdb::DBTransaction;
use primitives::{H256, U256};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rlp::{Encodable, Rlp};
use token_generator::TokenGenerator;

use super::downloader::{BodyDownloader, HeaderDownloader};
use super::message::{Message, RequestMessage, ResponseMessage};
use crate::snapshot::snapshot_path;

const SYNC_TIMER_TOKEN: TimerToken = 0;
const SYNC_EXPIRE_TOKEN_BEGIN: TimerToken = SYNC_TIMER_TOKEN + 1;
const SYNC_EXPIRE_TOKEN_LIMIT: usize = 1000;
const SYNC_EXPIRE_TOKEN_END: TimerToken = SYNC_EXPIRE_TOKEN_BEGIN + SYNC_EXPIRE_TOKEN_LIMIT;

const SYNC_TIMER_INTERVAL: u64 = 1000;
const SYNC_EXPIRE_REQUEST_INTERVAL: u64 = 15000;

#[derive(Debug, PartialEq)]
pub struct TokenInfo {
    node_id: NodeId,
    request_id: Option<u64>,
}

#[derive(Debug)]
enum State {
    SnapshotHeader(BlockHash, u64),
    SnapshotBody {
        header: EncodedHeader,
        prev_root: H256,
    },
    SnapshotChunk {
        block: BlockHash,
        restore: SnapshotRestore,
    },
    Full,
}

impl State {
    fn initial(client: &Client, snapshot_target: Option<(BlockHash, u64)>) -> Self {
        let (hash, num) = match snapshot_target {
            Some(target) => target,
            None => return State::Full,
        };
        let header = match client.block_header(&num.into()) {
            Some(ref h) if h.hash() == hash => h.clone(),
            _ => return State::SnapshotHeader(hash, num),
        };
        if client.block_body(&hash.into()).is_none() {
            let parent_hash = header.parent_hash();
            let parent =
                client.block_header(&parent_hash.into()).expect("Parent header of the snapshot header must exist");
            return State::SnapshotBody {
                header,
                prev_root: parent.transactions_root(),
            }
        }

        let state_db = client.state_db().read();
        let state_root = header.state_root();
        match TrieFactory::readonly(state_db.as_hashdb(), &state_root) {
            Ok(ref trie) if trie.is_complete() => State::Full,
            _ => State::SnapshotChunk {
                block: hash,
                restore: SnapshotRestore::new(state_root),
            },
        }
    }

    fn next(&self, client: &Client) -> Self {
        match self {
            State::SnapshotHeader(hash, _) => {
                let header = client.block_header(&(*hash).into()).expect("Snapshot header is imported");
                let parent = client
                    .block_header(&header.parent_hash().into())
                    .expect("Parent of the snapshot header must be imported");
                State::SnapshotBody {
                    header,
                    prev_root: parent.transactions_root(),
                }
            }
            State::SnapshotBody {
                header,
                ..
            } => State::SnapshotChunk {
                block: header.hash(),
                restore: SnapshotRestore::new(header.state_root()),
            },
            State::SnapshotChunk {
                ..
            } => State::Full,
            State::Full => State::Full,
        }
    }
}

pub struct Extension {
    state: State,
    requests: HashMap<NodeId, Vec<(u64, RequestMessage)>>,
    connected_nodes: HashSet<NodeId>,
    header_downloaders: HashMap<NodeId, HeaderDownloader>,
    body_downloader: BodyDownloader,
    tokens: HashMap<NodeId, TimerToken>,
    tokens_info: HashMap<TimerToken, TokenInfo>,
    token_generator: TokenGenerator,
    client: Arc<Client>,
    api: Box<dyn Api>,
    last_request: u64,
    nonce: u64,
    snapshot_dir: Option<String>,
}

impl Extension {
    pub fn new(
        client: Arc<Client>,
        api: Box<dyn Api>,
        snapshot_target: Option<(BlockHash, u64)>,
        snapshot_dir: Option<String>,
    ) -> Extension {
        api.set_timer(SYNC_TIMER_TOKEN, Duration::from_millis(SYNC_TIMER_INTERVAL)).expect("Timer set succeeds");

        let state = State::initial(&client, snapshot_target);
        cdebug!(SYNC, "Initial state is {:?}", state);
        let mut header = client.best_header();
        let mut hollow_headers = vec![header.decode()];
        while client.block_body(&BlockId::Hash(header.hash())).is_none() {
            if let Some(h) = client.block_header(&BlockId::Hash(header.parent_hash())) {
                header = h;
                hollow_headers.push(header.decode());
            } else {
                break
            }
        }
        let mut body_downloader = BodyDownloader::default();
        for neighbors in hollow_headers.windows(2).rev() {
            let child = &neighbors[0];
            let parent = &neighbors[1];
            cdebug!(SYNC, "Adding block #{} (hash: {}) for initial body download target", child.number(), child.hash());
            let is_empty = child.transactions_root() == parent.transactions_root();
            body_downloader.add_target(child, is_empty);
        }
        cinfo!(SYNC, "Sync extension initialized");
        Extension {
            state,
            requests: Default::default(),
            connected_nodes: Default::default(),
            header_downloaders: Default::default(),
            body_downloader,
            tokens: Default::default(),
            tokens_info: Default::default(),
            token_generator: TokenGenerator::new(SYNC_EXPIRE_TOKEN_BEGIN, SYNC_EXPIRE_TOKEN_END),
            client,
            api,
            last_request: Default::default(),
            nonce: Default::default(),
            snapshot_dir,
        }
    }

    fn move_state(&mut self) {
        let next_state = self.state.next(&self.client);
        cdebug!(SYNC, "Transitioning the state to {:?}", next_state);
        if discriminant(&next_state) == discriminant(&State::Full) {
            let best_hash = match &self.state {
                State::SnapshotHeader(hash, _) => *hash,
                State::SnapshotBody {
                    header,
                    ..
                } => header.hash(),
                State::SnapshotChunk {
                    block,
                    ..
                } => *block,
                State::Full => unreachable!("Trying to transition state from State::Full"),
            };
            self.client.force_update_best_block(&best_hash);
            for downloader in self.header_downloaders.values_mut() {
                downloader.update_pivot(best_hash);
            }
            self.send_status_broadcast();
        }
        self.state = next_state;
    }

    fn dismiss_request(&mut self, id: &NodeId, request_id: u64) {
        if let Some(requests) = self.requests.get_mut(id) {
            requests.retain(|(i, _)| *i != request_id);
        }
    }

    fn send_status(&mut self, id: &NodeId) {
        if discriminant(&self.state) != discriminant(&State::Full) {
            return
        }

        let chain_info = self.client.chain_info();
        self.api.send(
            id,
            Arc::new(
                Message::Status {
                    nonce: U256::from(self.nonce),
                    best_hash: chain_info.best_proposal_block_hash,
                    genesis_hash: chain_info.genesis_hash,
                }
                .rlp_bytes(),
            ),
        );
        self.nonce += 1;
    }

    fn send_status_broadcast(&mut self) {
        if discriminant(&self.state) != discriminant(&State::Full) {
            return
        }

        let chain_info = self.client.chain_info();
        for id in self.connected_nodes.iter() {
            self.api.send(
                id,
                Arc::new(
                    Message::Status {
                        nonce: U256::from(self.nonce),
                        best_hash: chain_info.best_proposal_block_hash,
                        genesis_hash: chain_info.genesis_hash,
                    }
                    .rlp_bytes(),
                ),
            );
            self.nonce += 1;
        }
    }

    fn send_header_request(&mut self, id: &NodeId, request: RequestMessage) {
        if let Some(requests) = self.requests.get_mut(id) {
            ctrace!(SYNC, "Send header request to {}", id);
            let request_id = self.last_request;
            self.last_request += 1;
            requests.push((request_id, request.clone()));
            self.api.send(id, Arc::new(Message::Request(request_id, request).rlp_bytes()));
        }
    }

    fn send_body_request(&mut self, id: &NodeId) {
        if let Some(downloader) = self.header_downloaders.get(&id) {
            if self.client.block_status(&BlockId::Hash(downloader.best_hash())) == BlockStatus::InChain {
                // Peer is lagging behind the local blockchain.
                // We don't need to request block bodies to this peer
                return
            }
        }

        self.check_sync_variable();
        if let Some(requests) = self.requests.get_mut(id) {
            let have_body_request = {
                requests.iter().any(|r| match r {
                    (_, RequestMessage::Bodies(..)) => true,
                    _ => false,
                })
            };
            if have_body_request {
                cdebug!(SYNC, "Wait body response");
                return
            }

            if let Some(request) = self.body_downloader.create_request() {
                cdebug!(SYNC, "Request body to {} {:?}", id, request);
                let request_id = self.last_request;
                self.last_request += 1;
                requests.push((request_id, request.clone()));
                self.api.send(id, Arc::new(Message::Request(request_id, request).rlp_bytes()));

                let token = &self.tokens[id];
                let token_info = self.tokens_info.get_mut(token).unwrap();

                let _ = self.api.clear_timer(*token);
                self.api
                    .set_timer_once(*token, Duration::from_millis(SYNC_EXPIRE_REQUEST_INTERVAL))
                    .expect("Timer set succeeds");
                token_info.request_id = Some(request_id);
            }
        }
        self.check_sync_variable();
    }

    fn send_chunk_request(&mut self, block: &BlockHash, root: &H256) {
        let have_chunk_request = self.requests.values().flatten().any(|r| match r {
            (_, RequestMessage::StateChunk(..)) => true,
            _ => false,
        });

        if !have_chunk_request {
            let mut peer_ids: Vec<_> = self.header_downloaders.keys().cloned().collect();
            peer_ids.shuffle(&mut thread_rng());
            if let Some(id) = peer_ids.first() {
                if let Some(requests) = self.requests.get_mut(&id) {
                    let req = RequestMessage::StateChunk(*block, vec![*root]);
                    cdebug!(SYNC, "Request chunk to {} {:?}", id, req);
                    let request_id = self.last_request;
                    self.last_request += 1;
                    requests.push((request_id, req.clone()));
                    self.api.send(id, Arc::new(Message::Request(request_id, req).rlp_bytes()));

                    let token = &self.tokens[id];
                    let token_info = self.tokens_info.get_mut(token).unwrap();

                    let _ = self.api.clear_timer(*token);
                    self.api
                        .set_timer_once(*token, Duration::from_millis(SYNC_EXPIRE_REQUEST_INTERVAL))
                        .expect("Timer set succeeds");
                    token_info.request_id = Some(request_id);
                }
            }
        }
    }

    fn check_sync_variable(&self) {
        let mut has_error = false;
        for id in self.header_downloaders.keys() {
            let requests = match self.requests.get(id) {
                Some(requests) => requests,
                None => continue,
            };

            let body_requests: Vec<RequestMessage> = requests
                .iter()
                .filter_map(|r| match r {
                    (_, msg @ RequestMessage::Bodies(..)) => Some(msg.clone()),
                    _ => None,
                })
                .collect();

            let chunk_requests: Vec<RequestMessage> = requests
                .iter()
                .filter_map(|r| match r {
                    (_, msg @ RequestMessage::StateChunk(..)) => Some(msg.clone()),
                    _ => None,
                })
                .collect();

            if body_requests.len() > 1 {
                cerror!(SYNC, "Body request length {} > 1, body_requests: {:?}", body_requests.len(), body_requests);
                has_error = true;
            }

            let token = &self.tokens[id];
            let token_info = &self.tokens_info[token];

            match (token_info.request_id, body_requests.len() + chunk_requests.len()) {
                (Some(_), 1) => {}
                (None, 0) => {}
                _ => {
                    cerror!(
                        SYNC,
                        "request_id: {:?}, body_requests.len(): {}, body_requests: {:?}, chunk_requests.len(): {}, chunk_requests: {:?}",
                        token_info.request_id,
                        body_requests.len(),
                        body_requests,
                        chunk_requests.len(),
                        chunk_requests
                    );
                    has_error = true;
                }
            }
        }

        debug_assert!(!has_error);
    }
}

impl NetworkExtension<Event> for Extension {
    fn name() -> &'static str {
        "block-propagation"
    }
    fn need_encryption() -> bool {
        false
    }

    fn versions() -> &'static [u64] {
        const VERSIONS: &[u64] = &[0];
        &VERSIONS
    }

    fn on_node_added(&mut self, id: &NodeId, _version: u64) {
        cinfo!(SYNC, "New peer detected #{}", id);
        self.send_status(id);

        let t = self.connected_nodes.insert(*id);
        debug_assert!(t, "{} is already added to peer list", id);

        let token = self.token_generator.gen().expect("Token generator is full");
        let token_info = TokenInfo {
            node_id: *id,
            request_id: None,
        };

        let t = self.requests.insert(*id, Vec::new());
        debug_assert_eq!(None, t);
        let t = self.tokens_info.insert(token, token_info);
        debug_assert_eq!(None, t);
        let t = self.tokens.insert(*id, token);
        debug_assert_eq!(None, t);
        debug_assert!(t.is_none());
    }

    fn on_node_removed(&mut self, id: &NodeId) {
        if self.connected_nodes.remove(id) {
            cinfo!(SYNC, "Peer removed #{}", id);

            self.header_downloaders.remove(id);

            for (_, request) in self.requests.remove(id).into_iter().flatten() {
                if let RequestMessage::Bodies(hashes) = request {
                    self.body_downloader.reset_downloading(&hashes);
                }
            }

            if let Some(token) = self.tokens.remove(id) {
                self.api.clear_timer(token).expect("Timer cancel failed");
                self.tokens_info.remove(&token);
                self.token_generator.restore(token);
            }
        }
    }

    fn on_message(&mut self, id: &NodeId, data: &[u8]) {
        if !self.requests.contains_key(id) {
            cdebug!(SYNC, "Message received after the node disconnected");
            debug_assert!(!self.tokens.contains_key(id));
            return
        }

        if let Ok(received_message) = Rlp::new(data).as_val() {
            match received_message {
                Message::Status {
                    nonce,
                    best_hash,
                    genesis_hash,
                } => self.on_peer_status(id, nonce, best_hash, genesis_hash),
                Message::Request(request_id, request) => self.on_peer_request(id, request_id, request),
                Message::Response(request_id, response) => self.on_peer_response(id, request_id, response),
            }
        } else {
            cinfo!(SYNC, "Invalid message from peer {}", id);
        }
    }

    fn on_timeout(&mut self, token: TimerToken) {
        match token {
            SYNC_TIMER_TOKEN => {
                let mut peer_ids: Vec<_> = self.header_downloaders.keys().cloned().collect();
                peer_ids.shuffle(&mut thread_rng());

                match self.state {
                    State::SnapshotHeader(_, num) => {
                        for id in &peer_ids {
                            self.send_header_request(id, RequestMessage::Headers {
                                start_number: num - 1,
                                max_count: 2,
                            });
                        }
                    }
                    State::SnapshotBody {
                        ref header,
                        ..
                    } => {
                        for id in &peer_ids {
                            if let Some(requests) = self.requests.get_mut(id) {
                                ctrace!(SYNC, "Send snapshot body request to {}", id);
                                let request = RequestMessage::Bodies(vec![header.hash()]);
                                let request_id = self.last_request;
                                self.last_request += 1;
                                requests.push((request_id, request.clone()));
                                self.api.send(id, Arc::new(Message::Request(request_id, request).rlp_bytes()));

                                let token = &self.tokens[id];
                                let token_info = self.tokens_info.get_mut(token).unwrap();

                                let _ = self.api.clear_timer(*token);
                                self.api
                                    .set_timer_once(*token, Duration::from_millis(SYNC_EXPIRE_REQUEST_INTERVAL))
                                    .expect("Timer set succeeds");
                                token_info.request_id = Some(request_id);
                            }
                        }
                    }
                    State::SnapshotChunk {
                        block,
                        ref mut restore,
                    } => {
                        if let Some(root) = restore.next_to_feed() {
                            self.send_chunk_request(&block, &root);
                        } else {
                            self.move_state();
                        }
                    }
                    State::Full => {
                        for id in &peer_ids {
                            let request =
                                self.header_downloaders.get_mut(id).and_then(HeaderDownloader::create_request);
                            if let Some(request) = request {
                                self.send_header_request(id, request);
                                break
                            }
                        }

                        for id in peer_ids {
                            self.send_body_request(&id);
                        }
                    }
                }
            }
            SYNC_EXPIRE_TOKEN_BEGIN..=SYNC_EXPIRE_TOKEN_END => {
                self.check_sync_variable();
                let (id, request_id) = {
                    let token_info = match self.tokens_info.get_mut(&token) {
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

                if let Some(requests) = self.requests.get_mut(&id) {
                    let expired_request = requests.iter().find(|(r, _)| *r == request_id);
                    if let Some((_, request)) = expired_request {
                        match request {
                            RequestMessage::Bodies(hashes) => {
                                self.body_downloader.reset_downloading(&hashes);
                            }
                            _ => unreachable!(),
                        }
                    }
                }

                self.dismiss_request(&id, request_id);
                self.check_sync_variable();
            }
            _ => unreachable!(),
        }
    }

    fn on_event(&mut self, event: Event) {
        match event {
            Event::GetPeers(channel) => {
                for peer in self.header_downloaders.keys() {
                    channel.send(*peer).unwrap();
                }
            }
            Event::NewHeaders {
                imported,
                enacted,
                retracted,
            } => {
                self.new_headers(imported, enacted, retracted);
            }
            Event::NewBlocks {
                imported,
                invalid,
            } => {
                self.new_blocks(imported, invalid);
            }
        }
    }
}

pub enum Event {
    GetPeers(EventSender<NodeId>),
    NewHeaders {
        imported: Vec<BlockHash>,
        enacted: Vec<BlockHash>,
        retracted: Vec<BlockHash>,
    },
    NewBlocks {
        imported: Vec<BlockHash>,
        invalid: Vec<BlockHash>,
    },
}

impl Extension {
    fn new_headers(&mut self, imported: Vec<BlockHash>, enacted: Vec<BlockHash>, retracted: Vec<BlockHash>) {
        if let State::Full = self.state {
            for peer in self.header_downloaders.values_mut() {
                peer.mark_as_imported(imported.clone());
            }
            let mut headers_to_download: Vec<_> = enacted
                .into_iter()
                .map(|hash| self.client.block_header(&BlockId::Hash(hash)).expect("Enacted header must exist"))
                .collect();
            headers_to_download.sort_unstable_by_key(EncodedHeader::number);
            #[allow(clippy::redundant_closure)]
            // False alarm. https://github.com/rust-lang/rust-clippy/issues/1439
            headers_to_download.dedup_by_key(|h| h.hash());

            let headers: Vec<_> = headers_to_download
                .into_iter()
                .filter(|header| self.client.block_body(&BlockId::Hash(header.hash())).is_none())
                .collect(); // FIXME: No need to collect here if self is not borrowed.
            for header in headers {
                let parent = self
                    .client
                    .block_header(&BlockId::Hash(header.parent_hash()))
                    .expect("Enacted header must have parent");
                let is_empty = header.transactions_root() == parent.transactions_root();
                self.body_downloader.add_target(&header.decode(), is_empty);
            }
            self.body_downloader.remove_target(&retracted);
        }
    }

    fn new_blocks(&mut self, imported: Vec<BlockHash>, invalid: Vec<BlockHash>) {
        self.body_downloader.remove_target(&imported);
        self.body_downloader.remove_target(&invalid);
        self.send_status_broadcast();
    }
}

impl Extension {
    fn on_peer_status(&mut self, from: &NodeId, nonce: U256, best_hash: BlockHash, genesis_hash: BlockHash) {
        // Validity check
        if genesis_hash != self.client.chain_info().genesis_hash {
            cinfo!(SYNC, "Genesis hash mismatch with peer {}", from);
            return
        }

        match self.header_downloaders.entry(*from) {
            Entry::Occupied(mut peer) => {
                if !peer.get_mut().update(nonce, best_hash) {
                    // FIXME: It should be an error level if the consensus is PoW.
                    cdebug!(SYNC, "Peer #{} status updated but score is less than before", from);
                    return
                }
            }
            Entry::Vacant(e) => {
                e.insert(HeaderDownloader::new(self.client.clone(), nonce, best_hash));
            }
        }
        cinfo!(SYNC, "Peer #{} status update: nonce: {}, best_hash: {}", from, nonce, best_hash);
    }

    fn on_peer_request(&self, from: &NodeId, id: u64, request: RequestMessage) {
        if !self.connected_nodes.contains(from) {
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
            } => {
                ctrace!(SYNC, "Received header request from {}", from);
                self.create_headers_response(start_number, max_count)
            }
            RequestMessage::Bodies(hashes) => {
                ctrace!(SYNC, "Received body request from {}", from);
                self.create_bodies_response(hashes)
            }
            RequestMessage::StateChunk(block_hash, chunk_root) => {
                self.create_state_chunk_response(block_hash, chunk_root)
            }
        };

        self.api.send(from, Arc::new(Message::Response(id, response).rlp_bytes()));
    }

    fn is_valid_request(&self, request: &RequestMessage) -> bool {
        match request {
            RequestMessage::Headers {
                ..
            } => true,
            RequestMessage::Bodies(hashes) => !hashes.is_empty(),
            RequestMessage::StateChunk {
                ..
            } => true,
        }
    }

    fn create_headers_response(&self, start_number: BlockNumber, max_count: u64) -> ResponseMessage {
        let best_proposal_header = self.client.best_proposal_header();
        let headers = (0..max_count)
            .map(|number| {
                let height = start_number + number;
                let block_id = if best_proposal_header.number() == height {
                    // If Engine != Tendermint
                    //    Best block == Best proposal block
                    //    We could get the best proposal block either by the block hash or the block number.
                    // If Engine == Tendermint
                    //    Best block = Best proposal block or its parent
                    //    We should get the best proposal block only by the block hash.
                    BlockId::Hash(best_proposal_header.hash())
                } else {
                    BlockId::Number(height)
                };
                self.client.block(&block_id)
            })
            .take_while(Option::is_some)
            .map(|block| block.expect("take_while guarantees existance of item").header().decode())
            .collect();
        ResponseMessage::Headers(headers)
    }

    fn create_bodies_response(&self, hashes: Vec<BlockHash>) -> ResponseMessage {
        let bodies = hashes
            .into_iter()
            .map(|hash| {
                self.client.block_body(&BlockId::Hash(hash)).map(|body| body.transactions()).unwrap_or_default()
            })
            .collect();
        ResponseMessage::Bodies(bodies)
    }

    fn create_state_chunk_response(&self, hash: BlockHash, chunk_roots: Vec<H256>) -> ResponseMessage {
        let mut result = Vec::new();
        for root in chunk_roots {
            if let Some(dir) = &self.snapshot_dir {
                let chunk_path = snapshot_path(&dir, &hash, &root);
                match fs::read(chunk_path) {
                    Ok(chunk) => result.push(chunk),
                    _ => result.push(Vec::new()),
                }
            }
        }
        ResponseMessage::StateChunk(result)
    }

    fn on_peer_response(&mut self, from: &NodeId, id: u64, mut response: ResponseMessage) {
        let last_request = self.requests[from].iter().find(|(i, _)| *i == id).cloned();
        if let Some((_, request)) = last_request {
            if let ResponseMessage::Headers(headers) = &mut response {
                headers.sort_unstable_by_key(Header::number);
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
                    self.check_sync_variable();
                    let hashes = match request {
                        RequestMessage::Bodies(hashes) => hashes,
                        _ => unreachable!(),
                    };
                    assert_eq!(bodies.len(), hashes.len());
                    if let Some(token) = self.tokens.get(from) {
                        if let Some(token_info) = self.tokens_info.get_mut(token) {
                            if token_info.request_id.is_none() {
                                ctrace!(SYNC, "Expired before handling response");
                                return
                            }
                            self.api.clear_timer(*token).expect("Timer clear succeed");
                            token_info.request_id = None;
                        }
                    }
                    self.dismiss_request(from, id);
                    self.on_body_response(hashes, bodies);
                    self.check_sync_variable();
                }
                ResponseMessage::StateChunk(chunks) => {
                    let roots = match request {
                        RequestMessage::StateChunk(_, roots) => roots,
                        _ => unreachable!(),
                    };
                    if let Some(token) = self.tokens.get(from) {
                        if let Some(token_info) = self.tokens_info.get_mut(token) {
                            if token_info.request_id.is_none() {
                                ctrace!(SYNC, "Expired before handling response");
                                return
                            }
                            self.api.clear_timer(*token).expect("Timer clear succeed");
                            token_info.request_id = None;
                        }
                    }
                    self.dismiss_request(from, id);
                    self.on_chunk_response(from, &roots, &chunks);
                }
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
                        cwarn!(SYNC, "Received headers are not a chain:\n  parent: (height: {}, hash: {}, parent: {}),\n  child: (height: {}, hash: {}, parent: {})",
                        parent.number(), parent.hash(), parent.parent_hash(), child.number(), child.hash(), child.parent_hash());
                        return false
                    }
                }

                headers.first().map(Header::number) == Some(*start_number)
            }
            (RequestMessage::Bodies(hashes), ResponseMessage::Bodies(bodies)) => {
                if hashes.len() != bodies.len() {
                    cwarn!(
                        SYNC,
                        "Received bodies' length({}) is not same with the requested hashes({})",
                        bodies.len(),
                        hashes.len()
                    );
                    return false
                }
                for body in bodies {
                    for tx in body {
                        let is_valid = match &tx.action {
                            Action::Custom {
                                handler_id,
                                ..
                            } => self.client.find_action_handler_for(*handler_id).is_some(),
                            _ => true,
                        };
                        if !is_valid {
                            cwarn!(SYNC, "Received transaction has some invalid actions");
                            return false
                        }
                    }
                }
                true
            }
            (RequestMessage::StateChunk(_, roots), ResponseMessage::StateChunk(chunks)) => {
                // Check length
                roots.len() == chunks.len()
            }
            _ => {
                cwarn!(SYNC, "Invalid response type");
                false
            }
        }
    }

    fn on_header_response(&mut self, from: &NodeId, headers: &[Header]) {
        ctrace!(SYNC, "Received header response from({}) with length({})", from, headers.len());
        match self.state {
            State::SnapshotHeader(hash, _) => match headers {
                [parent, header] if header.hash() == hash => {
                    match self.client.import_trusted_header(parent) {
                        Ok(_)
                        | Err(BlockImportError::Import(ImportError::AlreadyInChain))
                        | Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {}
                        Err(err) => {
                            cwarn!(SYNC, "Cannot import header({}): {:?}", parent.hash(), err);
                            return
                        }
                    }
                    match self.client.import_trusted_header(header) {
                        Ok(_)
                        | Err(BlockImportError::Import(ImportError::AlreadyInChain))
                        | Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {}
                        Err(err) => {
                            cwarn!(SYNC, "Cannot import header({}): {:?}", header.hash(), err);
                            return
                        }
                    }
                    self.move_state();
                }
                _ => cdebug!(
                    SYNC,
                    "Peer {} responded with a invalid response. requested hash: {}, response length: {}",
                    from,
                    hash,
                    headers.len()
                ),
            },
            State::SnapshotBody {
                ..
            } => {}
            State::SnapshotChunk {
                ..
            } => {}
            State::Full => {
                let (mut completed, peer_is_caught_up) = if let Some(peer) = self.header_downloaders.get_mut(from) {
                    let encoded: Vec<_> = headers.iter().map(|h| EncodedHeader::new(h.rlp_bytes().to_vec())).collect();
                    peer.import_headers(&encoded);
                    (peer.downloaded(), peer.is_caught_up())
                } else {
                    (Vec::new(), true)
                };
                completed.sort_unstable_by_key(EncodedHeader::number);

                let mut exists = Vec::new();
                let mut queued = Vec::new();

                for header in completed {
                    let hash = header.hash();
                    match self.client.import_header(header.clone().into_inner()) {
                        Err(BlockImportError::Import(ImportError::AlreadyInChain)) => exists.push(hash),
                        Err(BlockImportError::Import(ImportError::AlreadyQueued)) => queued.push(hash),
                        // FIXME: handle import errors
                        Err(err) => {
                            cwarn!(SYNC, "Cannot import header({}): {:?}", header.hash(), err);
                            break
                        }
                        _ => {}
                    }
                }

                let request = self.header_downloaders.get_mut(from).and_then(|peer| {
                    peer.mark_as_queued(queued);
                    peer.mark_as_imported(exists);
                    peer.create_request()
                });
                if !peer_is_caught_up {
                    if let Some(request) = request {
                        self.send_header_request(from, request);
                    }
                }
            }
        }
    }

    fn on_body_response(&mut self, hashes: Vec<BlockHash>, bodies: Vec<Vec<UnverifiedTransaction>>) {
        ctrace!(SYNC, "Received body response with lenth({}) {:?}", hashes.len(), hashes);

        match self.state {
            State::SnapshotBody {
                ref header,
                prev_root,
            } => {
                let body = bodies.first().expect("Body response in SnapshotBody state has only one body");
                let new_root = skewed_merkle_root(prev_root, body.iter().map(Encodable::rlp_bytes));
                if header.transactions_root() == new_root {
                    let block = Block {
                        header: header.decode(),
                        transactions: body.clone(),
                    };
                    match self.client.import_trusted_block(&block) {
                        Ok(_) | Err(BlockImportError::Import(ImportError::AlreadyInChain)) => {
                            self.move_state();
                        }
                        Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {}
                        // FIXME: handle import errors
                        Err(err) => {
                            cwarn!(SYNC, "Cannot import block({}): {:?}", header.hash(), err);
                        }
                    }
                }
            }
            State::Full => {
                {
                    self.body_downloader.import_bodies(hashes, bodies);
                    let completed = self.body_downloader.drain();
                    for (hash, transactions) in completed {
                        let header = self
                            .client
                            .block_header(&BlockId::Hash(hash))
                            .expect("Downloaded body's header must exist")
                            .decode();
                        let block = Block {
                            header,
                            transactions,
                        };
                        cdebug!(SYNC, "Body download completed for #{}({})", block.header.number(), hash);
                        match self.client.import_block(block.rlp_bytes(&Seal::With)) {
                            Err(BlockImportError::Import(ImportError::AlreadyInChain)) => {
                                cwarn!(SYNC, "Downloaded already existing block({})", hash)
                            }
                            Err(BlockImportError::Import(ImportError::AlreadyQueued)) => {
                                cwarn!(SYNC, "Downloaded already queued in the verification queue({})", hash)
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

                let mut peer_ids: Vec<_> = self.header_downloaders.keys().cloned().collect();
                peer_ids.shuffle(&mut thread_rng());

                for id in peer_ids {
                    self.send_body_request(&id);
                }
            }
            _ => {}
        }
    }

    fn on_chunk_response(&mut self, from: &NodeId, roots: &[H256], chunks: &[Vec<u8>]) {
        let (block, restore) = match self.state {
            State::SnapshotChunk {
                block,
                ref mut restore,
            } => (block, restore),
            _ => return,
        };
        for (r, c) in roots.iter().zip(chunks) {
            if c.is_empty() {
                cdebug!(SYNC, "Peer {} sent empty response for chunk request {}", from, r);
                continue
            }
            let decompressor = ChunkDecompressor::from_slice(c);
            let raw_chunk = match decompressor.decompress() {
                Ok(chunk) => chunk,
                Err(e) => {
                    cwarn!(SYNC, "Decode failed for chunk response from peer {}: {}", from, e);
                    continue
                }
            };
            let recovered = match raw_chunk.recover(*r) {
                Ok(chunk) => chunk,
                Err(e) => {
                    cwarn!(SYNC, "Invalid chunk response from peer {}: {}", from, e);
                    continue
                }
            };

            let batch = {
                let mut state_db = self.client.state_db().write();
                let hash_db = state_db.as_hashdb_mut();
                restore.feed(hash_db, recovered);

                let mut batch = DBTransaction::new();
                match state_db.journal_under(&mut batch, 0, H256::zero()) {
                    Ok(_) => batch,
                    Err(e) => {
                        cwarn!(SYNC, "Failed to write state chunk to database: {}", e);
                        continue
                    }
                }
            };
            self.client.db().write_buffered(batch);
            match self.client.db().flush() {
                Ok(_) => cdebug!(SYNC, "Wrote state chunk to database: {}", r),
                Err(e) => cwarn!(SYNC, "Failed to flush database: {}", e),
            }
        }

        if let Some(root) = restore.next_to_feed() {
            self.send_chunk_request(&block, &root);
        } else {
            self.move_state();
        }
    }
}

pub struct BlockSyncSender(EventSender<Event>);

impl From<EventSender<Event>> for BlockSyncSender {
    fn from(sender: EventSender<Event>) -> Self {
        BlockSyncSender(sender)
    }
}

impl ChainNotify for BlockSyncSender {
    fn new_headers(
        &self,
        imported: Vec<BlockHash>,
        _invalid: Vec<BlockHash>,
        enacted: Vec<BlockHash>,
        retracted: Vec<BlockHash>,
        _sealed: Vec<BlockHash>,
        _new_best_proposal: Option<BlockHash>,
    ) {
        self.0
            .send(Event::NewHeaders {
                imported,
                enacted,
                retracted,
            })
            .unwrap();
    }

    fn new_blocks(
        &self,
        imported: Vec<BlockHash>,
        invalid: Vec<BlockHash>,
        _enacted: Vec<BlockHash>,
        _retracted: Vec<BlockHash>,
        _sealed: Vec<BlockHash>,
    ) {
        self.0
            .send(Event::NewBlocks {
                imported,
                invalid,
            })
            .unwrap();
    }
}
