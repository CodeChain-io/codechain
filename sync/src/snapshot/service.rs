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
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{spawn, JoinHandle};

use ccore::{BlockChainClient, BlockChainTrait, BlockId, ChainNotify, Client, DatabaseClient};
use ctypes::BlockHash;
use parking_lot::Mutex;

use super::error::Error;
use super::snapshot::{Snapshot, WriteSnapshot};

pub struct Service {
    client: Arc<Client>,
    /// Snapshot root directory
    root_dir: String,
    /// Snapshot creation period in unit of block numbers
    period: u64,
    thread_ids: AtomicUsize,
    joins: Arc<Mutex<HashMap<usize, JoinHandle<()>>>>,
}

impl Service {
    pub fn new(client: Arc<Client>, root_dir: String, period: u64) -> Arc<Self> {
        Arc::new(Self {
            client,
            root_dir,
            period,
            thread_ids: AtomicUsize::new(0),
            joins: Default::default(),
        })
    }
}

impl ChainNotify for Service {
    /// fires when chain has new blocks.
    fn new_blocks(
        &self,
        _imported: Vec<BlockHash>,
        _invalid: Vec<BlockHash>,
        enacted: Vec<BlockHash>,
        _retracted: Vec<BlockHash>,
        _sealed: Vec<BlockHash>,
    ) {
        let best_number = self.client.chain_info().best_block_number;
        let is_checkpoint = enacted
            .iter()
            .map(|hash| self.client.block_number(&BlockId::Hash(*hash)).expect("Enacted block must exist"))
            .any(|number| number % self.period == 0);
        if is_checkpoint && best_number > self.period {
            let number = (best_number / self.period - 1) * self.period;
            let header = self.client.block_header(&BlockId::Number(number)).expect("Snapshot target must exist");

            let db = self.client.database();
            let path: PathBuf = [self.root_dir.clone(), format!("{:x}", *header.hash())].iter().collect();
            let root = header.state_root();
            // FIXME: The db can be corrupted because the CodeChain doesn't wait child threads end on exit.
            let id = self.thread_ids.fetch_add(1, Ordering::SeqCst);
            let joins = Arc::clone(&self.joins);
            let join = spawn(move || {
                match Snapshot::try_new(path).map(|s| s.write_snapshot(db.as_ref(), &root)) {
                    Ok(_) => {}
                    Err(Error::FileError(ErrorKind::AlreadyExists)) => {}
                    Err(e) => cerror!(SNAPSHOT, "{}", e),
                }
                joins.lock().remove(&id);
            });
            self.joins.lock().insert(id, join);
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let mut joins = self.joins.lock();
        for (_, join) in joins.drain() {
            join.join().unwrap();
        }
    }
}
