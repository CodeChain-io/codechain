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

use std::collections::hash_map::Iter;
use std::collections::{HashMap, VecDeque};

use super::super::session::Session;
use super::super::{NodeId, SocketAddr};

pub struct SessionCandidate {
    registered: HashMap<NodeId, (Session, SocketAddr)>,
    prepared: VecDeque<HashMap<NodeId, (Session, SocketAddr)>>,
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

    pub fn contains_registered(&self, node_id: &NodeId) -> bool {
        self.registered.contains_key(node_id)
    }

    pub fn contains_key(&self, node_id: &NodeId) -> bool {
        if self.registered.contains_key(node_id) {
            return true
        }
        self.prepared.iter().any(|v| v.contains_key(node_id))
    }

    pub fn get(&self, node_id: &NodeId) -> Option<&(Session, SocketAddr)> {
        let regisstered = self.registered.get(node_id);
        if regisstered.is_some() {
            return regisstered
        }

        for u in self.prepared.iter() {
            let u = u.get(node_id);
            if u.is_some() {
                return u
            }
        }
        None
    }

    pub fn iter(&self) -> Iter<NodeId, (Session, SocketAddr)> {
        self.registered.iter()
    }

    pub fn remove(&mut self, node_id: &NodeId) -> bool {
        let removed = self.registered.remove(node_id);
        if removed.is_some() {
            return true
        }

        for mut u in self.prepared.iter_mut() {
            if u.remove(node_id).is_some() {
                return true
            }
        }
        return false
    }

    pub fn insert(&mut self, node_id: NodeId, session: Session, socket_address: SocketAddr) -> bool {
        if self.contains_key(&node_id) {
            return false
        }
        let unmatured = self.prepared.front_mut().expect("It must be exist");
        let t = unmatured.insert(node_id, (session, socket_address));
        debug_assert!(t.is_none());
        true
    }

    pub fn promote(&mut self) {
        let unmatured = self.prepared.pop_back().expect("It must be exist");
        self.prepared.push_front(Default::default());
        for (node_id, candidate) in unmatured {
            let session = self.registered.insert(node_id, candidate);
            debug_assert!(session.is_none());
        }
    }
}

#[cfg(test)]
mod tests {
    use ctypes::Secret;

    use super::super::super::session::Nonce;
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
        let node_id0 = 123456.into();
        let t = candidates.insert(node_id0, session0, socket_address0);
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
