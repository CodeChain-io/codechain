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

use std::ops::Deref;
use std::sync::Arc;
use std::vec::Vec;

use ccore::{Client, COL_STATE};
use ctypes::H256;
use jsonrpc_core::Result;
use kvdb::KeyValueDB;
use rlp::UntrustedRlp;

use super::super::errors;
use super::super::traits::Devel;
use super::super::types::Bytes;

pub struct DevelClient {
    db: Arc<KeyValueDB>,
}

impl DevelClient {
    pub fn new(client: &Client) -> Self {
        Self {
            db: client.database(),
        }
    }
}

impl Devel for DevelClient {
    fn get_state_trie_keys(&self, offset: usize, limit: usize) -> Result<Vec<H256>> {
        let iter = self.db.iter(COL_STATE);
        Ok(iter.skip(offset).take(limit).map(|val| H256::from(val.0.deref())).collect())
    }

    fn get_state_trie_value(&self, key: H256) -> Result<Vec<Bytes>> {
        match self.db.get(COL_STATE, &key) {
            Ok(Some(value)) => {
                let rlp = UntrustedRlp::new(&value);
                Ok(rlp.as_list::<Vec<u8>>().map_err(errors::rlp)?.into_iter().map(Bytes::from).collect())
            }
            Ok(None) => Ok(Vec::new()),
            Err(err) => Err(errors::kvdb(err)),
        }
    }
}
