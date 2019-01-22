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

use std::cmp;
use std::fmt;
use std::str::FromStr;

use bech32::Bech32;
use parking_lot::Mutex;
use primitives::H160;
use serde::de::{Error as SerdeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Address, Error, NetworkId};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash)]
pub struct PlatformAddress {
    /// The network id of the address.
    pub network_id: NetworkId,
    /// The version of the address.
    pub version: u8,
    /// Public key hash.
    address: Address,
}

impl PlatformAddress {
    pub fn new_v1(network_id: NetworkId, address: Address) -> Self {
        assert!(check_network_id(network_id));
        Self {
            network_id,
            version: 1,
            address,
        }
    }

    pub fn address(&self) -> &Address {
        self.try_address().unwrap()
    }

    pub fn into_address(self) -> Address {
        self.try_into_address().unwrap()
    }

    pub fn try_address(&self) -> Result<&Address, Error> {
        if !check_network_id(self.network_id) {
            return Err(Error::InvalidNetworkId(self.network_id))
        }
        Ok(&self.address)
    }

    pub fn try_into_address(self) -> Result<Address, Error> {
        if !check_network_id(self.network_id) {
            return Err(Error::InvalidNetworkId(self.network_id))
        }
        Ok(self.address)
    }
}

fn rearrange_bits(data: &[u8], from: usize, into: usize) -> Vec<u8> {
    let mut vec = Vec::with_capacity((data.len() * from + (into - 1)) / into);

    let mut group_index = 0;
    let mut group_required_bits = into;

    for val in data.iter() {
        let mut ungrouped_bits = from;

        while ungrouped_bits > 0 {
            let min = cmp::min(group_required_bits, ungrouped_bits);
            let min_mask = (1 << min) - 1;

            if group_required_bits == into {
                vec.push(0);
            }

            if ungrouped_bits >= group_required_bits {
                vec[group_index] |= (val >> (ungrouped_bits - group_required_bits)) & min_mask;
            } else {
                vec[group_index] |= (val & min_mask) << (group_required_bits - ungrouped_bits);
            }

            group_required_bits -= min;
            if group_required_bits == 0 {
                group_index += 1;
                group_required_bits = into;
            }
            ungrouped_bits -= min;
        }
    }
    vec
}

impl fmt::Display for PlatformAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        assert!(check_network_id(self.network_id));
        let hrp = format!("{}c", self.network_id);
        let mut data = Vec::new();
        data.push(self.version);
        data.extend(&self.address.to_vec());
        let mut encoded = Bech32 {
            hrp,
            data: rearrange_bits(&data, 8, 5),
        }
        .to_string()
        .unwrap();
        encoded.remove(3);
        write!(f, "{}", encoded)
    }
}

impl FromStr for PlatformAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error>
    where
        Self: Sized, {
        if s.len() < 7 {
            return Err(Error::Bech32InvalidLength)
        }
        let mut encoded = s.to_string();
        encoded.insert(3, '1');
        let decoded = Bech32::from_string(encoded)?;
        let network_id = decoded
            .hrp
            .get(0..2)
            .expect("decoded.hrp.len() == 3")
            .parse::<NetworkId>()
            .map_err(|_| Error::Bech32UnknownHRP)?;
        if !check_network_id(network_id) {
            return Err(Error::InvalidNetworkId(network_id))
        }
        if Some("c") != decoded.hrp.get(2..3) {
            return Err(Error::Bech32UnknownHRP)
        }
        let data = rearrange_bits(&decoded.data, 5, 8);
        if data[0] != 1 {
            return Err(Error::InvalidPlatformAddressVersion(data[0]))
        }
        Ok(Self {
            network_id,
            version: data[0],
            address: {
                let mut arr = [0u8; 20];
                arr[..20].copy_from_slice(&data[1..=20]);
                H160(arr).into()
            },
        })
    }
}

impl From<&'static str> for PlatformAddress {
    fn from(s: &'static str) -> Self {
        s.parse().unwrap_or_else(|_| panic!("invalid string literal for {}: '{}'", stringify!(Self), s))
    }
}

impl Serialize for PlatformAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl<'a> Deserialize<'a> for PlatformAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>, {
        deserializer.deserialize_any(PlatformAddressVisitor)
    }
}

