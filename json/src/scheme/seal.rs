// Copyright 2018-2019 Kodebox, Inc.
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
use crate::hash::{H256, H520};
use crate::uint::Uint;

#[derive(Debug, PartialEq, Deserialize)]
pub struct SeedInfo {
    /// Seed signer index in the validator set
    pub seed_signer_idx: Uint,
    /// Seed hash generated from the vrf
    pub seed: H256,
    /// Seed proof
    pub proof: Bytes,
}

/// Tendermint seal.
#[derive(Debug, PartialEq, Deserialize)]
pub struct TendermintSeal {
    /// Seal round.
    pub prev_view: Uint,
    /// Proposal seal signature.
    pub cur_view: Uint,
    /// Proposal seal signature.
    pub precommits: Vec<H520>,
    /// Precommit signatures' bitset
    pub precommit_bitset: Bytes,
    /// Seed information for randomized leader election
    pub vrf_seed_info: SeedInfo,
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
    use primitives::{H256 as Core256, H520 as Core520};
    use serde_json;

    use super::{Seal, SeedInfo, TendermintSeal};
    use crate::bytes::Bytes;
    use crate::hash::{H256, H520};

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
                ],
                "precommit_bitset": "0x0000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000001",
                "vrf_seed_info": {
                    "seed_signer_idx": "0x0",
                    "seed": "0x0000000000000000000000000000000000000000000000000000000000000001",
                    "proof": "0x0000001000000000000000000000000000000000000000000000000000000001"
                }
            }
        }]"#;

        let deserialized: Vec<Seal> = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized, vec![
            Seal::Generic(Bytes::new(vec![
                0xe0, 0x11, 0xbb, 0xe8, 0xdb, 0x4e, 0x34, 0x7b, 0x4e, 0x8c, 0x93, 0x7c, 0x1c, 0x83, 0x70, 0xe4, 0xb5,
                0xed, 0x33, 0xad, 0xb3, 0xdb, 0x69, 0xcb, 0xdb, 0x7a, 0x38, 0xe1, 0xe5, 0x0b, 0x1b, 0x82, 0xfa,
            ])),
            Seal::Tendermint(TendermintSeal {
                prev_view: 0x3.into(),
                cur_view: 0x4.into(),
                precommits: vec![H520(Core520::from("0x4000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004"))],
                precommit_bitset: Bytes::new(vec![
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
                ]),
                vrf_seed_info: SeedInfo {
                    seed_signer_idx: 0x0.into(),
                    seed: H256(Core256::from("0x0000000000000000000000000000000000000000000000000000000000000001")),
                    proof: Bytes::new(vec![
                        0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
                    ])
                },
            }),
        ]);
    }
}
