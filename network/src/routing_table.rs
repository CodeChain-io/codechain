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

use std::collections::HashMap;
use std::sync::Arc;

use ccrypto::aes::{self, SymmetricCipherError};
use ckey::{exchange, Generator, KeyPair, Public, Random, Secret};
use parking_lot::{Mutex, RwLock};
use primitives::Bytes;
use rand::rngs::OsRng;
use rand::Rng;

use crate::session::{Nonce, Session};
use crate::SocketAddr;

#[derive(Debug, PartialEq, Clone, Copy)]
enum SecretOrigin {
    Shared,
    Preimported,
}

// Candidate -> Registered -> Establishing2 -> Established
//                 ->         Establishing1 -> Established
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "cargo-clippy", allow(clippy::large_enum_variant))]
enum State {
    Candidate(KeyPair),
    Registered {
        local_key_pair: KeyPair,
        remote_public: Public,
        secret_origin: SecretOrigin,
    },
    Establishing1(KeyPair),
    Establishing2 {
        local_key_pair: KeyPair,
        remote_public: Public,
        shared_secret: Secret,
        secret_origin: SecretOrigin,
    },
    Established {
        local_key_pair: KeyPair,
        remote_public: Public,
        shared_secret: Secret,
        secret_origin: SecretOrigin,
        nonce: Nonce,
    },
    Banned,
}

