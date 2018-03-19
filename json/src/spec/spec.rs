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

use std::io::Read;
use serde_json;
use serde_json::Error;
use super::{Genesis, Engine};

/// Spec deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Spec {
    /// Spec name.
    pub name: String,
    /// Special fork name.
    #[serde(rename="dataDir")]
    pub data_dir: Option<String>,
    /// Engine.
    pub engine: Engine,
    /// Genesis header.
    pub genesis: Genesis,
    /// Boot nodes.
    pub nodes: Option<Vec<String>>,
}

impl Spec {
    /// Loads test from json.
    pub fn load<R>(reader: R) -> Result<Self, Error> where R: Read {
        serde_json::from_reader(reader)
    }
}

#[cfg(test)]
mod tests {
    use serde_json;
    use super::Spec;

    #[test]
    fn spec_deserialization() {
        let s = r#"{
	"name": "Morden",
	"dataDir": "morden",
	"engine": {
		"tendermint": {
			"params": {
				"validators" : [
                                            "0x82a978b3f5962a5b0957d9ee9eef472ee55b42f1",
                                            "0x7d577a597b2742b498cb5cf0c26cdcd726d39e6e"
                                ],
				"timeoutPropose": 10000,
				"timeoutPrevote": 10000,
				"timeoutPrecommit": 10000,
				"timeoutCommit": 10000
			}
		}
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
		"author": "0x0000000000000000000000000000000000000000",
		"timestamp": "0x00",
		"parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000"
	},
	"nodes": [
		"enode://b1217cbaa440e35ed471157123fe468e19e8b5ad5bedb4b1fdbcbdab6fb2f5ed3e95dd9c24a22a79fdb2352204cea207df27d92bfd21bfd41545e8b16f637499@104.44.138.37:30303"
	]
		}"#;
        let _deserialized: Spec = serde_json::from_str(s).unwrap();
        // TODO: validate all fields
    }
}
