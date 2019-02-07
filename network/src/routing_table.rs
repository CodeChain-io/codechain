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

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ckey::{exchange, Generator, KeyPair, Public, Random, Secret};
use parking_lot::{Mutex, RwLock};
use rand::rngs::OsRng;
use rand::Rng;
use rlp::{Decodable, Encodable, UntrustedRlp};

use crate::session::{Nonce, Session};
use crate::SocketAddr;

#[derive(Clone, Copy, Debug, PartialEq)]
enum SecretOrigin {
    Shared,
    Preimported,
}

// Discovery flow : Candidate -> KeyPairShared -> SecretShared -> TemporaryNonceShared -> SessionShared -> (Establishing) -> Established
// Offline secret exchange flow :                 SecretShared ->
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "cargo-clippy", allow(clippy::large_enum_variant))]
enum State {
    Candidate,
    KeyPairShared(KeyPair),
    SecretShared {
        local_key_pair: KeyPair,
        remote_public: Public,
        shared_secret: Secret,
        secret_origin: SecretOrigin,
    },
    TemporaryNonceShared {
        local_key_pair: KeyPair,
        remote_public: Public,
        shared_secret: Secret,
        temporary_nonce: Nonce,
        secret_origin: SecretOrigin,
    },
    SessionShared {
        local_key_pair: KeyPair,
        remote_public: Public,
        shared_secret: Secret,
        nonce: Nonce,
        secret_origin: SecretOrigin,
    },
    Establishing {
        local_key_pair: KeyPair,
        remote_public: Public,
        shared_secret: Secret,
        nonce: Nonce,
        secret_origin: SecretOrigin,
    },
    Established {
        local_key_pair: KeyPair,
        remote_public: Public,
        shared_secret: Secret,
        nonce: Nonce,
        secret_origin: SecretOrigin,
    },
    Banned,
}

impl State {
    fn key_pair_shared() -> RwLock<Self> {
        let ephemeral = Random.generate().unwrap();
        RwLock::new(State::KeyPairShared(ephemeral))
    }

    fn local_public(&self) -> Option<&Public> {
        match self {
            State::Candidate => None,
            State::KeyPairShared(key_pair) => Some(key_pair.public()),
            State::SecretShared {
                local_key_pair,
                ..
            } => Some(local_key_pair.public()),
            State::TemporaryNonceShared {
                local_key_pair,
                ..
            } => Some(local_key_pair.public()),
            State::SessionShared {
                local_key_pair,
                ..
            } => Some(local_key_pair.public()),
            State::Establishing {
                local_key_pair,
                ..
            } => Some(local_key_pair.public()),
            State::Established {
                local_key_pair,
                ..
            } => Some(local_key_pair.public()),
            State::Banned => None,
        }
    }

    fn remote_public(&self) -> Option<&Public> {
        match self {
            State::Candidate => None,
            State::KeyPairShared(_) => None,
            State::SecretShared {
                remote_public,
                ..
            } => Some(remote_public),
            State::TemporaryNonceShared {
                remote_public,
                ..
            } => Some(remote_public),
            State::SessionShared {
                remote_public,
                ..
            } => Some(remote_public),
            State::Establishing {
                remote_public,
                ..
            } => Some(remote_public),
            State::Established {
                remote_public,
                ..
            } => Some(remote_public),
            State::Banned => None,
        }
    }
}

pub struct RoutingTable {
    entries: RwLock<HashMap<SocketAddr, RwLock<State>>>,

    rng: Mutex<OsRng>,
}

