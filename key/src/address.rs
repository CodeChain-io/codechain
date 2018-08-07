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
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use bech32::Bech32;
use heapsize::HeapSizeOf;
use primitives::H160;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};
use rustc_hex::FromHexError;
use serde::de::{Error as SerdeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Error, Network};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Address(H160);

impl Address {
    pub fn random() -> Self {
        Address(H160::random())
    }
}

impl Deref for Address {
    type Target = H160;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Address {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0)
    }
}

impl PartialOrd for Address {
    fn partial_cmp(&self, m: &Address) -> Option<cmp::Ordering> {
        self.0.partial_cmp(&m.0)
    }
}

impl Ord for Address {
    fn cmp(&self, m: &Address) -> cmp::Ordering {
        self.0.cmp(&m.0)
    }
}

impl Hash for Address {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Default for Address {
    fn default() -> Self {
        Address(Default::default())
    }
}

impl Encodable for Address {
    fn rlp_append(&self, s: &mut RlpStream) {
        let data: H160 = self.0.into();
        data.rlp_append(s);
    }
}

impl Decodable for Address {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let data = H160::decode(rlp)?;
        Ok(Address(data))
    }
}

impl FromStr for Address {
    type Err = FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Address(H160::from_str(s)?))
    }
}

impl From<H160> for Address {
    fn from(s: H160) -> Self {
        Address(s)
    }
}

impl From<u64> for Address {
    fn from(s: u64) -> Self {
        Address(H160::from(s))
    }
}

impl From<[u8; 20]> for Address {
    fn from(s: [u8; 20]) -> Self {
        Address(H160::from(s))
    }
}

impl From<&'static str> for Address {
    fn from(s: &'static str) -> Self {
        Address(H160::from(s))
    }
}

impl Into<[u8; 20]> for Address {
    fn into(self) -> [u8; 20] {
        self.0.into()
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0.as_ref()
    }
}

impl HeapSizeOf for Address {
    fn heap_size_of_children(&self) -> usize {
        self.0.heap_size_of_children()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FullAddress {
    /// The network of the address.
    pub network: Network,
    /// The version of the address.
    pub version: u8,
    /// Public key hash.
    pub address: Address,
}

impl FullAddress {
    pub fn create_version0(network_id: u64, address: Address) -> Result<Self, Error> {
        let network = match network_id {
            // FIXME: 0x11 is the network id for SOLO
            0x11 => Network::Mainnet,
            _ => return Err(Error::InvalidNetwork),
        };
        Ok(FullAddress {
            network,
            version: 0,
            address,
        })
    }

    fn to_string(&self) -> String {
        let hrp = match self.network {
            Network::Mainnet => "ccc",
            Network::Testnet => "tcc",
        }.to_string();
        let mut data = Vec::new();
        data.push(self.version);
        data.extend(&self.address.to_vec());
        let mut encoded = Bech32 {
            hrp,
            data: rearrange_bits(&data, 8, 5),
        }.to_string()
            .unwrap();
        encoded.remove(3);
        encoded
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

impl fmt::Display for FullAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl FromStr for FullAddress {
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
        let network = match decoded.hrp.as_str().as_ref() {
            "ccc" => Some(Network::Mainnet),
            "tcc" => Some(Network::Testnet),
            _ => None,
        };
        match network {
            Some(network) => {
                let data = rearrange_bits(&decoded.data, 5, 8);
                Ok(FullAddress {
                    network,
                    version: data[0],
                    address: {
                        let mut arr = [0u8; 20];
                        for i in 0..20 {
                            arr[i] = data[1 + i];
                        }
                        Address(H160(arr))
                    },
                })
            }
            None => Err(Error::Bech32UnknownHRP),
        }
    }
}

impl From<&'static str> for FullAddress {
    fn from(s: &'static str) -> Self {
        s.parse().unwrap()
    }
}

impl Serialize for FullAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl<'a> Deserialize<'a> for FullAddress {
    fn deserialize<D>(deserializer: D) -> Result<FullAddress, D::Error>
    where
        D: Deserializer<'a>, {
        deserializer.deserialize_any(FullAddressVisitor)
    }
}

struct FullAddressVisitor;

impl<'a> Visitor<'a> for FullAddressVisitor {
    type Value = FullAddress;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a bech32 encoded string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: SerdeError, {
        FullAddress::from_str(value).map_err(|e| SerdeError::custom(format!("{}", e)))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: SerdeError, {
        FullAddress::from_str(value.as_ref()).map_err(|e| SerdeError::custom(format!("{}", e)))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use serde_json;

    use super::{rearrange_bits, FullAddress, Network};

    #[test]
    fn test_full_address_serialize() {
        let address = FullAddress::from_str("cccqql54g07mu04fm4s8d6em6kmxenkkxzfzytqcve5").unwrap();
        let serialized = serde_json::to_string(&address).unwrap();
        assert_eq!(serialized, r#""cccqql54g07mu04fm4s8d6em6kmxenkkxzfzytqcve5""#);
    }

    #[test]
    fn test_full_address_deserialize() {
        let addr1: Result<FullAddress, _> = serde_json::from_str(r#""""#);
        let addr2: Result<FullAddress, _> = serde_json::from_str(r#""cccqql54g07mu04fm4s8d6em6kmxenkkxzfzytqcve5""#);

        assert!(addr1.is_err());
        assert!(addr2.is_ok());
    }

    #[test]
    fn test_full_address_to_string() {
        let address = FullAddress {
            network: Network::Mainnet,
            version: 0,
            address: "3f4aa1fedf1f54eeb03b759deadb36676b184911".into(),
        };

        assert_eq!("cccqql54g07mu04fm4s8d6em6kmxenkkxzfzytqcve5".to_string(), address.to_string());
    }

    #[test]
    fn test_address_from_str() {
        let address = FullAddress {
            network: Network::Mainnet,
            version: 0,
            address: "3f4aa1fedf1f54eeb03b759deadb36676b184911".into(),
        };

        assert_eq!(address, "cccqql54g07mu04fm4s8d6em6kmxenkkxzfzytqcve5".into());
    }

    #[test]
    fn test_rearrange_bits_from_8_into_5() {
        let vec = vec![0b11101110, 0b11101110, 0b11101110, 0b11101110, 0b11101110];
        let rearranged = rearrange_bits(&vec, 8, 5);
        assert_eq!(rearranged, vec![0b11101, 0b11011, 0b10111, 0b01110, 0b11101, 0b11011, 0b10111, 0b01110]);
    }

    #[test]
    fn test_rearrange_bits_from_5_into_8() {
        let vec = vec![0b11101, 0b11011, 0b10111, 0b01110, 0b11101, 0b11011, 0b10111, 0b01110];
        let rearranged = rearrange_bits(&vec, 5, 8);
        assert_eq!(rearranged, vec![0b11101110, 0b11101110, 0b11101110, 0b11101110, 0b11101110]);
    }

    #[test]
    fn test_rearrange_bits_from_8_into_5_padded() {
        let vec = vec![0b11101110, 0b11101110, 0b11101110];
        let rearranged = rearrange_bits(&vec, 8, 5);
        assert_eq!(rearranged, vec![0b11101, 0b11011, 0b10111, 0b01110, 0b11100]);
    }

    #[test]
    fn test_rearrange_bits_from_5_into_8_padded() {
        let vec = vec![0b11101, 0b11011, 0b10111, 0b01110, 0b11101];
        let rearranged = rearrange_bits(&vec, 5, 8);
        assert_eq!(rearranged, vec![0b11101110, 0b11101110, 0b11101110, 0b10000000]);
    }
}