impl State {
    fn local_public(&self) -> Option<&Public> {
        match self {
            State::Candidate(key_pair) => Some(key_pair.public()),
            State::Registered {
                local_key_pair,
                ..
            } => Some(local_key_pair.public()),
            State::Establishing1(key_pair) => Some(key_pair.public()),
            State::Establishing2 {
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
            State::Candidate(_) => None,
            State::Registered {
                remote_public,
                ..
            } => Some(remote_public),
            State::Establishing1(_) => None,
            State::Establishing2 {
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

    fn session(&self) -> Option<Session> {
        match self {
            State::Established {
                nonce,
                shared_secret,
                ..
            } => Some(Session::new(*shared_secret, *nonce)),
            _ => None,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        let ephemeral = Random.generate().unwrap();
        State::Candidate(ephemeral)
    }
}

pub struct RoutingTable {
    entries: RwLock<HashMap<SocketAddr, State>>,

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

    pub fn is_establishing_or_established(&self, target: &SocketAddr) -> bool {
        let entries = self.entries.read();
        match entries.get(target) {
            Some(State::Establishing1 {
                ..
            }) => true,
            Some(State::Establishing2 {
                ..
            }) => true,
            Some(State::Established {
                ..
            }) => true,
            _ => false,
        }
    }

    pub fn is_established(&self, target: &SocketAddr) -> bool {
        let entries = self.entries.read();
        if let Some(State::Established {
            ..
        }) = entries.get(target)
        {
            true
        } else {
            false
        }
    }

    pub fn is_establishing(&self, target: &SocketAddr) -> bool {
        let entries = self.entries.read();
        match entries.get(target) {
            Some(State::Establishing1 {
                ..
            }) => true,
            Some(State::Establishing2 {
                ..
            }) => true,
            _ => false,
        }
    }

    pub fn all_addresses(&self) -> Vec<SocketAddr> {
        let entries = self.entries.read();
        entries.keys().cloned().collect()
    }

    pub fn candidates(&self) -> Vec<SocketAddr> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter_map(|(addr, state)| match state {
                State::Candidate(_) => Some(addr),
                State::Registered {
                    ..
                } => Some(addr),
                _ => None,
            })
            .cloned()
            .collect()
    }

    pub fn established_addresses(&self) -> Vec<SocketAddr> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter_map(|(addr, state)| {
                if let State::Established {
                    ..
                } = state
                {
                    Some(addr)
                } else {
                    None
                }
            })
            .cloned()
            .collect()
    }

    pub fn reachable_addresses(&self, from: &SocketAddr) -> Vec<SocketAddr> {
        let entries = self.entries.read();
        entries.keys().filter(|addr| from.is_reachable(addr)).cloned().collect()
    }

    pub fn touch(&self, target: SocketAddr) -> Option<Public> {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        entry.local_public().cloned()
    }

    pub fn touch_addresses<I: IntoIterator<Item = SocketAddr>>(&self, targets: I) {
        let mut entries = self.entries.write();
        for target in targets.into_iter() {
            entries.entry(target).or_default();
        }
    }

    pub fn register_remote_public(&self, target: SocketAddr, remote: Public) -> Option<Public> {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        let new_state = match entry {
            State::Candidate(local_key_pair)
            | State::Registered {
                local_key_pair,
                ..
            } => State::Registered {
                local_key_pair: *local_key_pair,
                remote_public: remote,
                secret_origin: SecretOrigin::Preimported,
            },
            _ => return None,
        };
        *entry = new_state;
        Some(*entry.local_public().expect("Registered state must have local public"))
    }

    pub fn reset_local_key(&self, target: SocketAddr) -> bool {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        let new_state = match entry {
            State::Candidate(_) => State::default(),
            State::Registered {
                remote_public,
                secret_origin,
                ..
            } => {
                let local_key_pair = Random.generate().unwrap();
                State::Registered {
                    local_key_pair,
                    remote_public: *remote_public,
                    secret_origin: *secret_origin,
                }
            }
            _ => return false,
        };
        *entry = new_state;
        true
    }

    pub fn try_establish(&self, target: SocketAddr) -> Result<Option<Public>, String> {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        let new_state = match entry {
            State::Candidate(local_key_pair) => State::Establishing1(*local_key_pair),
            State::Registered {
                local_key_pair,
                remote_public,
                secret_origin,
            } => {
                let shared_secret = exchange(remote_public, local_key_pair.private())
                    .map_err(|e| format!("Cannot exchange key: {:?}", e))?;
                State::Establishing2 {
                    local_key_pair: *local_key_pair,
                    remote_public: *remote_public,
                    shared_secret,
                    secret_origin: *secret_origin,
                }
            }
            _ => return Err("Invalid state".to_string()),
        };
        *entry = new_state;
        Ok(entry.remote_public().cloned())
    }

    pub fn set_recipient_establish1(
        &self,
        target: SocketAddr,
        received_remote_public: Public,
    ) -> Result<Option<(Bytes, Public, Session)>, String> {
        let mut entries = self.entries.write();
        let mut rng = self.rng.lock();
        let entry = entries.entry(target).or_default();
        let (new_state, shared_secret, nonce, local_public) = match entry {
            State::Candidate(local_key_pair) => {
                let nonce = rng.gen();
                let shared_secret = exchange(&received_remote_public, local_key_pair.private())
                    .map_err(|e| format!("Cannot exchange key: {:?}", e))?;
                (
                    State::Established {
                        local_key_pair: *local_key_pair,
                        remote_public: received_remote_public,
                        shared_secret,
                        secret_origin: SecretOrigin::Shared,
                        nonce,
                    },
                    shared_secret,
                    nonce,
                    *local_key_pair.public(),
                )
            }
            State::Registered {
                local_key_pair,
                remote_public,
                secret_origin,
            } => {
                if *remote_public != received_remote_public {
                    return Err(format!(
                        "Unexpected remote public received. expected: {}, got: {}",
                        remote_public, received_remote_public
                    ))
                }
                let nonce = rng.gen();
                let shared_secret = exchange(remote_public, local_key_pair.private())
                    .map_err(|e| format!("Cannot exchange key: {:?}", e))?;
                (
                    State::Established {
                        local_key_pair: *local_key_pair,
                        remote_public: *remote_public,
                        shared_secret,
                        secret_origin: *secret_origin,
                        nonce,
                    },
                    shared_secret,
                    nonce,
                    *local_key_pair.public(),
                )
            }
            State::Establishing1(_) => return Ok(None),
            State::Establishing2 {
                remote_public,
                ..
            } => {
                if *remote_public != received_remote_public {
                    return Err(format!(
                        "Unexpected remote public received. expected: {}, got: {}",
                        remote_public, received_remote_public
                    ))
                }
                return Ok(None)
            }
            _ => return Err("Cannot establish a connection for Recipient".to_string()),
        };
        let encrypted_nonce =
            encrypt_nonce(nonce, &shared_secret).map_err(|e| format!("Cannot encrypt nonce: {:?}", e))?;
        *entry = new_state;
        Ok(Some((encrypted_nonce, local_public, entry.session().expect("Established connection must have a session"))))
    }

    pub fn set_recipient_establish2(
        &self,
        target: SocketAddr,
        received_local_public: Public,
        received_remote_public: Public,
    ) -> Result<Option<(Bytes, Public, Session)>, String> {
        let mut entries = self.entries.write();
        let mut rng = self.rng.lock();
        let entry = entries.entry(target).or_default();
        let (new_state, shared_secret, nonce, local_public) = match entry {
            State::Candidate(local_key_pair) => {
                if received_local_public != *local_key_pair.public() {
                    return Err(format!(
                        "Unexpected local public received. expected: {}, got: {}",
                        local_key_pair.public(),
                        received_local_public
                    ))
                }
                let nonce = rng.gen();
                let shared_secret = exchange(&received_remote_public, local_key_pair.private())
                    .map_err(|e| format!("Cannot exchange key: {:?}", e))?;
                (
                    State::Established {
                        local_key_pair: *local_key_pair,
                        remote_public: received_remote_public,
                        shared_secret,
                        secret_origin: SecretOrigin::Shared,
                        nonce,
                    },
                    shared_secret,
                    nonce,
                    *local_key_pair.public(),
                )
            }
            State::Registered {
                local_key_pair,
                remote_public,
                secret_origin,
            } => {
                if received_local_public != *local_key_pair.public() {
                    return Err(format!(
                        "Unexpected local public received. expected: {}, got: {}",
                        local_key_pair.public(),
                        received_local_public
                    ))
                }
                if *remote_public != received_remote_public {
                    return Err(format!(
                        "Unexpected remote public received. expected: {}, got: {}",
                        remote_public, received_remote_public
                    ))
                }
                let nonce = rng.gen();
                let shared_secret = exchange(remote_public, local_key_pair.private())
                    .map_err(|e| format!("Cannot exchange key: {:?}", e))?;
                (
                    State::Established {
                        local_key_pair: *local_key_pair,
                        remote_public: *remote_public,
                        shared_secret,
                        secret_origin: *secret_origin,
                        nonce,
                    },
                    shared_secret,
                    nonce,
                    *local_key_pair.public(),
                )
            }
            State::Establishing1(local_key_pair) => {
                if received_local_public != *local_key_pair.public() {
                    return Err(format!(
                        "Unexpected local public received. expected: {}, got: {}",
                        local_key_pair.public(),
                        received_local_public
                    ))
                }
                return Ok(None)
            }
            State::Establishing2 {
                local_key_pair,
                remote_public,
                ..
            } => {
                if received_local_public != *local_key_pair.public() {
                    return Err(format!(
                        "Unexpected local public received. expected: {}, got: {}",
                        local_key_pair.public(),
                        received_local_public
                    ))
                }
                if *remote_public != received_remote_public {
                    return Err(format!(
                        "Unexpected remote public received. expected: {}, got: {}",
                        remote_public, received_remote_public
                    ))
                }
                return Ok(None)
            }
            _ => return Err("Cannot establish a connection for Recipient".to_string()),
        };
        let encrypted_nonce =
            encrypt_nonce(nonce, &shared_secret).map_err(|e| format!("Cannot encrypt nonce: {:?}", e))?;
        *entry = new_state;
        Ok(Some((encrypted_nonce, local_public, entry.session().expect("Established connection must have a session"))))
    }

    pub fn set_initiator_establish(
        &self,
        target: SocketAddr,
        remote_public: Public,
        encrypted_nonce: &[u8],
    ) -> Result<Session, String> {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        let new_state = match entry {
            State::Establishing1(local_key_pair) => {
                let shared_secret = exchange(&remote_public, local_key_pair.private())
                    .map_err(|e| format!("Cannot exchange key: {:?}", e))?;
                let nonce = decrypt_nonce(encrypted_nonce, &shared_secret)
                    .map_err(|e| format!("Cannot decrypt nonce: {:?}", e))?;
                State::Established {
                    local_key_pair: *local_key_pair,
                    remote_public,
                    shared_secret,
                    secret_origin: SecretOrigin::Shared,
                    nonce,
                }
            }
            State::Establishing2 {
                local_key_pair,
                remote_public: reserved_remote_public,
                shared_secret,
                secret_origin,
            } => {
                if remote_public != *reserved_remote_public {
                    return Err(format!(
                        "Ack with an unexepected remote key. expected: {}, got: {}",
                        reserved_remote_public, remote_public
                    ))
                }
                debug_assert_eq!(*shared_secret, exchange(&remote_public, local_key_pair.private()).unwrap());
                let nonce = decrypt_nonce(encrypted_nonce, &shared_secret)
                    .map_err(|e| format!("Cannot decrypt nonce: {:?}", e))?;
                State::Established {
                    local_key_pair: *local_key_pair,
                    remote_public,
                    shared_secret: *shared_secret,
                    secret_origin: *secret_origin,
                    nonce,
                }
            }
            _ => return Err("Initiator is not Establishing1".to_string()),
        };
        *entry = new_state;
        Ok(entry.session().expect("Established connection must have a session"))
    }

    pub fn reset_initiator_establish(&self, target: SocketAddr) -> Result<(), String> {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        let new_state = match entry {
            State::Establishing1(local_key_pair) => State::Candidate(*local_key_pair),
            State::Establishing2 {
                local_key_pair,
                remote_public,
                secret_origin,
                ..
            } => State::Registered {
                local_key_pair: *local_key_pair,
                remote_public: *remote_public,
                secret_origin: *secret_origin,
            },
            _ => return Err("Initiator is not Establishing1".to_string()),
        };
        *entry = new_state;
        Ok(())
    }

    // true if the connection is established
    pub fn ban(&self, target: SocketAddr) -> bool {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        let mut new_state = State::Banned;
        std::mem::swap(&mut new_state, entry);
        if let State::Established {
            ..
        } = new_state
        {
            true
        } else {
            false
        }
    }

    pub fn unban(&self, target: SocketAddr) -> bool {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        match entry {
            State::Banned => {}
            _ => return false,
        }
        *entry = State::default();
        true
    }

    pub fn remove(&self, target: &SocketAddr) -> bool {
        let mut entries = self.entries.write();
        if let Some(&State::Banned) = entries.get(target) {
            return false
        }
        entries.remove(target);
        true
    }

    pub fn local_public(&self, target: SocketAddr) -> Option<Public> {
        let mut entries = self.entries.write();
        let entry = entries.entry(target).or_default();
        entry.local_public().cloned()
    }
}

fn decrypt_nonce(encrypted_bytes: &[u8], shared_secret: &Secret) -> Result<Nonce, SymmetricCipherError> {
    let iv = 0; // FIXME: Use proper iv
    let unecrypted = aes::decrypt(encrypted_bytes, shared_secret, &iv)?;
    debug_assert_eq!(std::mem::size_of::<Nonce>(), 16);
    if unecrypted.len() != 16 {
        return Err(SymmetricCipherError::InvalidLength) // FIXME
    }
    let mut nonce_bytes = [0u8; 16];
    nonce_bytes.copy_from_slice(&unecrypted);
    Ok(Nonce::from_be_bytes(nonce_bytes))
}

fn encrypt_nonce(nonce: Nonce, shared_secret: &Secret) -> Result<Bytes, SymmetricCipherError> {
    let iv = 0; // FIXME: Use proper iv
    Ok(aes::encrypt(&nonce.to_be_bytes(), shared_secret, &iv)?)
}
