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

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ckeys::{exchange, Generator, KeyPair, Public, Random};
use ctypes::Secret;
use parking_lot::{Mutex, RwLock};
use rand::{OsRng, Rng};
use rlp::{Decodable, Encodable, UntrustedRlp};

use super::session::{Nonce, Session};
use super::{IntoSocketAddr, NodeId, SocketAddr};

#[derive(Clone, Debug, PartialEq)]
enum State {
    Intermediate,
    Candidate,
    Alive,
    KeyPairShared(KeyPair),
    SecretShared(Secret),
    TemporaryNonceShared(Secret, Nonce),
    SessionShared(Session),
    Established(NodeId),
}

pub struct RoutingTable {
    entries: RwLock<HashMap<NodeId, Mutex<Cell<State>>>>,

    // remote node id => local node id
    // One node can have multiple node ids because the machine can has a multiple ip addresses
    // This field represents the local node id that remote node thinks.
    remote_to_local_node_ids: RwLock<HashMap<NodeId, NodeId>>,

    rng: Mutex<OsRng>,
}

impl RoutingTable {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entries: RwLock::new(HashMap::new()),
            remote_to_local_node_ids: RwLock::new(HashMap::new()),
            rng: Mutex::new(OsRng::new().unwrap()),
        })
    }

    pub fn all_addresses(&self) -> HashSet<SocketAddr> {
        let entries = self.entries.read();
        entries.keys().map(|node_id| node_id.into_addr()).collect()
    }

    pub fn add_candidate(&self, addr: SocketAddr) -> bool {
        let mut entries = self.entries.write();
        let remote_node_id = addr.into();
        if entries.contains_key(&remote_node_id) {
            ctrace!(ROUTING_TABLE, "{:?} is already in table", addr);
            return false
        }
        let t = entries.insert(remote_node_id, Mutex::new(Cell::new(State::Candidate)));
        debug_assert!(t.is_none());
        ctrace!(ROUTING_TABLE, "Candidate added {:?}", addr);
        true
    }

    pub fn remove_node(&self, addr: SocketAddr) -> bool {
        let mut entries = self.entries.write();
        let mut remote_to_local_node_ids = self.remote_to_local_node_ids.write();

        let remote_node_id = addr.into();
        let result = entries.remove(&remote_node_id).is_some();
        if result {
            remote_to_local_node_ids.remove(&remote_node_id);
            ctrace!(ROUTING_TABLE, "Remove {:?}", addr);
        }
        result
    }

    pub fn add_node(&self, addr: &SocketAddr, local_node_id: NodeId) -> bool {
        let mut entries = self.entries.write();
        let mut remote_to_local_node_ids = self.remote_to_local_node_ids.write();

        let remote_node_id = addr.into();

        if let Some(entry) = entries.get(&remote_node_id) {
            let entry = entry.lock();
            let old_state = entry.replace(State::Intermediate);
            if old_state != State::Candidate {
                entry.set(old_state);
                ctrace!(ROUTING_TABLE, "{:?} is already alive", addr);
                return false
            }
            entry.set(State::Alive);

            let t = remote_to_local_node_ids.insert(remote_node_id, local_node_id);
            assert_eq!(None, t);
            ctrace!(ROUTING_TABLE, "Mark {:?} alive", addr);
            return true
        }

        let t = entries.insert(remote_node_id, Mutex::new(Cell::new(State::Alive)));
        debug_assert!(t.is_none());
        let t = remote_to_local_node_ids.insert(remote_node_id, local_node_id);
        assert_eq!(None, t);
        ctrace!(ROUTING_TABLE, "Add {:?} as alive", addr);
        true
    }

    pub fn register_key_pair_for_secret(&self, remote_address: &SocketAddr) -> Option<Public> {
        let entries = self.entries.read();
        let remote_node_id = remote_address.into();
        let result = entries.get(&remote_node_id).and_then(|entry| {
            let entry = entry.lock();
            let old_state = entry.replace(State::Intermediate);
            if old_state != State::Alive {
                entry.set(old_state);
                return None
            }
            let ephemeral = Random.generate().unwrap();
            let pub_key = ephemeral.public().clone();
            entry.set(State::KeyPairShared(ephemeral));
            ctrace!(ROUTING_TABLE, "Share pub-key({}) with {:?}", pub_key, remote_address);
            Some(pub_key)
        });
        if result.is_none() {
            ctrace!(ROUTING_TABLE, "{:?} is not alive", remote_address);
        }
        result
    }

    pub fn share_secret(&self, remote_address: &SocketAddr, remote_public: &Public) -> Option<Secret> {
        let entries = self.entries.read();
        let remote_node_id = remote_address.into();
        let result = entries.get(&remote_node_id).and_then(|entry| {
            let entry = entry.lock();
            let old_state = entry.replace(State::Intermediate);
            if let State::KeyPairShared(local_key_pair) = &old_state {
                if let Some(secret) = exchange(remote_public, local_key_pair.clone().private()).ok() {
                    entry.set(State::SecretShared(secret.clone()));
                    ctrace!(ROUTING_TABLE, "Secret shared with {:?}", remote_address);
                    return Some(secret)
                }
            }
            entry.set(old_state);
            None
        });
        if result.is_none() {
            ctrace!(ROUTING_TABLE, "Cannot share secret with {:?}", remote_address);
        }
        result
    }

    pub fn request_session(&self, remote_address: &SocketAddr) -> Option<Vec<u8>> {
        let entries = self.entries.read();
        let mut rng = self.rng.lock();

        let remote_node_id = remote_address.into();
        let result = entries.get(&remote_node_id).and_then(|entry| {
            let entry = entry.lock();
            let old_state = entry.replace(State::Intermediate);
            if let State::SecretShared(shared_secret) = &old_state {
                let temporary_nonce: Nonce = rng.gen();
                entry.set(State::TemporaryNonceShared(shared_secret.clone(), temporary_nonce.clone()));
                let temporary_session = Session::new_with_zero_nonce(shared_secret.clone());
                let result = encode_and_encrypt_nonce(&temporary_session, &temporary_nonce);
                if result.is_some() {
                    ctrace!(ROUTING_TABLE, "Temporary nonce shared with {:?}", remote_address);
                }
                return result
            }
            entry.set(old_state);
            None
        });
        if result.is_none() {
            ctrace!(ROUTING_TABLE, "Cannot share temporary nonce with {:?}", remote_address);
        }
        result
    }

    pub fn create_requested_session(
        &self,
        remote_address: &SocketAddr,
        encrypted_temporary_nonce: &[u8],
    ) -> Option<Vec<u8>> {
        let entries = self.entries.read();
        let mut rng = self.rng.lock();

        let remote_node_id = remote_address.into();
        let result = entries.get(&remote_node_id).and_then(|entry| {
            let entry = entry.lock();
            let old_state = entry.replace(State::Intermediate);
            if let State::SecretShared(shared_secret) = old_state {
                let temporary_session = {
                    let temporary_zero_session = Session::new_with_zero_nonce(shared_secret.clone());
                    let temporary_nonce = decrypt_and_decode_nonce(&temporary_zero_session, encrypted_temporary_nonce)?;
                    Session::new(shared_secret.clone(), temporary_nonce)
                };

                let nonce: Nonce = rng.gen();
                entry.set(State::SessionShared(Session::new(shared_secret, nonce.clone())));

                let encrypted_nonce = encode_and_encrypt_nonce(&temporary_session, &nonce);
                if encrypted_nonce.is_some() {
                    ctrace!(ROUTING_TABLE, "Create session to {:?}", remote_address);
                }
                return encrypted_nonce
            }
            entry.set(old_state);
            None
        });
        if result.is_none() {
            ctrace!(ROUTING_TABLE, "Cannot create session to {:?}", remote_address);
        }
        result
    }

    pub fn create_allowed_session(&self, remote_address: &SocketAddr, received_nonce: &[u8]) -> bool {
        let entries = self.entries.read();
        let remote_node_id = remote_address.into();
        if let Some(entry) = entries.get(&remote_node_id) {
            let entry = entry.lock();
            let old_state = entry.replace(State::Intermediate);
            if let State::TemporaryNonceShared(shared_secret, temporary_nonce) = old_state.clone() {
                let temporary_session = Session::new(shared_secret.clone(), temporary_nonce);
                let nonce = match decrypt_and_decode_nonce(&temporary_session, &received_nonce) {
                    Some(nonce) => nonce,
                    None => {
                        entry.set(old_state);
                        ctrace!(ROUTING_TABLE, "Cannot allow session to {:?}. Cannot decrypt nonce", remote_address);
                        return false
                    }
                };

                entry.set(State::SessionShared(Session::new(shared_secret, nonce)));
                ctrace!(ROUTING_TABLE, "Allow session to {:?}", remote_address);
                return true
            }
            entry.set(old_state);
        }
        ctrace!(ROUTING_TABLE, "Cannot allow session to {:?}. Invalid state", remote_address);
        false
    }

    pub fn establish(&self, remote_address: &SocketAddr) -> bool {
        let entries = self.entries.read();
        let remote_node_id = remote_address.into();
        if let Some(entry) = entries.get(&remote_node_id) {
            let entry = entry.lock();
            let old_state = entry.replace(State::Intermediate);
            if let State::SessionShared(_) = old_state {
                entry.set(State::Established(remote_node_id));
                ctrace!(ROUTING_TABLE, "Connection to {:?} established", remote_address);
                return true
            }
            entry.set(old_state);
        }
        ctrace!(ROUTING_TABLE, "Cannot establish connection to {:?} established", remote_address);
        false
    }

    pub fn unestablished_session(&self, remote_address: &SocketAddr) -> Option<Session> {
        let entries = self.entries.read();
        let remote_node_id = remote_address.into();
        if let Some(entry) = entries.get(&remote_node_id) {
            let entry = entry.lock();
            let old_state = entry.replace(State::Intermediate);
            if let State::SessionShared(session) = old_state {
                entry.set(State::SessionShared(session.clone()));
                ctrace!(ROUTING_TABLE, "Unestablish connection to {:?}", remote_address);
                return Some(session)
            }
            entry.set(old_state);
        }
        ctrace!(ROUTING_TABLE, "Connection to {:?} is not established yet", remote_address);
        None
    }

    pub fn unestablished_addresses(&self, len: usize) -> Vec<SocketAddr> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|(_remote_node_id, entry)| {
                let entry = entry.lock();
                let old_state = entry.replace(State::Intermediate);
                if let State::SessionShared(_) = old_state {
                    entry.set(old_state);
                    return true
                }
                entry.set(old_state);
                false
            })
            .take(len)
            .map(|(remote_node_id, _entry)| remote_node_id.into_addr())
            .collect()
    }

    pub fn local_node_id(&self, remote_node_id: &NodeId) -> Option<NodeId> {
        let remote_to_local_node_ids = self.remote_to_local_node_ids.read();

        remote_to_local_node_ids.get(&remote_node_id).cloned()
    }

    pub fn candidates(&self, len: &usize) -> Vec<SocketAddr> {
        let entries = self.entries.read();
        let mut rng = self.rng.lock();

        let mut addresses = entries
            .iter()
            .filter(|(_remote_node_id, entry)| {
                let entry = entry.lock();
                let old_state = entry.replace(State::Intermediate);
                let result = State::Candidate == old_state;
                entry.set(old_state);
                result
            })
            .map(|(remote_node_id, _entry)| remote_node_id.into_addr())
            .collect::<Vec<_>>();

        rng.shuffle(&mut addresses);
        addresses.into_iter().take(*len).collect::<Vec<_>>()
    }
}

fn decrypt_and_decode_nonce(session: &Session, encrypted_bytes: &[u8]) -> Option<Nonce> {
    session
        .decrypt(&encrypted_bytes)
        .map_err(|err| {
            ctrace!(ROUTING_TABLE, "Cannot decode nonce {:?}", err);
            err
        })
        .ok()
        .and_then(|unencrypted_bytes| {
            let rlp = UntrustedRlp::new(&unencrypted_bytes);
            Decodable::decode(&rlp)
                .map_err(|err| {
                    ctrace!(ROUTING_TABLE, "Cannot decrypt nonce {:?}", err);
                    err
                })
                .ok()
        })
}

fn encode_and_encrypt_nonce(session: &Session, nonce: &Nonce) -> Option<Vec<u8>> {
    let encoded_nonce = nonce.rlp_bytes();
    session
        .encrypt(&encoded_nonce)
        .map_err(|err| {
            ctrace!(ROUTING_TABLE, "Cannot encrypt nonce {:?}", err);
            err
        })
        .ok()
}