impl RoutingTable {
    #![cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entries: RwLock::new(HashMap::new()),
            rng: Mutex::new(OsRng::new().unwrap()),
        })
    }

    pub fn is_secret_shared(&self, addr: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(addr) {
            let state = entry.read();
            match *state {
                State::Candidate => false,
                State::KeyPairShared(_) => false,
                _ => true,
            }
        } else {
            true
        }
    }

    pub fn reset_imported_secret(&self, addr: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(addr) {
            let mut state = entry.write();
            match *state {
                State::TemporaryNonceShared {
                    local_key_pair,
                    remote_public,
                    shared_secret,
                    secret_origin,
                    ..
                } if secret_origin == SecretOrigin::Preimported => {
                    cinfo!(NETWORK, "{} does not load secret", addr);
                    *state = State::SecretShared {
                        local_key_pair,
                        remote_public,
                        shared_secret,
                        secret_origin,
                    };
                    return true
                }
                _ => return false,
            }
        }
        false
    }

    pub fn all_addresses(&self) -> HashSet<SocketAddr> {
        let entries = self.entries.read();
        entries.keys().cloned().collect()
    }

    pub fn reachable_addresses(&self, from: &SocketAddr) -> HashSet<SocketAddr> {
        let entries = self.entries.read();
        entries.keys().filter(|addr| from.is_reachable(addr)).cloned().collect()
    }

    pub fn is_connected(&self, addr: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(addr) {
            let state = entry.read();
            match *state {
                State::Established {
                    ..
                } => return true,
                _ => return false,
            }
        }
        false
    }

    pub fn add_candidate(&self, addr: SocketAddr) -> bool {
        let mut entries = self.entries.write();
        if entries.contains_key(&addr) {
            ctrace!(ROUTING_TABLE, "{} is already in table", addr);
            return false
        }
        let t = entries.insert(addr, RwLock::new(State::Candidate));
        debug_assert!(t.is_none());
        ctrace!(ROUTING_TABLE, "Candidate added {}", addr);
        true
    }

    pub fn remove_node(&self, addr: &SocketAddr) -> bool {
        self.remove_node_internal(addr, false)
    }

    pub fn remove_node_on_shutdown(&self, addr: &SocketAddr) -> bool {
        self.remove_node_internal(addr, true)
    }

    fn remove_node_internal(&self, addr: &SocketAddr, on_shutdown: bool) -> bool {
        let mut entries = self.entries.write();

        if let Some(entry) = entries.get(addr) {
            let state = entry.read();
            match *state {
                State::Banned => return false,
                State::SessionShared {
                    ..
                } => {
                    if on_shutdown {
                        return false
                    }
                }
                _ => {}
            }
        }
        let result = entries.remove(addr).is_some();
        if result {
            ctrace!(ROUTING_TABLE, "Remove {}", addr);
        }
        result
    }

    pub fn register_key_pair_for_secret(&self, remote_address: SocketAddr) -> Option<Public> {
        let mut entries = self.entries.write();
        let entry = entries.entry(remote_address).or_insert_with(|| RwLock::new(State::Candidate));
        let mut state = entry.write();
        if *state == State::Candidate {
            let ephemeral = Random.generate().unwrap();
            let pub_key = *ephemeral.public();
            ctrace!(ROUTING_TABLE, "Share pub-key({}) with {}", pub_key, remote_address);
            *state = State::KeyPairShared(ephemeral);
        }
        state.local_public().cloned()
    }

    pub fn share_secret(&self, remote_address: SocketAddr, remote_public: Public) -> Option<Public> {
        let mut entries = self.entries.write();
        let entry = entries.entry(remote_address).or_insert_with(State::key_pair_shared);
        let mut state = entry.write();
        let local_public = state.local_public().cloned();
        if let Some(registered_remote_public) = state.remote_public() {
            if *registered_remote_public == remote_public {
                return Some(local_public.expect("The local key always exists when a remote public key exists."))
            }
        }
        if local_public.is_some() {
            if let State::KeyPairShared(local_key_pair) = state.clone() {
                if let Ok(shared_secret) = exchange(&remote_public, local_key_pair.private()) {
                    *state = State::SecretShared {
                        local_key_pair,
                        remote_public,
                        shared_secret,
                        secret_origin: SecretOrigin::Shared,
                    };
                    ctrace!(ROUTING_TABLE, "Secret shared with {}", remote_address);
                    return Some(*local_key_pair.public())
                }
            }
        }
        ctrace!(ROUTING_TABLE, "Cannot share secret with {}", remote_address);
        None
    }

    pub fn request_session(&self, remote_address: &SocketAddr) -> Option<Vec<u8>> {
        let entries = self.entries.read();
        let mut rng = self.rng.lock();

        let result = entries.get(remote_address).and_then(|entry| {
            let mut state = entry.write();
            let (shared_secret, secret_origin, local_key_pair, remote_public) = match *state {
                State::SecretShared {
                    shared_secret,
                    secret_origin,
                    local_key_pair,
                    remote_public,
                } => (shared_secret, secret_origin, local_key_pair, remote_public),
                _ => return None,
            };

            let temporary_nonce: Nonce = rng.gen();
            *state = State::TemporaryNonceShared {
                local_key_pair,
                remote_public,
                shared_secret,
                temporary_nonce,
                secret_origin,
            };
            let temporary_session = Session::new_with_zero_nonce(shared_secret);
            let result = encode_and_encrypt_nonce(&temporary_session, &temporary_nonce);
            if result.is_some() {
                ctrace!(ROUTING_TABLE, "Temporary nonce shared with {}", remote_address);
            }
            result
        });
        if result.is_none() {
            ctrace!(ROUTING_TABLE, "Cannot share temporary nonce with {}", remote_address);
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

        let result = entries.get(remote_address).and_then(|entry| {
            let mut state = entry.write();
            let (shared_secret, secret_origin, local_key_pair, remote_public) = match *state {
                State::SecretShared {
                    shared_secret,
                    secret_origin,
                    local_key_pair,
                    remote_public,
                    ..
                } => (shared_secret, secret_origin, local_key_pair, remote_public),
                _ => return None,
            };
            let temporary_session = {
                let temporary_zero_session = Session::new_with_zero_nonce(shared_secret);
                let temporary_nonce = decrypt_and_decode_nonce(&temporary_zero_session, encrypted_temporary_nonce)?;
                Session::new(shared_secret, temporary_nonce)
            };

            let nonce: Nonce = rng.gen();
            *state = State::SessionShared {
                local_key_pair,
                remote_public,
                shared_secret,
                nonce,
                secret_origin,
            };

            let encrypted_nonce = encode_and_encrypt_nonce(&temporary_session, &nonce);
            if encrypted_nonce.is_some() {
                ctrace!(ROUTING_TABLE, "Create session to {}", remote_address);
            }
            encrypted_nonce
        });
        if result.is_none() {
            ctrace!(ROUTING_TABLE, "Cannot create session to {}", remote_address);
        }
        result
    }

    pub fn create_allowed_session(&self, remote_address: &SocketAddr, received_nonce: &[u8]) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(remote_address) {
            let mut state = entry.write();
            if let State::TemporaryNonceShared {
                local_key_pair,
                remote_public,
                shared_secret,
                temporary_nonce,
                secret_origin,
            } = state.clone()
            {
                let temporary_session = Session::new(shared_secret, temporary_nonce);
                let nonce = match decrypt_and_decode_nonce(&temporary_session, &received_nonce) {
                    Some(nonce) => nonce,
                    None => {
                        ctrace!(ROUTING_TABLE, "Cannot allow session to {}. Cannot decrypt nonce", remote_address);
                        return false
                    }
                };

                *state = State::SessionShared {
                    local_key_pair,
                    remote_public,
                    shared_secret,
                    nonce,
                    secret_origin,
                };
                ctrace!(ROUTING_TABLE, "Allow session to {}", remote_address);
                return true
            }
        }
        ctrace!(ROUTING_TABLE, "Cannot allow session to {}. Invalid state", remote_address);
        false
    }

    pub fn set_establishing(&self, remote_address: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(remote_address) {
            let mut state = entry.write();
            if let State::SessionShared {
                local_key_pair,
                remote_public,
                shared_secret,
                nonce,
                secret_origin,
            } = *state
            {
                *state = State::Establishing {
                    local_key_pair,
                    remote_public,
                    shared_secret,
                    nonce,
                    secret_origin,
                };
                ctrace!(ROUTING_TABLE, "Connection to {} set establishing", remote_address);
                return true
            }
        }
        ctrace!(ROUTING_TABLE, "Cannot set connection to {} as establishing", remote_address);
        false
    }

    pub fn establish(&self, remote_address: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(remote_address) {
            let mut state = entry.write();
            match *state {
                State::SessionShared {
                    local_key_pair,
                    remote_public,
                    shared_secret,
                    nonce,
                    secret_origin,
                } => {
                    *state = State::Established {
                        local_key_pair,
                        remote_public,
                        shared_secret,
                        nonce,
                        secret_origin,
                    };
                    ctrace!(ROUTING_TABLE, "Connection to {} is established", remote_address);
                    return true
                }
                State::Establishing {
                    local_key_pair,
                    remote_public,
                    shared_secret,
                    nonce,
                    secret_origin,
                } => {
                    *state = State::Established {
                        local_key_pair,
                        remote_public,
                        shared_secret,
                        nonce,
                        secret_origin,
                    };
                    ctrace!(ROUTING_TABLE, "Connection to {} is established", remote_address);
                    return true
                }
                _ => {}
            }
        }
        ctrace!(ROUTING_TABLE, "Cannot establish connection to {}", remote_address);
        false
    }

    pub fn reset_session(&self, remote_address: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(remote_address) {
            let mut state = entry.write();
            if let State::Establishing {
                local_key_pair,
                remote_public,
                shared_secret,
                nonce,
                secret_origin,
            } = *state
            {
                *state = State::SessionShared {
                    local_key_pair,
                    remote_public,
                    shared_secret,
                    nonce,
                    secret_origin,
                };
                ctrace!(ROUTING_TABLE, "Connection to {} is ready to reconnect", remote_address);
                return true
            }
        }
        ctrace!(ROUTING_TABLE, "Cannot reset connection to {}, because it's not establishing", remote_address);
        false
    }

    pub fn ban(&self, remote_address: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(remote_address) {
            let mut state = entry.write();
            *state = State::Banned;
            return true
        }
        false
    }

    pub fn unban(&self, remote_address: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(remote_address) {
            let mut state = entry.write();
            if *state == State::Banned {
                *state = State::Candidate;
                return true
            }
        }
        false
    }

    pub fn unestablished_session(&self, remote_address: &SocketAddr) -> Option<Session> {
        let entries = self.entries.read();
        if let Some(entry) = entries.get(remote_address) {
            let state = entry.read();
            if let State::SessionShared {
                shared_secret,
                nonce,
                ..
            } = *state
            {
                ctrace!(ROUTING_TABLE, "Unestablish connection to {}", remote_address);
                return Some(Session::new(shared_secret, nonce))
            }
        }
        ctrace!(ROUTING_TABLE, "Connection to {} is not established yet", remote_address);
        None
    }

    pub fn unestablished_addresses(&self, len: usize) -> Vec<SocketAddr> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|(_remote_node_id, entry)| {
                let state = entry.read();
                match *state {
                    State::SessionShared {
                        ..
                    } => true,
                    _ => false,
                }
            })
            .take(len)
            .map(|(remote_node_id, _entry)| *remote_node_id)
            .collect()
    }

    pub fn candidates(&self, len: usize) -> Vec<SocketAddr> {
        let entries = self.entries.read();
        let mut rng = self.rng.lock();

        let mut addresses = entries
            .iter()
            .filter(|(_remote_node_id, entry)| {
                let state = entry.read();
                State::Candidate == *state
            })
            .map(|(remote_node_id, _entry)| *remote_node_id)
            .collect::<Vec<_>>();

        rng.shuffle(&mut addresses);
        addresses.into_iter().take(len).collect::<Vec<_>>()
    }
}

fn decrypt_and_decode_nonce(session: &Session, encrypted_bytes: &[u8]) -> Option<Nonce> {
    session
        .decrypt(&encrypted_bytes)
        .map_err(|e| {
            ctrace!(ROUTING_TABLE, "Cannot decode nonce {:?}", e);
            e
        })
        .ok()
        .and_then(|unencrypted_bytes| {
            let rlp = UntrustedRlp::new(&unencrypted_bytes);
            Decodable::decode(&rlp)
                .map_err(|e| {
                    ctrace!(ROUTING_TABLE, "Cannot decrypt nonce {:?}", e);
                    e
                })
                .ok()
        })
}

fn encode_and_encrypt_nonce(session: &Session, nonce: &Nonce) -> Option<Vec<u8>> {
    let encoded_nonce = nonce.rlp_bytes();
    session
        .encrypt(&encoded_nonce)
        .map_err(|e| {
            ctrace!(ROUTING_TABLE, "Cannot encrypt nonce {:?}", e);
            e
        })
        .ok()
}
