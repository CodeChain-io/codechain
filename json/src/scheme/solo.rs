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

use std::collections::HashMap;

use crate::uint::Uint;

/// Solo params deserialization.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoloParams {
    /// Block reward.
    pub block_reward: Option<Uint>,
    #[serde(flatten)]
    pub action_handlers: SoloActionHandlersParams,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoloActionHandlersParams {
    pub hit: Option<HashMap<(), ()>>,
}

/// Solo engine deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Solo {
    pub params: SoloParams,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use primitives::U256;
    use serde_json;

    use super::Solo;
    use crate::uint::Uint;

    #[test]
    fn basic_authority_deserialization() {
        let s = r#"{
            "params": {
                "blockReward": "0x0d",
                "hit": {}
            }
        }"#;

        let deserialized: Solo = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.params.block_reward, Some(Uint(U256::from(0x0d))));
        assert_eq!(deserialized.params.action_handlers.hit, Some(HashMap::new()));
    }
}
