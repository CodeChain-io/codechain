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

use super::{Accounts, Engine, Genesis, Params, Shards};
use serde_json;
use serde_json::Error;
use std::io::Read;

/// Scheme deserialization.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scheme {
    /// Scheme name.
    pub name: String,
    /// Special fork name.
    pub data_dir: Option<String>,
    /// Engine.
    pub engine: Engine,
    /// Scheme params.
    pub params: Params,
    /// Genesis header.
    pub genesis: Genesis,
    /// Genesis state.
    pub accounts: Accounts,
    pub shards: Shards,
    /// Boot nodes.
    pub nodes: Option<Vec<String>>,
}

impl Scheme {
    /// Loads test from json.
    pub fn load<R>(reader: R) -> Result<Self, Error>
    where
        R: Read, {
        serde_json::from_reader(reader)
    }
}

#[cfg(test)]
mod tests {
    use super::Scheme;
    use serde_json;

    #[test]
    fn spec_deserialization() {
        let s = r#"{
            "name": "Morden",
            "dataDir": "morden",
            "engine": {
                "tendermint": {
                    "params": {
                        "validators" : [
                            "tccq8qlwpt7xcs9lec3c8tyt3kqxlgsus8q4qp3m6ft",
                            "tccqx6l27p92t5g86jmyz366rxy7tmqhkru8y37utys"
                        ],
                        "timeoutPropose": 10000,
                        "timeoutPrevote": 10000,
                        "timeoutPrecommit": 10000,
                        "timeoutCommit": 10000
                    }
                }
            },
            "params": {
                "maxExtraDataSize": "0x20",
                "maxMetadataSize": "0x0400",
                "networkID" : "tc",
                "minParcelCost" : "10",
                "maxBodySize": 4194304,
                "snapshotPeriod": 16384,
                "useShardValidator": true
            },
            "genesis": {
                "seal": {
                    "tendermint": {
                        "round": "0x0",
                        "proposal": "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                        "precommits": [
                        "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
                        ]
                    }
                },
                "score": "0x20000",
                "author": "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqhhn9p3",
                "timestamp": "0x00",
                "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000"
            },
            "nodes": [
            "enode://b1217cbaa440e35ed471157123fe468e19e8b5ad5bedb4b1fdbcbdab6fb2f5ed3e95dd9c24a22a79fdb2352204cea207df27d92bfd21bfd41545e8b16f637499@104.44.138.37:30303"
            ],
            "accounts": {
                "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqyca3rwt": { "balance": "1", "nonce": "1048576" },
                "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqgfrhflv": { "balance": "1", "nonce": "1048576" },
                "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqvxf40sk": { "balance": "1", "nonce": "1048576" },
                "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqszkma5z": { "balance": "1", "nonce": "1048576" },
                "tccq8txq9uafdg8y2de9m2tdkhsfsj3m9nluq94hyan": { "balance": "1606938044258990275541962092341162602522202993782792835301376", "nonce": "1048576" }
            },
            "shards": {
            }
        }"#;
        let _deserialized: Scheme = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
