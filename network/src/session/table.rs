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

use super::session::{Session, SharedSecret};
use super::super::SocketAddr;

pub struct Table {
    table: HashMap<SocketAddr, Session>,
}

impl Table {
    pub fn new() -> Self {
        Self {
            table: Default::default(),
        }
    }

    pub fn get(&self, k: &SocketAddr) -> Option<Session> {
        self.table.get(&k).map(|s| s.clone()).or_else(|| {
            // FIXME
            let mut s = Session::new(SharedSecret::zero());
            s.set_ready(10000);
            Some(s)
        })
    }

    pub fn get_mut(&mut self, k: &SocketAddr) -> Option<&mut Session> {
        self.table.get_mut(&k)
    }

    pub fn contains_key(&self, k: &SocketAddr) -> bool {
        self.table.contains_key(&k)
    }

    pub fn insert(&mut self, k: SocketAddr, v: Session) -> Option<Session> {
        self.table.insert(k, v)
    }

    pub fn remove(&mut self, k: &SocketAddr) -> Option<Session> {
        self.table.remove(k)
    }
}
