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

use std::sync::Arc;
use parking_lot::Mutex;

use ccore::BlockChainClient;
use cnetwork::{Api, Error, Extension, NodeId};

use manager::DownloadManager;

const EXTENSION_NAME: &'static str = "block-propagation";
const SYNC_TIMER_ID: usize = 0;
const SYNC_TIMER_INTERVAL: u64 = 1000;

pub struct BlockSyncExtension {
    client: Arc<BlockChainClient>,
    manager: Mutex<DownloadManager>,
    api: Mutex<Option<Arc<Api>>>,
}

impl BlockSyncExtension {
    pub fn new(client: Arc<BlockChainClient>) -> Arc<Self> {
        Arc::new(Self {
            client,
            manager: Mutex::new(DownloadManager::new()),
            api: Mutex::new(None),
        })
    }
}

impl Extension for BlockSyncExtension {
    fn name(&self) -> String { String::from(EXTENSION_NAME) }
    fn need_encryption(&self) -> bool { false }

    fn on_initialize(&self, api: Arc<Api>) {
        api.set_timer(SYNC_TIMER_ID, SYNC_TIMER_INTERVAL);
        *self.api.lock() = Some(api);
    }

    fn on_node_added(&self, id: &NodeId) {
        self.api.lock().as_ref().map(|api| api.connect(id));
    }
    fn on_node_removed(&self, _id: &NodeId) { unimplemented!() }

    fn on_connected(&self, _id: &NodeId) { unimplemented!() }
    fn on_connection_allowed(&self, id: &NodeId) { self.on_connected(id); }
    fn on_connection_denied(&self, _id: &NodeId, _error: Error) {}

    fn on_message(&self, _id: &NodeId, _message: &Vec<u8>) { unimplemented!() }

    fn on_close(&self) { *self.api.lock() = None }

    fn on_timer_set_allowed(&self, _timer_id: usize) {}
    fn on_timer_set_denied(&self, _timer_id: usize, _error: Error) { debug_assert!(false) }

    fn on_timeout(&self, timer_id: usize) {
        debug_assert_eq!(timer_id, SYNC_TIMER_ID);
    }
}


