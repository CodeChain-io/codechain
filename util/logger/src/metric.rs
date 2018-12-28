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

use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use parking_lot::RwLock;
use time;

pub type MetricKey = &'static str;

pub struct MetricEntry {
    pub value: u64,
    pub prev: u64,
}

impl MetricEntry {
    fn new_with(initial: u64) -> Self {
        MetricEntry {
            value: initial,
            prev: 0,
        }
    }

    fn changed(&self) -> u64 {
        self.value - self.prev
    }

    fn inc(&mut self) {
        self.value += 1;
    }

    fn reset_prev(&mut self) {
        self.prev = self.value;
    }
}

pub struct Metric {
    inner: Arc<RwLock<MetricInner>>,
}

pub struct MetricInner {
    table: BTreeMap<MetricKey, MetricEntry>,
    prev_print_time: Instant,
}

impl Default for Metric {
    fn default() -> Self {
        Self::new()
    }
}

impl Metric {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricInner {
                table: BTreeMap::new(),
                prev_print_time: Instant::now(),
            })),
        }
    }

    pub fn start_thread(&self) {
        let inner = Arc::clone(&self.inner);
        thread::Builder::new()
            .name("Metric logger".to_string())
            .spawn(move || loop {
                thread::sleep(::std::time::Duration::new(1, 0));
                inner.write().print();
            })
            .unwrap();
    }

    pub fn increase(&self, key: &'static str) {
        let mut guard = self.inner.write();
        guard.increase(key);
    }

    pub fn print(&self) {
        let mut guard = self.inner.write();
        guard.print();
    }
}

impl MetricInner {
    pub fn increase(&mut self, key: &'static str) {
        self.table.entry(key).or_insert_with(|| MetricEntry::new_with(1)).inc();
    }

    pub fn print(&mut self) {
        let timestamp = time::strftime("%Y-%m-%d %H:%M:%S %Z", &time::now()).unwrap();
        println!("Metric at : {}", timestamp);
        let elapsed_secs = (Instant::now() - self.prev_print_time).as_secs() as f64;
        for (k, v) in self.table.iter_mut() {
            println!(
                "Metric {}: cur {} changed {} speed {}",
                k,
                v.value,
                v.changed(),
                (v.changed() as f64) / elapsed_secs
            );
            v.reset_prev();
        }

        self.prev_print_time = Instant::now();
    }
}
