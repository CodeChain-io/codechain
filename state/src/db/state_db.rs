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

// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.
//
// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use std::collections::HashMap;
use std::sync::Arc;

use ctypes::ShardId;
use hashdb::{AsHashDB, HashDB};
use journaldb::{self, Algorithm, JournalDB};
use kvdb::DBTransaction;
use kvdb_memorydb;
use primitives::H256;
use util_error::UtilError;

use crate::cache::{GlobalCache, ShardCache, TopCache};
use crate::impls::TopLevelState;

/// State database abstraction.
pub struct StateDB {
    /// Backing database.
    db: Box<JournalDB>,
    cache: GlobalCache,
    current_hash: Option<H256>,
}

impl StateDB {
    /// Create a new instance wrapping `JournalDB`
    pub fn new(db: Box<JournalDB>) -> StateDB {
        StateDB {
            db,
            cache: Default::default(),
            current_hash: None,
        }
    }

    pub fn new_with_memorydb() -> Self {
        let memorydb = Arc::new(kvdb_memorydb::create(0));
        let db = journaldb::new(memorydb, Algorithm::Archive, None);
        Self::new(db)
    }

    /// Journal all recent operations under the given era and ID.
    pub fn journal_under(&mut self, batch: &mut DBTransaction, now: u64, id: H256) -> Result<u32, UtilError> {
        let records = self.db.journal_under(batch, now, &id)?;
        self.current_hash = Some(id);
        Ok(records)
    }

    /// Mark a given candidate from an ancient era as canonical, enacting its removals from the
    /// backing database and reverting any non-canonical historical commit's insertions.
    pub fn mark_canonical(
        &mut self,
        batch: &mut DBTransaction,
        end_era: u64,
        canon_id: &H256,
    ) -> Result<u32, UtilError> {
        self.db.mark_canonical(batch, end_era, canon_id)
    }

    /// Check if pruning is enabled on the database.
    pub fn is_pruned(&self) -> bool {
        self.db.is_pruned()
    }

    /// Check if the database is empty.
    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }

    pub fn top_cache(&self) -> TopCache {
        self.cache.top_cache()
    }

    pub fn shard_caches(&self) -> HashMap<ShardId, ShardCache> {
        self.cache.shard_caches()
    }

    pub fn override_state(&mut self, state: &TopLevelState) {
        self.cache.override_cache(state.top_cache(), state.shard_caches());
        self.current_hash = Some(state.root());
    }

    pub fn clone(&self, hash: &H256) -> Self {
        let (cache, current_hash) = if self.current_hash.as_ref() == Some(hash) {
            (self.cache.clone(), self.current_hash)
        } else {
            (Default::default(), None)
        };

        Self {
            db: self.db.boxed_clone(),
            cache,
            current_hash,
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl AsHashDB for StateDB {
    /// Conversion method to interpret self as `HashDB` reference
    fn as_hashdb(&self) -> &HashDB {
        self.db.as_hashdb()
    }

    /// Conversion method to interpret self as mutable `HashDB` reference
    fn as_hashdb_mut(&mut self) -> &mut HashDB {
        self.db.as_hashdb_mut()
    }
}
