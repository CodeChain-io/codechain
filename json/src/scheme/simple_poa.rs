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

use ckey::Public;

use crate::uint::Uint;

/// Authority params deserialization.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimplePoAParams {
    /// Valid authorities
    pub validators: Vec<Public>,
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

    use ckey::Public;
    use primitives::U256;
    use serde_json;

    use super::SimplePoA;
    use crate::uint::Uint;

    #[test]
    fn basic_authority_deserialization() {
        let s = r#"{
            "params": {
                "validators" : ["0x2a8a69439f2396c9a328289fdc3905d9736da9e14eb1a282cfd2c036cc21a17a5d05595160b7924e5ecf3f2628b440e601f3a531e92fa81571a70e6c695b2d08"],
                "blockReward": "0x0d"
            }
        }"#;

        let deserialized: SimplePoA = serde_json::from_str(s).unwrap();

        let vs = vec![Public::from_str("2a8a69439f2396c9a328289fdc3905d9736da9e14eb1a282cfd2c036cc21a17a5d05595160b7924e5ecf3f2628b440e601f3a531e92fa81571a70e6c695b2d08").unwrap()];
        assert_eq!(deserialized.params.validators, vs);
        assert_eq!(deserialized.params.block_reward, Some(Uint(U256::from(0x0d))));
    }
}
