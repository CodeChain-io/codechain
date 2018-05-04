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

use std::collections::{HashMap, VecDeque};

use super::super::session::{Nonce, Session};
use super::super::SocketAddr;

pub struct SessionCandidate {
    registered: HashMap<Nonce, (Session, SocketAddr)>,
    prepared: VecDeque<HashMap<Nonce, (Session, SocketAddr)>>,
}

impl SessionCandidate {
    pub fn new(step: usize) -> Self {
        let mut unmatured = VecDeque::with_capacity(step);
        for _ in 0..step {
            unmatured.push_back(Default::default());
        }
        Self {
            registered: Default::default(),
            prepared: unmatured,
        }
    }

    pub fn contains_registered(&self, nonce: &Nonce) -> bool {
        self.registered.contains_key(nonce)
    }

    pub fn contains_key(&self, nonce: &Nonce) -> bool {
        if self.registered.contains_key(nonce) {
            return true
        }
        self.prepared.iter().any(|v| v.contains_key(nonce))
    }

    pub fn remove(&mut self, nonce: &Nonce) -> bool {
        let removed = self.registered.remove(nonce);
        if removed.is_some() {
            return true
        }

        for mut u in self.prepared.iter_mut() {
            if u.remove(nonce).is_some() {
                return true
            }
        }
        return false
    }

    pub fn insert(&mut self, session: Session, socket_address: SocketAddr) -> bool {
        if self.contains_key(session.id()) {
            return false
        }
        let unmatured = self.prepared.front_mut().expect("It must be exist");
        let t = unmatured.insert(session.id().clone(), (session, socket_address));
        debug_assert!(t.is_none());
        true
    }

    pub fn promote(&mut self) {
        let unmatured = self.prepared.pop_back().expect("It must be exist");
        self.prepared.push_front(Default::default());
        for (nonce, candidate) in unmatured {
            let session = self.registered.insert(nonce, candidate);
            debug_assert!(session.is_none());
        }
    }
}

#[cfg(test)]
mod tests {
    use ctypes::Secret;

    use super::*;

    #[test]
    fn promote() {
        const STEP: usize = 3;
        let mut candidates = SessionCandidate::new(STEP);
        assert!(candidates.registered.is_empty());
        assert_eq!(STEP, candidates.prepared.len());
        assert!(candidates.prepared.get(0).unwrap().is_empty());
        assert!(candidates.prepared.get(1).unwrap().is_empty());
        assert!(candidates.prepared.get(2).unwrap().is_empty());

        let secret0 = Secret::zero();
        let nonce0 = Nonce::zero();
        let session0 = Session::new(secret0, nonce0);
        let socket_address0 = SocketAddr::v4(127, 0, 0, 1, 8000);
        let t = candidates.insert(session0, socket_address0);
        assert!(t);

        assert!(candidates.registered.is_empty());
        assert_eq!(STEP, candidates.prepared.len());
        assert_eq!(1, candidates.prepared.get(0).unwrap().len());
        assert!(candidates.prepared.get(1).unwrap().is_empty());
        assert!(candidates.prepared.get(2).unwrap().is_empty());

        candidates.promote();
        assert!(candidates.registered.is_empty());
        assert_eq!(STEP, candidates.prepared.len());
        assert!(candidates.prepared.get(0).unwrap().is_empty());
        assert_eq!(1, candidates.prepared.get(1).unwrap().len());
        assert!(candidates.prepared.get(2).unwrap().is_empty());

        candidates.promote();
        assert!(candidates.registered.is_empty());
        assert_eq!(STEP, candidates.prepared.len());
        assert!(candidates.prepared.get(0).unwrap().is_empty());
        assert!(candidates.prepared.get(1).unwrap().is_empty());
        assert_eq!(1, candidates.prepared.get(2).unwrap().len());

        candidates.promote();
        assert_eq!(1, candidates.registered.len());
        assert_eq!(STEP, candidates.prepared.len());
        assert!(candidates.prepared.get(0).unwrap().is_empty());
        assert!(candidates.prepared.get(1).unwrap().is_empty());
        assert!(candidates.prepared.get(2).unwrap().is_empty());

        candidates.promote();
        assert_eq!(1, candidates.registered.len());
        assert_eq!(STEP, candidates.prepared.len());
        assert!(candidates.prepared.get(0).unwrap().is_empty());
        assert!(candidates.prepared.get(1).unwrap().is_empty());
        assert!(candidates.prepared.get(2).unwrap().is_empty());
    }
}
