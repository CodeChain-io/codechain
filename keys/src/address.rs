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
use codechain_types::H160;

use {Address, Error, Network};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FullAddress {
    /// The network of the address.
    pub network: Network,
    /// The version of the address.
    pub version: u8,
    /// Public key hash.
    pub address: Address,
}

fn rearrange_bits(data: &Vec<u8>, from: usize, into: usize) -> Vec<u8> {
    let mut vec = Vec::with_capacity((data.len() * from + (into - 1)) / into);

    let mut group_index = 0;
    let mut group_required_bits = into;

    for val in data.iter() {
        let mut ungrouped_bits= from;

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
        let hrp = match self.network {
            Network::Mainnet => "cc",
            Network::Testnet => "tc",
        }.to_string();
        let mut data = Vec::new();
        data.push(self.version);
        data.extend(&self.address.to_vec());
        let encode_result = Bech32 {
            hrp,
            data: rearrange_bits(&data, 8, 5),
        }.to_string();
        write!(f, "{}", encode_result.unwrap())
    }
}

impl FromStr for FullAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> where Self: Sized {
        let decoded = Bech32::from_string(s.to_string())?;
        let network = match decoded.hrp.as_str().as_ref() {
            "cc" => Some(Network::Mainnet),
            "tc" => Some(Network::Testnet),
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
                        H160(arr)
                    },
                })
            }
            None => Err(Error::Bech32UnknownHRP)
        }
    }
}

impl From<&'static str> for FullAddress {
    fn from(s: &'static str) -> Self {
        s.parse().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::{rearrange_bits, FullAddress};
    use {Network};

    #[test]
    fn test_full_address_to_string() {
        let address = FullAddress {
            network: Network::Mainnet,
            version: 0,
            address: "3f4aa1fedf1f54eeb03b759deadb36676b184911".into(),
        };

        assert_eq!("cc1qql54g07mu04fm4s8d6em6kmxenkkxzfzya9wyew".to_owned(), address.to_string());
    }

    #[test]
    fn test_address_from_str() {
        let address = FullAddress {
            network: Network::Mainnet,
            version: 0,
            address: "3f4aa1fedf1f54eeb03b759deadb36676b184911".into(),
        };

        assert_eq!(address, "cc1qql54g07mu04fm4s8d6em6kmxenkkxzfzya9wyew".into());
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

