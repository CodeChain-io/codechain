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

use error::Error;

pub type CheckpointId = usize;

pub trait StateWithCheckpoint {
    /// Create a recoverable checkpoint of this state.
    fn create_checkpoint(&mut self, id: CheckpointId);
    /// Merge last checkpoint with previous.
    fn discard_checkpoint(&mut self, id: CheckpointId);
    /// Revert to the last checkpoint and discard it.
    fn revert_to_checkpoint(&mut self, id: CheckpointId);
}

pub trait StateWithCache {
    /// Commits our cached account changes into the trie.
    fn commit(&mut self) -> Result<(), Error>;

    /// Propagate local cache into shared canonical state cache.
    fn propagate_to_global_cache(&mut self);

    /// Clear state cache
    fn clear(&mut self);
}
