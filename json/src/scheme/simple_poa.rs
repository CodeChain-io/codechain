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

use ckey::PlatformAddress;

use super::super::uint::Uint;

/// Authority params deserialization.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimplePoAParams {
    /// Valid authorities
    pub validators: Vec<PlatformAddress>,
    /// Block reward.
    pub block_reward: Option<Uint>,
}

/// Authority engine deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct SimplePoA {
    pub params: SimplePoAParams,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ckey::PlatformAddress;
    use primitives::U256;
    use serde_json;

    use super::super::super::uint::Uint;
    use super::SimplePoA;

    #[test]
    fn basic_authority_deserialization() {
        let s = r#"{
            "params": {
                "validators" : ["tccq8qlwpt7xcs9lec3c8tyt3kqxlgsus8q4qp3m6ft"],
                "blockReward": "0x0d"
            }
        }"#;

        let deserialized: SimplePoA = serde_json::from_str(s).unwrap();

        let vs = vec![PlatformAddress::from_str("tccq8qlwpt7xcs9lec3c8tyt3kqxlgsus8q4qp3m6ft").unwrap()];
        assert_eq!(deserialized.params.validators, vs);
        assert_eq!(deserialized.params.block_reward, Some(Uint(U256::from(0x0d))));
    }
}
