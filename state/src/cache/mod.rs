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

use std::fmt;
use std::hash::Hash;

use rlp::{Decodable, Encodable};

mod global_cache;
mod lru_cache;
mod shard_cache;
mod top_cache;
mod write_back;

pub use self::global_cache::GlobalCache;
pub use self::shard_cache::ShardCache;
pub use self::top_cache::TopCache;
pub use self::write_back::WriteBack;

pub trait CacheableItem: Clone + Default + fmt::Debug + Decodable + Encodable {
    type Address: AsRef<[u8]> + Clone + Copy + fmt::Debug + Eq + Hash;
    fn is_null(&self) -> bool;
}
