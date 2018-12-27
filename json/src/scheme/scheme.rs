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
                            "0x4f1541fc6bdec60bf0ac6380a8e3914a469fe6cd4fa817c890d5823cfdda83932f61dc083e1b6736dadeceb5afd3fcfbac915e5fa2c9c20acf1c30b080114d7f",
                            "0x1ac8248deb29a58c4bdbfce031fb22c7ba3bcc9384bf6de058a1c8bef5a17422cf8ca26666a5505684db7364eabeed6fc678b02658ae7c1848a4ae6e50244cf2"
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
                "maxTextContentSize": "0x0200",
                "networkID" : "tc",
                "minPayParcelCost" : 10,
                "minSetRegularKeyParcelCost" : 11,
                "minCreateShardParcelCost" : 12,
                "minSetShardOwnersParcelCost" : 13,
                "minSetShardUsersParcelCost" : 14,
                "minWrapCccParcelCost" : 15,
                "minCustomParcelCost" : 16,
                "minStoreParcelCost" : 17,
                "minRemoveParcelCost" : 18,
                "minAssetMintCost" : 19,
                "minAssetTransferCost" : 20,
                "minAssetSchemeChangeCost" : 21,
                "minAssetComposeCost" : 22,
                "minAssetDecomposeCost" : 23,
                "minAssetUnwrapCccCost" : 24,
                "maxBodySize": 4194304,
                "snapshotPeriod": 16384
            },
            "genesis": {
                "seal": {
                    "tendermint": {
                        "prev_view": "0x0",
                        "cur_view": "0x0",
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
                "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqyca3rwt": { "balance": "1", "seq": "1048576" },
                "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqgfrhflv": { "balance": "1", "seq": "1048576" },
                "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqvxf40sk": { "balance": "1", "seq": "1048576" },
                "tccqyqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqszkma5z": { "balance": "1", "seq": "1048576" },
                "tccq8txq9uafdg8y2de9m2tdkhsfsj3m9nluq94hyan": { "balance": "1606938044258990275541962092341162602522202993782792835301376", "seq": "1048576" }
            },
            "shards": {
            }
        }"#;
        let _deserialized: Scheme = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
