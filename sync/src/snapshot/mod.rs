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


use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::thread::{spawn, JoinHandle};

use ccore::snapshot_notify::{NotifyReceiverSource, ReceiverCanceller};
use ccore::{BlockChainClient, BlockChainTrait, BlockId, Client};
use cmerkle::snapshot::{ChunkCompressor, Error as SnapshotError, Snapshot};
use cstate::{StateDB, TopLevelState, TopStateView};
use ctypes::BlockHash;
use hashdb::{AsHashDB, HashDB};
use primitives::H256;

pub struct Service {
    join_handle: Option<JoinHandle<()>>,
    canceller: Option<ReceiverCanceller>,
}

pub fn snapshot_dir(root_dir: &str, block: &BlockHash) -> PathBuf {
    let mut path = PathBuf::new();
    path.push(root_dir);
    path.push(format!("{:x}", **block));
    path
}

pub fn snapshot_path(root_dir: &str, block: &BlockHash, chunk_root: &H256) -> PathBuf {
    let mut path = snapshot_dir(root_dir, block);
    path.push(format!("{:x}", chunk_root));
    path
}

impl Service {
    pub fn new(
        client: Arc<Client>,
        notify_receiver_source: NotifyReceiverSource,
        root_dir: String,
        expiration: Option<u64>,
    ) -> Self {
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
                {
                    let db_lock = client.state_db().read();
                    if let Err(err) = snapshot(&db_lock, block_hash, state_root, &root_dir) {
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
                }

                if let Some(expiration) = expiration {
                    if let Err(err) = cleanup_expired(&client, &root_dir, expiration) {
                        cerror!(SYNC, "Snapshot cleanup error after block hash {}, err: {}", block_hash, err);
                    }
                }
            }
            cinfo!(SYNC, "Snapshot service is stopped")
        });

        Self {
            canceller: Some(canceller),
            join_handle: Some(join_handle),
        }
    }
}
fn snapshot(db: &StateDB, block_hash: BlockHash, root: H256, dir: &str) -> Result<(), SnapshotError> {
    snapshot_trie(db.as_hashdb(), block_hash, root, dir)?;

    let top_state = TopLevelState::from_existing(db.clone(&root), root)?;
    let shard_roots = {
        let metadata = top_state.metadata()?.expect("Metadata must exist for snapshot block");
        let shard_num = *metadata.number_of_shards();
        (0..shard_num).map(|n| top_state.shard_root(n))
    };
    for sr in shard_roots {
        snapshot_trie(db.as_hashdb(), block_hash, sr?.expect("Shard root must exist"), dir)?;
    }
    Ok(())
}

fn snapshot_trie(db: &dyn HashDB, block_hash: BlockHash, root: H256, root_dir: &str) -> Result<(), SnapshotError> {
    let snapshot_dir = snapshot_dir(root_dir, &block_hash);
    fs::create_dir_all(snapshot_dir)?;

    for chunk in Snapshot::from_hashdb(db, root) {
        let chunk_path = snapshot_path(root_dir, &block_hash, &chunk.root);
        let chunk_file = fs::File::create(chunk_path)?;
        let compressor = ChunkCompressor::new(chunk_file);
        compressor.compress_chunk(&chunk)?;
    }

    Ok(())
}

fn cleanup_expired(client: &Client, root_dir: &str, expiration: u64) -> Result<(), SnapshotError> {
    for entry in fs::read_dir(root_dir)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                cerror!(SYNC, "Snapshot cleanup can't retrieve entry. err: {}", err);
                continue
            }
        };
        let path = entry.path();

        match entry.file_type().map(|x| x.is_dir()) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(err) => {
                cerror!(SYNC, "Snapshot cleanup can't retrieve file info: {}, err: {}", path.to_string_lossy(), err);
                continue
            }
        }

        let name = match path.file_name().expect("Directories always have file name").to_str() {
            Some(n) => n,
            None => continue,
        };
        let hash = match H256::from_str(name) {
            Ok(h) => BlockHash::from(h),
            Err(_) => continue,
        };
        let number = if let Some(number) = client.block_number(&BlockId::Hash(hash)) {
            number
        } else {
            cerror!(SYNC, "Snapshot cleanup can't retrieve block number for block_hash: {}", hash);
            continue
        };

        if number + expiration < client.best_block_header().number() {
            cleanup_snapshot(root_dir, hash)
        }
    }
    Ok(())
}

/// Remove all files in `root_dir/block_hash`
fn cleanup_snapshot(root_dir: &str, block_hash: BlockHash) {
    let path = snapshot_dir(root_dir, &block_hash);
    let rename_to = PathBuf::from(root_dir).join(format!("{:x}.old", *block_hash));
    // It is okay to ignore errors. We just wanted them to be removed.
    match fs::rename(path, &rename_to) {
        Ok(()) => {}
        Err(err) => {
            cerror!(SYNC, "Snapshot cleanup: renaming {} failed, reason: {}", block_hash, err);
        }
    }
    // Ignore the error. Cleanup failure is not a critical error.
    match fs::remove_dir_all(rename_to) {
        Ok(()) => {}
        Err(err) => {
            cerror!(SYNC, "Snapshot cleanup: removing {} failed, reason: {}", block_hash, err);
        }
    }
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
