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

use crate::bytes::Bytes;
use crate::hash::H520;
use crate::uint::Uint;

/// Tendermint seal.
#[derive(Debug, PartialEq, Deserialize)]
pub struct TendermintSeal {
    /// Seal round.
    pub prev_view: Uint,
    /// Proposal seal signature.
    pub cur_view: Uint,
    /// Proposal seal signature.
    pub precommits: Vec<H520>,
}

/// Seal variants.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Seal {
    /// Tendermint seal.
    Tendermint(TendermintSeal),
    /// Generic seal.
    Generic(Bytes),
}

#[cfg(test)]
mod tests {
    use primitives::{H520 as Core520, U256};
    use serde_json;

    use super::{Seal, TendermintSeal};
    use crate::bytes::Bytes;
    use crate::hash::H520;
    use crate::uint::Uint;

    #[test]
    fn seal_deserialization() {
        let s = r#"[{
            "generic": "0xe011bbe8db4e347b4e8c937c1c8370e4b5ed33adb3db69cbdb7a38e1e50b1b82fa"
        },{
            "tendermint": {
                "prev_view": "0x3",
                "cur_view": "0x4",
                "precommits": [
                "0x4000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004"
                ]
            }
        }]"#;

        let deserialized: Vec<Seal> = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.len(), 2);

        // [0]
        assert_eq!(
            deserialized[0],
            Seal::Generic(Bytes::new(vec![
                0xe0, 0x11, 0xbb, 0xe8, 0xdb, 0x4e, 0x34, 0x7b, 0x4e, 0x8c, 0x93, 0x7c, 0x1c, 0x83, 0x70, 0xe4, 0xb5,
                0xed, 0x33, 0xad, 0xb3, 0xdb, 0x69, 0xcb, 0xdb, 0x7a, 0x38, 0xe1, 0xe5, 0x0b, 0x1b, 0x82, 0xfa,
            ]))
        );

        // [1]
        assert_eq!(deserialized[1], Seal::Tendermint(TendermintSeal {
            prev_view: Uint(U256::from(0x3)),
            cur_view: Uint(U256::from(0x4)),
            precommits: vec![H520(Core520::from("0x4000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004"))]
        }));
    }
}
