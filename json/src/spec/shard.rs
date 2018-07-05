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

#[derive(Debug, PartialEq, Deserialize)]
pub struct Shard {
    pub nonce: Option<u64>,
    // FIXME: Add worlds
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;

    #[test]
    fn shard_deserialization() {
        let s = r#"{
			"nonce": 0
		}"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: Some(0)
            },
            shard
        );
    }

    #[test]
    fn shard_with_non_zero_nonce_deserialization() {
        let s = r#"{
			"nonce": 100
		}"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: Some(100)
            },
            shard
        );
    }

    #[test]
    fn shard_without_nonce_deserialization() {
        let s = r#"{
		}"#;
        let shard: Shard = serde_json::from_str(s).unwrap();
        assert_eq!(
            Shard {
                nonce: None
            },
            shard
        );
    }
}
