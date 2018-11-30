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

use std::cell::RefMut;

use cmerkle::{Result as TrieResult, TrieDB, TrieMut};

use super::WriteBack;
use crate::{AssetScheme, AssetSchemeAddress, OwnedAsset, OwnedAssetAddress};

pub struct ShardCache {
    asset_scheme: WriteBack<AssetScheme>,
    asset: WriteBack<OwnedAsset>,
}

impl ShardCache {
    pub fn new(
        asset_schemes: impl Iterator<Item = (AssetSchemeAddress, AssetScheme)>,
        assets: impl Iterator<Item = (OwnedAssetAddress, OwnedAsset)>,
    ) -> Self {
        Self {
            asset_scheme: WriteBack::new_with_iter(asset_schemes),
            asset: WriteBack::new_with_iter(assets),
        }
    }

    pub fn checkpoint(&mut self) {
        self.asset_scheme.checkpoint();
        self.asset.checkpoint();
    }

    pub fn discard_checkpoint(&mut self) {
        self.asset_scheme.discard_checkpoint();
        self.asset.discard_checkpoint();
    }

    pub fn revert_to_checkpoint(&mut self) {
        self.asset_scheme.revert_to_checkpoint();
        self.asset.revert_to_checkpoint();
    }

    pub fn commit<'db>(&mut self, trie: &mut (TrieMut + 'db)) -> TrieResult<()> {
        self.asset_scheme.commit(trie)?;
        self.asset.commit(trie)?;
        Ok(())
    }

    pub fn asset_scheme(&self, a: &AssetSchemeAddress, db: TrieDB) -> TrieResult<Option<AssetScheme>> {
        self.asset_scheme.get(a, db)
    }

    pub fn asset_scheme_mut(&self, a: &AssetSchemeAddress, db: TrieDB) -> TrieResult<RefMut<AssetScheme>> {
        self.asset_scheme.get_mut(a, db)
    }

    pub fn remove_asset_scheme(&self, address: &AssetSchemeAddress) {
        self.asset_scheme.remove(address)
    }

    pub fn asset(&self, a: &OwnedAssetAddress, db: TrieDB) -> TrieResult<Option<OwnedAsset>> {
        self.asset.get(a, db)
    }

    pub fn asset_mut(&self, a: &OwnedAssetAddress, db: TrieDB) -> TrieResult<RefMut<OwnedAsset>> {
        self.asset.get_mut(a, db)
    }

    pub fn remove_asset(&self, address: &OwnedAssetAddress) {
        self.asset.remove(address)
    }

    pub fn cached_assets(&self) -> Vec<(usize, OwnedAssetAddress, Option<OwnedAsset>)> {
        self.asset.items()
    }

    pub fn cached_asset_schemes(&self) -> Vec<(usize, AssetSchemeAddress, Option<AssetScheme>)> {
        self.asset_scheme.items()
    }
}

impl Clone for ShardCache {
    fn clone(&self) -> Self {
        Self {
            asset_scheme: self.asset_scheme.clone(),
            asset: self.asset.clone(),
        }
    }
}

impl Default for ShardCache {
    fn default() -> Self {
        Self::new(::std::iter::empty(), ::std::iter::empty())
    }
}
