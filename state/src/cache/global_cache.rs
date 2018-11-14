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

use std::collections::{HashMap, HashSet};

use super::super::{Account, ActionData, AssetScheme, Metadata, OwnedAsset, RegularAccount, Shard};
use super::lru_cache::LruCache;
use super::{ShardCache, TopCache};

use ctypes::ShardId;

pub struct GlobalCache {
    account: LruCache<Account>,
    regular_account: LruCache<RegularAccount>,
    metadata: LruCache<Metadata>,
    shard: LruCache<Shard>,
    action_data: LruCache<ActionData>,

    asset_scheme: LruCache<AssetScheme>,
    asset: LruCache<OwnedAsset>,
}

impl GlobalCache {
    pub fn new(
        account: usize,
        regular_account: usize,
        shard: usize,
        action_data: usize,
        asset_scheme: usize,
        asset: usize,
    ) -> Self {
        Self {
            account: LruCache::new(account),
            regular_account: LruCache::new(regular_account),
            metadata: LruCache::new(1),
            shard: LruCache::new(shard),
            action_data: LruCache::new(action_data),

            asset_scheme: LruCache::new(asset_scheme),
            asset: LruCache::new(asset),
        }
    }

    pub fn top_cache(&self) -> TopCache {
        TopCache::new(
            self.account.iter().map(|(addr, item)| (addr.clone(), item.clone())),
            self.regular_account.iter().map(|(addr, item)| (addr.clone(), item.clone())),
            self.metadata.iter().map(|(addr, item)| (addr.clone(), item.clone())),
            self.shard.iter().map(|(addr, item)| (addr.clone(), item.clone())),
            self.action_data.iter().map(|(addr, item)| (addr.clone(), item.clone())),
        )
    }

    fn shard_cache(&self, shard_id: ShardId) -> ShardCache {
        ShardCache::new(
            self.asset_scheme
                .iter()
                .filter(|(addr, _)| addr.shard_id() == shard_id)
                .map(|(addr, item)| (addr.clone(), item.clone())),
            self.asset
                .iter()
                .filter(|(addr, _)| addr.shard_id() == shard_id)
                .map(|(addr, item)| (addr.clone(), item.clone())),
        )
    }

    fn shard_ids(&self) -> HashSet<ShardId> {
        self.asset_scheme
            .iter()
            .map(|(addr, _)| addr.shard_id())
            .chain(self.asset.iter().map(|(addr, _)| addr.shard_id()))
            .collect()
    }

    pub fn shard_caches(&self) -> HashMap<ShardId, ShardCache> {
        self.shard_ids().into_iter().map(|shard_id| (shard_id, self.shard_cache(shard_id))).collect()
    }

    pub fn override_cache(&mut self, top_cache: &TopCache, shard_caches: &HashMap<ShardId, ShardCache>) {
        self.clear();

        for (addr, item) in top_cache.cached_accounts().into_iter() {
            match item {
                Some(item) => self.account.insert(addr, item),
                None => self.account.remove(&addr),
            };
        }
        for (addr, item) in top_cache.cached_regular_accounts().into_iter() {
            match item {
                Some(item) => self.regular_account.insert(addr, item),
                None => self.regular_account.remove(&addr),
            };
        }
        for (addr, item) in top_cache.cached_metadata().into_iter() {
            match item {
                Some(item) => self.metadata.insert(addr, item),
                None => self.metadata.remove(&addr),
            };
        }
        for (addr, item) in top_cache.cached_shards().into_iter() {
            match item {
                Some(item) => self.shard.insert(addr, item),
                None => self.shard.remove(&addr),
            };
        }
        for (addr, item) in top_cache.cached_action_data().into_iter() {
            match item {
                Some(item) => self.action_data.insert(addr, item),
                None => self.action_data.remove(&addr),
            };
        }
        for (addr, item) in shard_caches.iter().flat_map(|(_, shard_cache)| shard_cache.cached_assets().into_iter()) {
            match item {
                Some(item) => self.asset.insert(addr, item),
                None => self.asset.remove(&addr),
            };
        }
        for (addr, item) in
            shard_caches.iter().flat_map(|(_, shard_cache)| shard_cache.cached_asset_schemes().into_iter())
        {
            match item {
                Some(item) => self.asset_scheme.insert(addr, item),
                None => self.asset_scheme.remove(&addr),
            };
        }
    }

    fn clear(&mut self) {
        self.account.clear();
        self.regular_account.clear();
        self.metadata.clear();
        self.shard.clear();
        self.action_data.clear();
        self.asset_scheme.clear();
        self.asset.clear();
    }
}

impl Default for GlobalCache {
    fn default() -> Self {
        // FIXME: Set the right number
        const N_ACCOUNT: usize = 100;
        const N_REGULAR_ACCOUNT: usize = 100;
        const N_SHARD: usize = 100;
        const N_ACTION_DATA: usize = 10;
        const N_ASSET_SCHEME: usize = 100;
        const N_ASSET: usize = 1000;
        Self::new(N_ACCOUNT, N_REGULAR_ACCOUNT, N_SHARD, N_ACTION_DATA, N_ASSET_SCHEME, N_ASSET)
    }
}

impl Clone for GlobalCache {
    fn clone(&self) -> Self {
        Self {
            account: self.account.clone(),
            regular_account: self.regular_account.clone(),
            metadata: self.metadata.clone(),
            shard: self.shard.clone(),
            action_data: self.action_data.clone(),

            asset_scheme: self.asset_scheme.clone(),
            asset: self.asset.clone(),
        }
    }
}
