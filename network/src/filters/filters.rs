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

use std::net::IpAddr;
use std::sync::Arc;

use parking_lot::RwLock;

use super::control::Control;
use super::filter::{Filter, FilterEntry};

pub struct Filters {
    whitelist: RwLock<Filter>,
    blacklist: RwLock<Filter>,
}

impl Filters {
    pub fn new(whitelist_vector: Vec<FilterEntry>, blacklist_vector: Vec<FilterEntry>) -> Arc<Self> {
        let whitelist = Filter::new(whitelist_vector);
        let blacklist = Filter::new(blacklist_vector);

        Arc::new(Self {
            whitelist: RwLock::new(whitelist),
            blacklist: RwLock::new(blacklist),
        })
    }
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            whitelist: RwLock::new(Default::default()),
            blacklist: RwLock::new(Default::default()),
        }
    }
}

impl Control for Filters {
    fn add_to_whitelist(&self, addr: IpAddr, tag: Option<String>) {
        let mut whitelist = self.whitelist.write();
        whitelist.add(addr, tag);
        cinfo!(NETFILTER, "{:?} is added to the whitelist", addr);
    }

    fn remove_from_whitelist(&self, addr: &IpAddr) {
        let mut whitelist = self.whitelist.write();
        whitelist.remove(&addr);
        cinfo!(NETFILTER, "{:?} is removed from the whitelist", addr);
    }

    fn add_to_blacklist(&self, addr: IpAddr, tag: Option<String>) {
        let mut blacklist = self.blacklist.write();
        blacklist.add(addr, tag);
        cinfo!(NETFILTER, "{:?} is added to the blacklist", addr);
    }

    fn remove_from_blacklist(&self, addr: &IpAddr) {
        let mut blacklist = self.blacklist.write();
        blacklist.remove(&addr);
        cinfo!(NETFILTER, "{:?} is removed from the blacklist", addr);
    }

    fn enable_whitelist(&self) {
        let mut whitelist = self.whitelist.write();
        whitelist.enable();
        cinfo!(NETFILTER, "The whitelist is enabled");
    }

    fn disable_whitelist(&self) {
        let mut whitelist = self.whitelist.write();
        whitelist.disable();
        cinfo!(NETFILTER, "The whitelist is disabled");
    }

    fn enable_blacklist(&self) {
        let mut blacklist = self.blacklist.write();
        blacklist.enable();
        cinfo!(NETFILTER, "The blacklist is enabled");
    }

    fn disable_blacklist(&self) {
        let mut blacklist = self.blacklist.write();
        blacklist.disable();
        cinfo!(NETFILTER, "The blacklist is disabled");
    }

    fn get_whitelist(&self) -> (Vec<FilterEntry>, bool) {
        let whitelist = self.whitelist.read();
        whitelist.status()
    }

    fn get_blacklist(&self) -> (Vec<FilterEntry>, bool) {
        let blacklist = self.blacklist.read();
        blacklist.status()
    }

    fn is_allowed(&self, addr: &IpAddr) -> bool {
        let whitelist = self.whitelist.read();
        let blacklist = self.blacklist.read();

        if whitelist.is_enabled() && !whitelist.contains(addr) {
            return false
        }

        if blacklist.is_enabled() && blacklist.contains(addr) {
            return false
        }
        true
    }
}
