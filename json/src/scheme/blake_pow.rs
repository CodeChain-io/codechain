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

use super::super::uint::Uint;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlakePoWParams {
    /// Block reward.
    pub block_reward: Option<Uint>,
    pub min_score: Option<Uint>,
    pub block_interval: Option<Uint>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct BlakePoW {
    pub params: BlakePoWParams,
}

#[cfg(test)]
mod tests {
    use primitives::U256;
    use serde_json;

    use super::super::super::uint::Uint;
    use super::*;

    #[test]
    fn blake_pow_deserialization() {
        let s = r#"{
            "params": {
                "blockReward": "0x0d",
                "minScore" : "0x020000",
                "blockInterval" : "120"
            }
        }"#;

        let deserialized: BlakePoW = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.params.block_reward, Some(Uint(U256::from(0x0d))));
        assert_eq!(deserialized.params.min_score, Some(Uint(U256::from(0x020000))));
        assert_eq!(deserialized.params.block_interval, Some(Uint(U256::from(120))));
    }
}
