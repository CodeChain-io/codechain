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

use primitives::H160;

use super::super::uint::Uint;
use super::World;

#[derive(Debug, PartialEq, Deserialize)]
pub struct Shard {
    pub nonce: Option<Uint>,
    pub owner: H160,
    pub worlds: Option<Vec<World>>,
}

#[cfg(test)]
mod tests {
    use primitives::U256;
    use serde_json;

    use super::*;

    #[test]
    fn shard_deserialization() {
        let s = r#"{
            "nonce": 0,
            "owner": "0x01234567890abcdef0123456789abcdef0123456",
            "worlds": [{
                "nonce": 3,
                "owners": ["0x01234567890abcdef0123456789abcdef0123457"]
            }]
        }"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: Some(Uint(U256::from(0))),
                owner: H160::from("01234567890abcdef0123456789abcdef0123456"),
                worlds: Some(vec![World {
                    nonce: Some(Uint(U256::from(3))),
                    owners: Some(vec![H160::from("01234567890abcdef0123456789abcdef0123457")]),
                }]),
            },
            shard
        );
    }

    #[test]
    fn shard_with_non_zero_nonce_deserialization() {
        let s = r#"{
            "nonce": 100,
            "owner": "0x01234567890abcdef0123456789abcdef0123456"
        }"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: Some(Uint(U256::from(100))),
                owner: H160::from("01234567890abcdef0123456789abcdef0123456"),
                worlds: None,
            },
            shard
        );
    }


    #[test]
    fn deserialization_of_empty_shard_must_fail() {
        let s = r#"{
        }"#;
        let result: Result<Shard, _> = serde_json::from_str(s);
        assert!(result.is_err());
    }

    #[test]
    fn shard_without_nonce_deserialization() {
        let s = r#"{
            "owner": "0x01234567890abcdef0123456789abcdef0123456"
        }"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: None,
                owner: H160::from("01234567890abcdef0123456789abcdef0123456"),
                worlds: None,
            },
            shard
        );
    }
}
