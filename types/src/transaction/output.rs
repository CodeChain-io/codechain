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

use primitives::{Bytes, H160};

use crate::ShardId;

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable)]
pub struct AssetTransferOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub asset_type: H160,
    pub shard_id: ShardId,
    pub quantity: u64,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct AssetMintOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub supply: u64,
}
