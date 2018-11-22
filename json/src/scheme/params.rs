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

use ckey::NetworkId;

use crate::uint::Uint;

/// Scheme params.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    /// Maximum size of extra data.
    pub max_extra_data_size: Uint,
    /// Maximum size of metadata.
    pub max_metadata_size: Uint,
    /// Network id.
    #[serde(rename = "networkID")]
    pub network_id: NetworkId,
    /// Minimum parcel cost.
    pub min_parcel_cost: Uint,
    /// Maximum size of block body.
    pub max_body_size: Uint,
    /// Snapshot creation period in unit of block numbers.
    pub snapshot_period: Uint,
}

#[cfg(test)]
mod tests {
    use primitives::U256;
    use serde_json;

    use super::Params;
    use crate::uint::Uint;

    #[test]
    fn params_deserialization() {
        let s = r#"{
            "maxExtraDataSize": "0x20",
            "maxMetadataSize": "0x0400",
            "networkID" : "tc",
            "minParcelCost" : "10",
            "maxBodySize" : 4194304,
            "snapshotPeriod": 16384
        }"#;

        let deserialized: Params = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized.max_extra_data_size, Uint(U256::from(0x20)));
        assert_eq!(deserialized.max_metadata_size, Uint(U256::from(0x0400)));
        assert_eq!(deserialized.network_id, "tc".into());
        assert_eq!(deserialized.min_parcel_cost, Uint(U256::from(10)));
        assert_eq!(deserialized.max_body_size, Uint(4194304.into()));
        assert_eq!(deserialized.snapshot_period, Uint(16384.into()));
    }
}
