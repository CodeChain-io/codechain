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
pub struct CuckooParams {
    /// Block reward.
    #[serde(rename = "blockReward")]
    pub block_reward: Option<Uint>,
    #[serde(rename = "minScore")]
    pub min_score: Option<Uint>,
    #[serde(rename = "maxVertex")]
    pub max_vertex: Option<Uint>,
    #[serde(rename = "maxEdge")]
    pub max_edge: Option<Uint>,
    #[serde(rename = "cycleLength")]
    pub cycle_length: Option<Uint>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Cuckoo {
    pub params: CuckooParams,
}

#[cfg(test)]
mod tests {
    use ctypes::U256;
    use serde_json;

    use super::super::super::uint::Uint;
    use super::*;

    #[test]
    fn cuckoo_deserialization() {
        let s = r#"{
			"params": {
				"blockReward": "0x0d",
                "minScore" : "0x020000",
                "maxVertex" : "16",
                "maxEdge" : "8",
                "cycleLength" : "6"
			}
		}"#;

        let deserialized: Cuckoo = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.params.block_reward, Some(Uint(U256::from(0x0d))));
        assert_eq!(deserialized.params.min_score, Some(Uint(U256::from(0x020000))));
        assert_eq!(deserialized.params.max_vertex, Some(Uint(U256::from(16))));
        assert_eq!(deserialized.params.max_edge, Some(Uint(U256::from(8))));
        assert_eq!(deserialized.params.cycle_length, Some(Uint(U256::from(6))));
    }
}
