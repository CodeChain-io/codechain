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

#[macro_use]
mod address;

mod account;
mod asset;
mod asset_scheme;
mod backend;
mod cache;
mod info;
mod metadata;
mod shard;
mod shard_level;
mod shard_metadata;
mod shard_state;
mod top_level;
mod top_state;
mod traits;

pub use self::account::Account;
pub use self::asset::{Asset, AssetAddress};
pub use self::asset_scheme::{AssetScheme, AssetSchemeAddress};
pub use self::backend::{Backend, Basic as BasicBackend, ShardBackend, TopBackend};
pub use self::cache::CacheableItem;
pub use self::info::{ShardStateInfo, TopStateInfo};
pub use self::metadata::{Metadata, MetadataAddress};
pub use self::shard::{Shard, ShardAddress};
pub use self::shard_metadata::{ShardMetadata, ShardMetadataAddress};
pub use self::shard_state::{ShardState, TransactionOutcome};
pub use self::top_level::{ParcelOutcome, TopLevelState};
pub use self::top_state::TopState;
pub use self::traits::StateWithCache;