struct PlatformAddressVisitor;

impl<'a> Visitor<'a> for PlatformAddressVisitor {
    type Value = PlatformAddress;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a bech32 encoded string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: SerdeError, {
        PlatformAddress::from_str(value).map_err(|e| SerdeError::custom(format!("{}", e)))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: SerdeError, {
        PlatformAddress::from_str(value.as_ref()).map_err(|e| SerdeError::custom(format!("{}", e)))
    }
}

// FIXME: The below code can be simplified since Mutex::new is the const function.
//        Clean up this function when a const function becomes stable.
lazy_static! {
    static ref NETWORK_ID: Mutex<Option<NetworkId>> = Mutex::new(None);
}
fn check_network_id(network_id: NetworkId) -> bool {
    let mut saved_network_id = NETWORK_ID.lock();
    if saved_network_id.is_none() {
        *saved_network_id = Some(network_id);
    }
    *saved_network_id == Some(network_id)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use serde_json;

    use super::{rearrange_bits, PlatformAddress};

    #[test]
    fn serialization() {
        let address = PlatformAddress::from_str("tccq8txjnstz9h2uj2xw4jczejp57ew9zp7nqycg65e").unwrap();
        let serialized = serde_json::to_string(&address).unwrap();
        assert_eq!(serialized, r#""tccq8txjnstz9h2uj2xw4jczejp57ew9zp7nqycg65e""#);
    }

    #[test]
    fn deserialization() {
        let addr1: Result<PlatformAddress, _> = serde_json::from_str(r#""""#);
        let addr2: Result<PlatformAddress, _> =
            serde_json::from_str(r#""tccq8txjnstz9h2uj2xw4jczejp57ew9zp7nqycg65e""#);

        assert!(addr1.is_err());
        assert!(addr2.is_ok());
    }

    #[test]
    fn to_string() {
        let address = PlatformAddress {
            network_id: "tc".into(),
            version: 1,
            address: "3f4aa1fedf1f54eeb03b759deadb36676b184911".into(),
        };

        assert_eq!("tccqyl54g07mu04fm4s8d6em6kmxenkkxzfzyxyy2hg".to_string(), address.to_string());
    }

    #[test]
    fn from_str() {
        let address = PlatformAddress {
            network_id: "tc".into(),
            version: 1,
            address: "3f4aa1fedf1f54eeb03b759deadb36676b184911".into(),
        };

        assert_eq!(address, "tccqyl54g07mu04fm4s8d6em6kmxenkkxzfzyxyy2hg".into());
    }

    #[test]
    fn rearrange_bits_from_8_into_5() {
        let vec = vec![0b1110_1110, 0b1110_1110, 0b1110_1110, 0b1110_1110, 0b1110_1110];
        let rearranged = rearrange_bits(&vec, 8, 5);
        assert_eq!(rearranged, vec![0b11101, 0b11011, 0b10111, 0b01110, 0b11101, 0b11011, 0b10111, 0b01110]);
    }

    #[test]
    fn rearrange_bits_from_5_into_8() {
        let vec = vec![0b11101, 0b11011, 0b10111, 0b01110, 0b11101, 0b11011, 0b10111, 0b01110];
        let rearranged = rearrange_bits(&vec, 5, 8);
        assert_eq!(rearranged, vec![0b1110_1110, 0b1110_1110, 0b1110_1110, 0b1110_1110, 0b1110_1110]);
    }

    #[test]
    fn rearrange_bits_from_8_into_5_padded() {
        let vec = vec![0b1110_1110, 0b1110_1110, 0b1110_1110];
        let rearranged = rearrange_bits(&vec, 8, 5);
        assert_eq!(rearranged, vec![0b11101, 0b11011, 0b10111, 0b01110, 0b11100]);
    }

    #[test]
    fn rearrange_bits_from_5_into_8_padded() {
        let vec = vec![0b11101, 0b11011, 0b10111, 0b01110, 0b11101];
        let rearranged = rearrange_bits(&vec, 5, 8);
        assert_eq!(rearranged, vec![0b1110_1110, 0b1110_1110, 0b1110_1110, 0b1000_0000]);
    }
}
