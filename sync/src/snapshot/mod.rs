// Copyright 2019 Kodebox, Inc.
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

use std::fs::{create_dir_all, File};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{spawn, JoinHandle};

use ccore::snapshot_notify::{NotifyReceiverSource, ReceiverCanceller};
use ccore::{BlockChainTrait, BlockId, Client};
use cmerkle::snapshot::{ChunkCompressor, Error as SnapshotError, Snapshot};
use ctypes::BlockHash;
use hashdb::{AsHashDB, HashDB};
use primitives::H256;
use std::ops::Deref;

pub struct Service {
    join_handle: Option<JoinHandle<()>>,
    canceller: Option<ReceiverCanceller>,
}

pub fn snapshot_dir(root_dir: &str, block: &BlockHash) -> PathBuf {
    let mut path = PathBuf::new();
    path.push(root_dir);
    path.push(format!("{:x}", block.deref()));
    path
}

pub fn snapshot_path(root_dir: &str, block: &BlockHash, chunk_root: &H256) -> PathBuf {
    let mut path = snapshot_dir(root_dir, block);
    path.push(format!("{:x}", chunk_root));
    path
}

impl Service {
    pub fn new(client: Arc<Client>, notify_receiver_source: NotifyReceiverSource, root_dir: String) -> Self {
        let NotifyReceiverSource(canceller, receiver) = notify_receiver_source;
        let join_handle = spawn(move || {
            cinfo!(SYNC, "Snapshot service is on");
            while let Ok(block_hash) = receiver.recv() {
                cinfo!(SYNC, "Snapshot is requested for block: {}", block_hash);
                let state_root = if let Some(header) = client.block_header(&BlockId::Hash(block_hash)) {
                    header.state_root()
                } else {
                    cerror!(SYNC, "There isn't corresponding header for the requested block hash: {}", block_hash,);
                    continue
                };
                let db_lock = client.state_db().read();
                if let Some(err) = snapshot(db_lock.as_hashdb(), block_hash, state_root, &root_dir).err() {
                    cerror!(
                        SYNC,
                        "Snapshot request failed for block: {}, chunk_root: {}, err: {}",
                        block_hash,
                        state_root,
                        err
                    );
                } else {
                    cinfo!(SYNC, "Snapshot is ready for block: {}", block_hash)
                }
                // TODO: Prune old snapshots
            }
            cinfo!(SYNC, "Snapshot service is stopped")
        });

        Self {
            canceller: Some(canceller),
            join_handle: Some(join_handle),
        }
    }
}

fn snapshot(db: &dyn HashDB, block_hash: BlockHash, chunk_root: H256, root_dir: &str) -> Result<(), SnapshotError> {
    let snapshot_dir = snapshot_dir(root_dir, &block_hash);
    create_dir_all(snapshot_dir)?;

    for chunk in Snapshot::from_hashdb(db, chunk_root) {
        let chunk_path = snapshot_path(root_dir, &block_hash, &chunk.root);
        let chunk_file = File::create(chunk_path)?;
        let compressor = ChunkCompressor::new(chunk_file);
        compressor.compress_chunk(&chunk)?;
    }

    Ok(())
}

impl Drop for Service {
    fn drop(&mut self) {
        if let Some(canceller) = self.canceller.take() {
            // The thread corresponding to the `self.join_handle` waits for the `self.canceller` is dropped.
            // It must be dropped first not to make deadlock at `handle.join()`.
            drop(canceller);
        }

        if let Some(handle) = self.join_handle.take() {
            handle.join().expect("Snapshot service thread shouldn't panic");
        }
    }
}
