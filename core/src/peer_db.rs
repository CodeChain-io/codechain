// Copyright 2020 Kodebox, Inc.
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
use crate::db::COL_EXTRA;
use cnetwork::{ManagingPeerdb, SocketAddr};
use kvdb::{DBTransaction, KeyValueDB};
use parking_lot::Mutex;
use rlp::RlpStream;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct PeerDb {
    db: Arc<dyn KeyValueDB>,
    peers_and_count: Mutex<(HashMap<SocketAddr, u64>, usize)>,
}

impl PeerDb {
    pub fn new(database: Arc<dyn KeyValueDB>) -> Arc<Self> {
        Arc::new(Self {
            db: database,
            peers_and_count: Default::default(),
        })
    }
}

impl ManagingPeerdb for PeerDb {
    fn insert(&self, key: SocketAddr) {
        let (peers, count) = &mut *self.peers_and_count.lock();
        peers.entry(key).or_insert_with(|| {
            *count += 1;
            SystemTime::now().duration_since(UNIX_EPOCH).expect("There is no time machine.").as_secs()
        });

        if let Some(batch) = get_db_transaction_if_enough_hit(peers, count) {
            self.db.write(batch).expect("The DB must alive");
        }
    }

    fn delete(&self, key: &SocketAddr) {
        let (peers, count) = &mut *self.peers_and_count.lock();
        if peers.remove(key).is_some() {
            *count += 1;
        }

        if let Some(batch) = get_db_transaction_if_enough_hit(peers, count) {
            self.db.write(batch).expect("The DB must alive");
        }
    }
}

// XXX: It may not be needed. Generally, in the p2p networks, the old node lives longer.
// Exaggeratedly, the new node does not affect the stability of the network. In other words, it
// doesn't matter even we don't restore the fresh updated nodes.
impl Drop for PeerDb {
    fn drop(&mut self) {
        let (peers, _) = &*self.peers_and_count.lock();
        let batch = get_db_transaction(peers);
        self.db.write(batch).expect("The DB must alive");
    }
}

fn get_db_transaction_if_enough_hit(peers: &HashMap<SocketAddr, u64>, count: &mut usize) -> Option<DBTransaction> {
    const UPDATE_AT: usize = 10;
    if *count < UPDATE_AT {
        return None
    }

    *count = 0;

    Some(get_db_transaction(peers))
}

fn get_db_transaction(peers: &HashMap<SocketAddr, u64>) -> DBTransaction {
    let mut s = RlpStream::new_list(peers.len());
    for (address, time) in peers {
        s.begin_list(2).append(address).append(time);
    }
    let encoded = s.drain();

    let mut batch = DBTransaction::new();

    const COLUMN_TO_WRITE: Option<u32> = COL_EXTRA;
    const PEER_DB_KEY: &[u8] = b"peer-list";
    batch.put(COLUMN_TO_WRITE, PEER_DB_KEY, &encoded);
    batch
}
