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

use super::super::uint::Uint;

/// Spec params.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Params {
    /// Account start nonce, defaults to 0.
    #[serde(rename = "accountStartNonce")]
    pub account_start_nonce: Option<Uint>,
    /// Maximum size of extra data.
    #[serde(rename = "maximumExtraDataSize")]
    pub maximum_extra_data_size: Uint,
    /// Network id.
    #[serde(rename = "networkID")]
    pub network_id: Uint,
    /// Minimum parcel cost.
    #[serde(rename = "minParcelCost")]
    pub min_parcel_cost: Uint,
}

#[cfg(test)]
mod tests {
    use ctypes::U256;
    use serde_json;

    use super::super::super::uint::Uint;
    use super::Params;

    #[test]
    fn params_deserialization() {
        let s = r#"{
			"accountStartNonce": "0x01",
			"maximumExtraDataSize": "0x20",
			"networkID" : "0x1",
			"minParcelCost" : "10"
		}"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.account_start_nonce, Some(Uint(U256::from(0x01))));
        assert_eq!(deserialized.maximum_extra_data_size, Uint(U256::from(0x20)));
        assert_eq!(deserialized.network_id, Uint(U256::from(0x1)));
        assert_eq!(deserialized.min_parcel_cost, Uint(U256::from(10)));
    }
}
