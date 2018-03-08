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

use std::fmt;
use std::str::FromStr;
use network::Network;
use {Error, AccountId};
use bech32::Bech32;
use hash::H160;

#[derive(Debug, PartialEq, Clone)]
pub struct Address {
	/// The network of the address.
	pub network: Network,
	/// Public key hash.
	pub account_id: AccountId,
}

trait IntoBase32 {
	fn into_base32(&self) -> Vec<u8>;
}

impl IntoBase32 for AccountId {
	fn into_base32(&self) -> Vec<u8> {
		let mut vec = Vec::new();
		for x in 0..4 {
			vec.push(((self[x * 5 + 0] & 0b11111000) >> 3));
			vec.push(((self[x * 5 + 0] & 0b00000111) << 2) | ((self[x * 5 + 1] & 0b11000000) >> 6));
			vec.push(((self[x * 5 + 1] & 0b00111110) >> 1));
			vec.push(((self[x * 5 + 1] & 0b00000001) << 4) | ((self[x * 5 + 2] & 0b11110000) >> 4));
			vec.push(((self[x * 5 + 2] & 0b00001111) << 1) | ((self[x * 5 + 3] & 0b10000000) >> 7));
			vec.push(((self[x * 5 + 3] & 0b01111100) >> 2));
			vec.push(((self[x * 5 + 3] & 0b00000011) << 3) | ((self[x * 5 + 4] & 0b11100000) >> 5));
			vec.push(((self[x * 5 + 4] & 0b00011111) >> 0));
		}
		vec
	}
}

impl fmt::Display for Address {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let hrp = match self.network {
			Network::Mainnet => "cc",
			Network::Testnet => "tc",
		};
		let encode_result = Bech32 {
			hrp: hrp.to_string(),
			data: self.account_id.into_base32(),
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
				let mut arr = [0u8; 20];
				for x in 0..4 {
					arr[x * 5 + 0] = ((decoded.data[x * 8 + 0] & 0b00011111) << 3) | ((decoded.data[x * 8 + 1] & 0b00011100) >> 2);
					arr[x * 5 + 1] = ((decoded.data[x * 8 + 1] & 0b00000011) << 6) | ((decoded.data[x * 8 + 2] & 0b00011111) << 1) | ((decoded.data[x * 8 + 3] & 0b00010000) >> 4);
					arr[x * 5 + 2] = ((decoded.data[x * 8 + 3] & 0b00001111) << 4) | ((decoded.data[x * 8 + 4] & 0b00011110) >> 1);
					arr[x * 5 + 3] = ((decoded.data[x * 8 + 4] & 0b00000001) << 7) | ((decoded.data[x * 8 + 5] & 0b00011111) << 2) | ((decoded.data[x * 8 + 6] & 0b00011000) >> 3);
					arr[x * 5 + 4] = ((decoded.data[x * 8 + 6] & 0b00000111) << 5) | ((decoded.data[x * 8 + 7] & 0b00011111) >> 0);
				}
				Ok(Address {
					network: network,
					account_id: H160(arr),
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
