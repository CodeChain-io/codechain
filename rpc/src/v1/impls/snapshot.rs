// Copyright 2018-2019 Kodebox, Inc.
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
use std::str::FromStr;
use std::sync::Arc;

use ccore::{BlockChainClient, BlockId};
use ctypes::BlockHash;
use primitives::H256;

use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Snapshot;
use super::super::types::BlockNumberAndHash;

pub struct SnapshotClient<C>
where
    C: BlockChainClient, {
    client: Arc<C>,
    snapshot_path: Option<String>,
}

impl<C> SnapshotClient<C>
where
    C: BlockChainClient,
{
    pub fn new(client: Arc<C>, snapshot_path: Option<String>) -> Self {
        SnapshotClient {
            client,
            snapshot_path,
        }
    }
}

impl<C> Snapshot for SnapshotClient<C>
where
    C: BlockChainClient + 'static,
{
    fn get_snapshot_list(&self) -> Result<Vec<BlockNumberAndHash>> {
        if let Some(snapshot_path) = &self.snapshot_path {
            let mut result = Vec::new();
            for entry in fs::read_dir(snapshot_path).map_err(errors::io)? {
                let entry = entry.map_err(errors::io)?;

                // Check if the entry is a directory
                let file_type = entry.file_type().map_err(errors::io)?;
                if !file_type.is_dir() {
                    continue
                }

                let path = entry.path();
                let name = match path.file_name().expect("Directories always have file name").to_str() {
                    Some(n) => n,
                    None => continue,
                };
                let hash = match H256::from_str(name) {
                    Ok(h) => BlockHash::from(h),
                    Err(_) => continue,
                };
                if let Some(number) = self.client.block_number(&BlockId::Hash(hash)) {
                    result.push(BlockNumberAndHash {
                        number,
                        hash,
                    });
                }
            }
            result.sort_unstable_by(|a, b| b.number.cmp(&a.number));
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }
}
