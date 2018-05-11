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

use super::super::bytes::Bytes;
use super::super::hash::{Address, H256};
use super::super::uint::Uint;
use super::Seal;

/// Spec genesis.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Genesis {
    /// Seal.
    pub seal: Seal,
    /// Difficulty.
    pub score: Uint,
    /// Block author, defaults to 0.
    pub author: Option<Address>,
    /// Block timestamp, defaults to 0.
    pub timestamp: Option<Uint>,
    /// Parent hash, defaults to 0.
    #[serde(rename = "parentHash")]
    pub parent_hash: Option<H256>,
    /// Parcels root.
    #[serde(rename = "parcelsRoot")]
    pub parcels_root: Option<H256>,
    /// Invoices root.
    #[serde(rename = "invoicesRoot")]
    pub invoices_root: Option<H256>,
    /// State root.
    #[serde(rename = "stateRoot")]
    pub state_root: Option<H256>,
    /// Extra data.
    #[serde(rename = "extraData")]
    pub extra_data: Option<Bytes>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ctypes::{H160, H256 as Eth256, H520 as Eth520, U256};
    use serde_json;

    use super::super::super::bytes::Bytes;
    use super::super::super::hash::{Address, H256, H520};
    use super::super::super::uint::Uint;
    use super::super::{Seal, TendermintSeal};
    use super::Genesis;

    #[test]
    fn genesis_deserialization() {
        let s = r#"{
			"score": "0x400000000",
                        "seal": {
                            "tendermint": {
				"round": "0x0",
				"proposal": "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
				"precommits": [
					"0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
				]
                            }
                        },
			"author": "0x1000000000000000000000000000000000000001",
			"timestamp": "0x07",
			"parentHash": "0x9000000000000000000000000000000000000000000000000000000000000000",
			"extraData": "0x11bbe8db4e347b4e8c937c1c8370e4b5ed33adb3db69cbdb7a38e1e50b1b82fa",
			"stateRoot": "0xd7f8974fb5ac78d9ac099b9ad5018bedc2ce0a72dad1827a1709da30580f0544"
		}"#;
        let deserialized: Genesis = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized, Genesis {
            seal: Seal::Tendermint(TendermintSeal {
                round: Uint(U256::from(0x0)),
                proposal: H520(Eth520::from("0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000")),
                precommits: vec![
                    H520(Eth520::from("0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000")),
                ]
            }),
            score: Uint(U256::from(0x400000000u64)),
            author: Some(Address(H160::from("0x1000000000000000000000000000000000000001"))),
            timestamp: Some(Uint(U256::from(0x07))),
            parent_hash: Some(H256(Eth256::from("0x9000000000000000000000000000000000000000000000000000000000000000"))),
            parcels_root: None,
            invoices_root: None,
            state_root: Some(H256(Eth256::from("0xd7f8974fb5ac78d9ac099b9ad5018bedc2ce0a72dad1827a1709da30580f0544"))),
            extra_data: Some(Bytes::from_str("0x11bbe8db4e347b4e8c937c1c8370e4b5ed33adb3db69cbdb7a38e1e50b1b82fa").unwrap()),
        });
    }
}
