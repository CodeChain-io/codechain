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

use super::{BlakePoW, Cuckoo, NullEngine, Solo, SoloAuthority, Tendermint};

/// Engine deserialization.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    /// Null engine.
    Null(NullEngine),
    Solo(Solo),
    SoloAuthority(SoloAuthority),
    Tendermint(Tendermint),
    Cuckoo(Cuckoo),
    BlakePoW(BlakePoW),
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use serde_json;

    #[test]
    fn engine_deserialization() {
        let s = r#"{
            "null": {
                "params": {
                    "blockReward": "0x0d"
                }
            }
        }"#;

        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::Null(_) => {} // unit test in its own file.
            _ => panic!(),
        }

        let s = r#"{
            "solo": {
                "params": {
                    "blockReward": "0x0d"
                }
            }
        }"#;

        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::Solo(_) => {} // solo is unit tested in its own file.
            _ => panic!(),
        };

        let s = r#"{
            "soloAuthority": {
                "params": {
                    "durationLimit": "0x0d",
                    "validators" : ["0xc6d9d2cd449a754c494264e1809c50e34d64562b"]
                }
            }
        }"#;
        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::SoloAuthority(_) => {} // solo authority is unit tested in its own file.
            _ => panic!(),
        };

        let s = r#"{
            "tendermint": {
                "params": {
                    "validators": ["0xc6d9d2cd449a754c494264e1809c50e34d64562b"]
                }
            }
        }"#;
        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::Tendermint(_) => {} // Tendermint is unit tested in its own file.
            _ => panic!(),
        };

        let s = r#"{
            "cuckoo": {
                "params": {
                    "blockReward": "0x0d",
                    "minScore" : "0x020000",
                    "maxVertex" : "16",
                    "maxEdge" : "8",
                    "cycleLength" : "6"
                }
            }
        }"#;
        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::Cuckoo(_) => {} // Tendermint is unit tested in its own file.
            _ => panic!(),
        };

        let s = r#"{
            "blakePoW": {
                "params": {
                    "blockReward": "0x0d"
                }
            }
        }"#;
        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::BlakePoW(_) => {} // BlakePoW is unit tested in its own file.
            _ => panic!(),
        };
    }
}
