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

#[derive(Debug, PartialEq, Deserialize)]
pub struct World {
    pub nonce: Option<Uint>,
    pub owners: Option<Vec<H160>>,
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;

    #[test]
    fn deserialization() {
        let s = r#"{
            "nonce": 0,
            "owners": ["0x01234567890abcdef0123456789abcdef0123456"]
        }"#;
        let world: World = serde_json::from_str(s).unwrap();
        assert_eq!(
            World {
                nonce: Some(Uint(0.into())),
                owners: Some(vec![H160::from("01234567890abcdef0123456789abcdef0123456")]),
            },
            world
        );
    }

    #[test]
    fn with_non_zero_nonce_deserialization() {
        let s = r#"{
            "nonce": 100,
            "owners": ["0x01234567890abcdef0123456789abcdef0123456"]
        }"#;
        let world: World = serde_json::from_str(s).unwrap();
        assert_eq!(
            World {
                nonce: Some(Uint(100.into())),
                owners: Some(vec![H160::from("01234567890abcdef0123456789abcdef0123456")]),
            },
            world
        );
    }


    #[test]
    fn deserialization_of_empty_world() {
        let s = r#"{
        }"#;
        let world: World = serde_json::from_str(s).unwrap();
        assert_eq!(
            World {
                nonce: None,
                owners: None,
            },
            world
        );
    }

    #[test]
    fn world_without_nonce_deserialization() {
        let s = r#"{
            "owners": ["0x01234567890abcdef0123456789abcdef0123456"]
        }"#;
        let world: World = serde_json::from_str(s).unwrap();
        assert_eq!(
            World {
                nonce: None,
                owners: Some(vec![H160::from("01234567890abcdef0123456789abcdef0123456")]),
            },
            world
        );
    }
}
