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
use network::Network;
use {Error, AccountId};
use bech32::Bech32;
use codechain_types::H160;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Address {
    /// The network of the address.
    pub network: Network,
    /// Public key hash.
    pub account_id: AccountId,
}

impl Address {
    pub fn default(network: Network) -> Self {
        Address {
            network,
            account_id: Default::default(),
        }
    }
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

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hrp = match self.network {
            Network::Mainnet => "cc",
            Network::Testnet => "tc",
        };
        let encode_result = Bech32 {
            hrp: hrp.to_string(),
            data: rearrange_bits(&self.account_id.to_vec(), 8, 5),
        }.to_string();
        write!(f, "{}", encode_result.unwrap())
    }
}

impl FromStr for Address {
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
                Ok(Address {
                    network,
                    account_id: {
                        let vec = rearrange_bits(&decoded.data, 5, 8);
                        let mut arr = [0u8; 20];
                        for i in 0..20 {
                            arr[i] = vec[i];
                        }
                        H160(arr)
                    },
                })
            }
            None => Err(Error::Bech32UnknownHRP)
        }
    }
}

impl From<&'static str> for Address {
    fn from(s: &'static str) -> Self {
        s.parse().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use network::Network;
    use {Address, Message, Generator, Random};

    #[test]
    fn test_address_to_string() {
        let address = Address {
            network: Network::Mainnet,
            account_id: "3f4aa1fedf1f54eeb03b759deadb36676b184911".into(),
        };

        assert_eq!("cc18a92rlklra2wavpmwkw74kekva43sjg3u9ct0x".to_owned(), address.to_string());
    }

    #[test]
    fn test_address_from_str() {
        let address = Address {
            network: Network::Mainnet,
            account_id: "3f4aa1fedf1f54eeb03b759deadb36676b184911".into(),
        };

        assert_eq!(address, "cc18a92rlklra2wavpmwkw74kekva43sjg3u9ct0x".into());
    }

    #[test]
    fn sign_and_verify() {
        let random = Random::new(Network::Mainnet);
        let keypair = random.generate().unwrap();
        let message = Message::default();
        let private= keypair.private();
        let public = keypair.public();
        let signature = private.sign(&message).unwrap();
        assert!(public.verify(&signature, &message).unwrap());
    }
}
