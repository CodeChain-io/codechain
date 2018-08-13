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
use super::World;

#[derive(Debug, PartialEq, Deserialize)]
pub struct Shard {
    pub nonce: Option<Uint>,
    pub owners: Vec<PlatformAddress>,
    pub users: Option<Vec<PlatformAddress>>,
    pub worlds: Option<Vec<World>>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ckey::PlatformAddress;
    use primitives::U256;
    use serde_json;

    use super::*;

    #[test]
    fn shard_deserialization() {
        let s = r#"{
            "nonce": 0,
            "owners": ["tccqqtk3q3rea46cq4cpa4h5tm43nw3supd6uxtltxv"],
            "worlds": [{
                "nonce": 3,
                "owners": ["tccqp9lfw377aaxwl2f9s34h5lpfru0y5tlrc5avutn"]
            }]
        }"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: Some(Uint(U256::from(0))),
                owners: vec![PlatformAddress::from_str("tccqqtk3q3rea46cq4cpa4h5tm43nw3supd6uxtltxv").unwrap()],
                users: None,
                worlds: Some(vec![World {
                    nonce: Some(Uint(U256::from(3))),
                    owners: Some(vec![
                        PlatformAddress::from_str("tccqp9lfw377aaxwl2f9s34h5lpfru0y5tlrc5avutn").unwrap(),
                    ]),
                    users: None,
                }]),
            },
            shard
        );
    }

    #[test]
    fn shard_with_non_zero_nonce_deserialization() {
        let s = r#"{
            "nonce": 100,
            "owners": ["tccqqtk3q3rea46cq4cpa4h5tm43nw3supd6uxtltxv"],
            "users": ["tccqp9lfw377aaxwl2f9s34h5lpfru0y5tlrc5avutn"]
        }"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: Some(Uint(U256::from(100))),
                owners: vec![PlatformAddress::from_str("tccqqtk3q3rea46cq4cpa4h5tm43nw3supd6uxtltxv").unwrap()],
                users: Some(vec![PlatformAddress::from_str("tccqp9lfw377aaxwl2f9s34h5lpfru0y5tlrc5avutn").unwrap()]),
                worlds: None,
            },
            shard
        );
    }


    #[test]
    fn deserialization_of_empty_shard_fails() {
        let s = r#"{
        }"#;
        let result: Result<Shard, _> = serde_json::from_str(s);
        assert!(result.is_err());
    }

    #[test]
    fn shard_without_nonce_deserialization() {
        let s = r#"{
            "owners": ["tccqqtk3q3rea46cq4cpa4h5tm43nw3supd6uxtltxv"]
        }"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: None,
                owners: vec![PlatformAddress::from_str("tccqqtk3q3rea46cq4cpa4h5tm43nw3supd6uxtltxv").unwrap()],
                users: None,
                worlds: None,
            },
            shard
        );
    }
}
