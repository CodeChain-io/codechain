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

use primitives::{H160, H256};

use crate::ShardId;

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable)]
pub struct AssetOutPoint {
    pub tracker: H256,
    pub index: usize,
    pub asset_type: H160,
    pub shard_id: ShardId,
    pub quantity: u64,
}

impl AssetOutPoint {
    pub fn related_shard(&self) -> ShardId {
        self.shard_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn related_shard_of_asset_out_point() {
        let mut asset_type = H256::new();
        asset_type[2..4].copy_from_slice(&[0xBE, 0xEF]);

        let p = AssetOutPoint {
            tracker: H256::random(),
            index: 3,
            asset_type,
            quantity: 34,
        };

        assert_eq!(0xBEEF, p.related_shard());
    }
}
