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

use super::{BlakePoW, Cuckoo, NullEngine, SimplePoA, Solo, Tendermint};

/// Engine deserialization.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    /// Null engine.
    Null(NullEngine),
    Solo(Solo),
    SimplePoA(SimplePoA),
    Tendermint(Box<Tendermint>),
    Cuckoo(Box<Cuckoo>),
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
            "simplePoA": {
                "params": {
                    "durationLimit": "0x0d",
                    "validators" : ["0x4f1541fc6bdec60bf0ac6380a8e3914a469fe6cd4fa817c890d5823cfdda83932f61dc083e1b6736dadeceb5afd3fcfbac915e5fa2c9c20acf1c30b080114d7f"]
                }
            }
        }"#;
        let deserialized: Engine = serde_json::from_str(s).unwrap();
        match deserialized {
            Engine::SimplePoA(_) => {} // simple poa is unit tested in its own file.
            _ => panic!(),
        };

        let s = r#"{
            "tendermint": {
                "params": {
                    "validators": ["0x1ac8248deb29a58c4bdbfce031fb22c7ba3bcc9384bf6de058a1c8bef5a17422cf8ca26666a5505684db7364eabeed6fc678b02658ae7c1848a4ae6e50244cf2"]
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
