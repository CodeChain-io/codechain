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

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ckeys::{exchange, Generator, KeyPair, Public, Random};
use ctypes::Secret;
use parking_lot::{Mutex, RwLock};
use rand::{OsRng, Rng};
use rlp::{Decodable, Encodable, UntrustedRlp};

use super::session::{Nonce, Session};
use super::{NodeId, SocketAddr};

pub struct RoutingTable {
    // Only the addresses are known
    candidates: RwLock<HashSet<SocketAddr>>,

    // addresses that shares node id
    uninitializeds: RwLock<HashSet<SocketAddr>>,

    // remote node id => key pair
    key_pairs: RwLock<HashMap<SocketAddr, KeyPair>>,

    // remote node id -> shared secret
    shared_secrets: RwLock<HashMap<SocketAddr, Secret>>,

    // remote node id -> temporary nonce
    temporary_nonces: RwLock<HashMap<SocketAddr, Nonce>>,

    // remote node id -> Session
    unestablished_sessions: RwLock<HashMap<SocketAddr, Session>>,

    established: RwLock<HashSet<SocketAddr>>,

    // remote node id => local node id
    // One node can have multiple node ids because the machine can has a multiple ip addresses
    // This field represents the local node id that remote node thinks.
    remote_to_local_node_ids: RwLock<HashMap<NodeId, NodeId>>,

    rng: Mutex<OsRng>,
}

impl RoutingTable {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            candidates: RwLock::new(HashSet::new()),
            uninitializeds: RwLock::new(HashSet::new()),

            key_pairs: RwLock::new(HashMap::new()),
            shared_secrets: RwLock::new(HashMap::new()),
            temporary_nonces: RwLock::new(HashMap::new()),
            unestablished_sessions: RwLock::new(HashMap::new()),
            established: RwLock::new(HashSet::new()),

            remote_to_local_node_ids: RwLock::new(HashMap::new()),

