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

use super::super::hash::Address;
use super::super::uint::Uint;

/// Tendermint params deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct TendermintParams {
    /// Valid validators.
    pub validators: Vec<Address>,
    /// Propose step timeout in milliseconds.
    #[serde(rename = "timeoutPropose")]
    pub timeout_propose: Option<Uint>,
    /// Prevote step timeout in milliseconds.
    #[serde(rename = "timeoutPrevote")]
    pub timeout_prevote: Option<Uint>,
    /// Precommit step timeout in milliseconds.
    #[serde(rename = "timeoutPrecommit")]
    pub timeout_precommit: Option<Uint>,
    /// Commit step timeout in milliseconds.
    #[serde(rename = "timeoutCommit")]
    pub timeout_commit: Option<Uint>,
    /// Reward per block.
    #[serde(rename = "blockReward")]
    pub block_reward: Option<Uint>,
}

/// Tendermint engine deserialization.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Tendermint {
    pub params: TendermintParams,
}

#[cfg(test)]
mod tests {
    use primitives::H160;
    use serde_json;

    use super::super::super::hash::Address;
    use super::Tendermint;

    #[test]
    fn tendermint_deserialization() {
        let s = r#"{
            "params": {
                "validators": ["0xc6d9d2cd449a754c494264e1809c50e34d64562b"]
            }
        }"#;

        let deserialized: Tendermint = serde_json::from_str(s).unwrap();
        let vs = vec![Address(H160::from("0xc6d9d2cd449a754c494264e1809c50e34d64562b"))];
        assert_eq!(deserialized.params.validators, vs);
    }
}