            rng: Mutex::new(OsRng::new().unwrap()),
        })
    }

    pub fn all_addresses(&self) -> HashSet<SocketAddr> {
        let uninitialized = self.uninitializeds.read();
        let key_pairs = self.key_pairs.read();
        let shared_secrets = self.shared_secrets.read();
        let unestablished_sessions = self.unestablished_sessions.read();
        let established = self.established.read();

        uninitialized
            .iter()
            .cloned()
            .chain(key_pairs.keys().cloned())
            .chain(shared_secrets.keys().cloned())
            .chain(unestablished_sessions.keys().cloned())
            .chain(established.iter().cloned())
            .collect()
    }

    pub fn add_candidate(&self, addr: SocketAddr) -> bool {
        let mut candidates = self.candidates.write();
        let uninitialized = self.uninitializeds.read();
        let key_pair = self.key_pairs.read();
        let shared_secret = self.shared_secrets.read();
        let temporary_nonce = self.temporary_nonces.read();
        let unestablished_sessions = self.unestablished_sessions.read();
        let established = self.established.read();

        if candidates.contains(&addr) {
            return false
        }

        if uninitialized.contains(&addr) {
            return false
        }
        if key_pair.contains_key(&addr) {
            return false
        }
        if shared_secret.contains_key(&addr) {
            return false
        }
        if temporary_nonce.contains_key(&addr) {
            return false
        }
        if unestablished_sessions.contains_key(&addr) {
            return false
        }
        if established.contains(&addr) {
            return false
        }

        let t = candidates.insert(addr);
        debug_assert!(t);
        return true
    }

    pub fn remove_node(&self, addr: SocketAddr) -> bool {
        let mut candidates = self.candidates.write();
        let mut uninitializeds = self.uninitializeds.write();
        let mut key_pairs = self.key_pairs.write();
        let mut shared_secrets = self.shared_secrets.write();
        let mut temporary_nonces = self.temporary_nonces.write();
        let mut unestablished_sessions = self.unestablished_sessions.write();
        let mut established = self.established.write();
        let mut remote_to_local_node_ids = self.remote_to_local_node_ids.write();

        if candidates.remove(&addr) {
            return true
        }
        let remote_node_id = addr.clone().into();
        let removed = remote_to_local_node_ids.remove(&remote_node_id).is_some();

        if uninitializeds.remove(&addr) {
            debug_assert!(removed);
            return true
        }
        if key_pairs.remove(&addr).is_some() {
            debug_assert!(removed);
            return true
        }
        if shared_secrets.remove(&addr).is_some() {
            debug_assert!(removed);
            return true
        }
        if temporary_nonces.remove(&addr).is_some() {
            debug_assert!(removed);
            return true
        }
        if unestablished_sessions.remove(&addr).is_some() {
            debug_assert!(removed);
            return true
        }
        if established.remove(&addr) {
            debug_assert!(removed);
            return true
        }

        debug_assert!(!removed);
        false
    }

    pub fn add_node(&self, addr: &SocketAddr, local_node_id: NodeId) -> bool {
        let mut candidates = self.candidates.write();
        let mut uninitializeds = self.uninitializeds.write();
        let mut remote_to_local_node_ids = self.remote_to_local_node_ids.write();

        candidates.remove(addr);
        if !uninitializeds.insert(addr.clone()) {
            let remote_node_id: NodeId = addr.into();
            debug_assert!(remote_to_local_node_ids.contains_key(&remote_node_id));
            return false
        }


        let remote_node_id: NodeId = addr.into();

        match remote_to_local_node_ids.insert(remote_node_id, local_node_id) {
            None => cinfo!(NET, "{:?} thinks my node id is {}", addr, local_node_id),
            Some(previous_local_node_id) if previous_local_node_id != local_node_id => {
                cinfo!(NET, "{:?} changes my node id {} to {}", addr, previous_local_node_id, local_node_id)
            }
            _ => {}
        }
        true
    }

    pub fn register_key_pair_for_secret(&self, remote_address: &SocketAddr) -> Option<Public> {
        let mut uninitializeds = self.uninitializeds.write();
        let mut key_pairs = self.key_pairs.write();

        if !uninitializeds.remove(remote_address) {
            return None
        }

        let ephemeral = Random.generate().unwrap();
        let pub_key = ephemeral.public().clone();
        let t = key_pairs.insert(remote_address.clone(), ephemeral);
        debug_assert!(t.is_none());
        Some(pub_key)
    }

    pub fn reset_key_pair_for_secret(&self, remote_address: &SocketAddr) -> bool {
        let mut candidates = self.candidates.write();
        let mut key_pairs = self.key_pairs.write();

        if let None = key_pairs.remove(remote_address) {
            return false
        }

        let t = candidates.insert(remote_address.clone());
        debug_assert!(t);
        true
    }

    pub fn share_secret(&self, remote_address: &SocketAddr, remote_public: &Public) -> Option<Secret> {
        let mut key_pairs = self.key_pairs.write();
        let mut shared_secrets = self.shared_secrets.write();

        key_pairs
            .remove(remote_address)
            .and_then(|local_key_pair| exchange(remote_public, local_key_pair.private()).ok())
            .map(|secret| {
                let t = shared_secrets.insert(remote_address.clone(), secret.clone());
                debug_assert!(t.is_none());
                secret
            })
    }

    pub fn request_session(&self, remote_address: &SocketAddr) -> Option<Vec<u8>> {
        let shared_secrets = self.shared_secrets.read();
        let mut temporary_nonces = self.temporary_nonces.write();
        let mut rng = self.rng.lock();

        if !shared_secrets.contains_key(remote_address) {
            return None
        }
        let shared_secret = shared_secrets.get(remote_address).unwrap();
        let temporary_nonce: Nonce = rng.gen();
        let t = temporary_nonces.insert(remote_address.clone(), temporary_nonce.clone());
        debug_assert!(t.is_none());

        let temporary_session = Session::new_with_zero_nonce(shared_secret.clone());
        encode_and_encrypt_nonce(&temporary_session, &temporary_nonce)
    }

    pub fn create_requested_session(
        &self,
        remote_address: &SocketAddr,
        encrypted_temporary_nonce: &[u8],
    ) -> Option<Vec<u8>> {
        let mut shared_secrets = self.shared_secrets.write();
        let temporary_nonces = self.temporary_nonces.read();
        let mut unestablished_sessions = self.unestablished_sessions.write();
        let mut rng = self.rng.lock();

        if temporary_nonces.contains_key(remote_address) {
            return None
        }
        debug_assert!(shared_secrets.contains_key(remote_address));

        let secret = shared_secrets.remove(remote_address).unwrap();

        let temporary_session = {
            let temporary_zero_session = Session::new_with_zero_nonce(secret.clone());
            let temporary_nonce = decrypt_and_decode_nonce(&temporary_zero_session, encrypted_temporary_nonce)?;
            Session::new(secret.clone(), temporary_nonce)
        };

        let nonce: Nonce = rng.gen();
        let encrypted_nonce = encode_and_encrypt_nonce(&temporary_session, &nonce);

        let t = unestablished_sessions.insert(remote_address.clone(), Session::new(secret, nonce));
        debug_assert!(t.is_none());
        encrypted_nonce
    }

    pub fn create_allowed_session(&self, remote_address: &SocketAddr, received_nonce: &[u8]) -> bool {
        let mut shared_secrets = self.shared_secrets.write();
        let mut temporary_nonces = self.temporary_nonces.write();
        let mut unestablished_sessions = self.unestablished_sessions.write();

        if !temporary_nonces.contains_key(remote_address) {
            return false
        }
        debug_assert!(shared_secrets.contains_key(remote_address));

        let secret = shared_secrets.get(remote_address).unwrap().clone();
        let temporary_nonce = temporary_nonces.get(remote_address).unwrap().clone();

        let temporary_session = Session::new(secret.clone(), temporary_nonce);
        let nonce = match decrypt_and_decode_nonce(&temporary_session, &received_nonce) {
            Some(nonce) => nonce,
            None => return false,
        };
        let t = shared_secrets.remove(remote_address);
        debug_assert!(t.is_some());
        let t = temporary_nonces.remove(remote_address);
        debug_assert!(t.is_some());

        let session = Session::new(secret, nonce);
        let t = unestablished_sessions.insert(remote_address.clone(), session);
        debug_assert!(t.is_none());
        true
    }

    pub fn establish(&self, remote_address: &SocketAddr) -> bool {
        let mut unestablished_sessions = self.unestablished_sessions.write();
        let mut established = self.established.write();

        if !unestablished_sessions.contains_key(remote_address) {
            return false
        }
        debug_assert!(!established.contains(remote_address));

        let t = unestablished_sessions.remove(remote_address);
        debug_assert!(t.is_some());
        established.insert(remote_address.clone());
        true
    }

    pub fn unestablished_session(&self, remote_address: &SocketAddr) -> Option<Session> {
        let unestablished_sessions = self.unestablished_sessions.read();

        unestablished_sessions.get(&remote_address).cloned()
    }

    pub fn unestablished_addresses(&self, len: usize) -> Vec<SocketAddr> {
        let unestablished_sessions = self.unestablished_sessions.read();
        unestablished_sessions.keys().take(len).cloned().collect()
    }

    pub fn local_node_id(&self, remote_node_id: &NodeId) -> Option<NodeId> {
        let remote_to_local_node_ids = self.remote_to_local_node_ids.read();

        remote_to_local_node_ids.get(&remote_node_id).cloned()
    }

    pub fn candidates(&self, len: &usize) -> Vec<SocketAddr> {
        let candidates = self.candidates.read();
        let mut rng = self.rng.lock();

        let mut addresses = candidates.iter().cloned().collect::<Vec<_>>();
        rng.shuffle(&mut addresses);
        addresses.into_iter().take(*len).collect()
    }
}

fn decrypt_and_decode_nonce(session: &Session, encrypted_bytes: &[u8]) -> Option<Nonce> {
    session.decrypt(&encrypted_bytes).ok().and_then(|unencrypted_bytes| {
        let rlp = UntrustedRlp::new(&unencrypted_bytes);
        Decodable::decode(&rlp).ok()
    })
}

fn encode_and_encrypt_nonce(session: &Session, nonce: &Nonce) -> Option<Vec<u8>> {
    let encoded_nonce = nonce.rlp_bytes();
    session.encrypt(&encoded_nonce).ok()
}
