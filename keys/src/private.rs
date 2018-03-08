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

//! Secret with additional network identifier and format type

use std::fmt;
use std::str::FromStr;
use secp256k1::key::SecretKey;
use secp256k1::Message as SecpMessage;
use hex::ToHex;
use base58::{ToBase58, FromBase58};
use network::Network;
use {Secret, DisplayLayout, Error, Message, Signature, SECP256K1};

/// Secret with additional network identifier and format type
#[derive(PartialEq)]
pub struct Private {
	/// The network on which this key should be used.
	pub network: Network,
	/// ECDSA key.
	pub secret: Secret,
	/// True if this private key represents a compressed address.
	pub compressed: bool,
}

impl Private {
	pub fn sign(&self, message: &Message) -> Result<Signature, Error> {
		let context = &SECP256K1;
		let sec = SecretKey::from_slice(context, &self.secret)?;
		let s = context.sign_recoverable(&SecpMessage::from_slice(&message[..])?, &sec)?;
		let (rec_id, data) = s.serialize_compact(context);
		let mut data_arr = [0; 65];

		// no need to check if s is low, it always is
		data_arr[0..64].copy_from_slice(&data[0..64]);
		data_arr[64] = rec_id.to_i32() as u8;
		Ok(Signature(data_arr))
	}
}

impl DisplayLayout for Private {
	type Target = Vec<u8>;

	fn layout(&self) -> Self::Target {
		let mut result = vec![];
		let network_byte = match self.network {
			Network::Mainnet => 128,
			Network::Testnet => 239,
		};

		result.push(network_byte);
		result.extend(&*self.secret);
		if self.compressed {
			result.push(1);
		}
		result
	}

	fn from_layout(data: &[u8]) -> Result<Self, Error> where Self: Sized {
		let compressed = match data.len() {
			33 => false,
			34 => true,
			_ => return Err(Error::InvalidPrivate),
		};

		if compressed && data[data.len() - 1] != 1 {
			return Err(Error::InvalidPrivate);
		}

		let network = match data[0] {
			128 => Network::Mainnet,
			239 => Network::Testnet,
			_ => return Err(Error::InvalidPrivate),
		};

		let mut secret = Secret::default();
		secret.copy_from_slice(&data[1..33]);

		let private = Private {
			network: network,
			secret: secret,
			compressed: compressed,
		};

		Ok(private)
	}
}

impl fmt::Debug for Private {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		try!(writeln!(f, "network: {:?}", self.network));
		try!(writeln!(f, "secret: {}", self.secret.to_hex()));
		writeln!(f, "compressed: {}", self.compressed)
	}
}

impl fmt::Display for Private {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.layout().to_base58().fmt(f)
	}
}

impl FromStr for Private {
	type Err = Error;

	fn from_str(s: &str) -> Result<Self, Error> where Self: Sized {
		let hex = try!(s.from_base58().map_err(|_| Error::InvalidPrivate));
		Private::from_layout(&hex)
	}
}

impl From<&'static str> for Private {
	fn from(s: &'static str) -> Self {
		s.parse().unwrap()
	}
}

#[cfg(test)]
mod tests {
	use codechain_types::H256;
	use network::Network;
	use super::Private;

	#[test]
	fn test_private_to_string() {
		let mut secret = H256::from("063377054c25f98bc538ac8dd2cf9064dd5d253a725ece0628a34e2f84803bd5");
		secret.reverse();
		let private = Private {
			network: Network::Mainnet,
			secret: secret,
			compressed: false,
		};

		assert_eq!("fGjuuRDK2425kL9J4KPf3S74zv617zwhZQQ8mgQGEnBhP".to_owned(), private.to_string());
	}

	#[test]
	fn test_private_from_str() {
		let mut secret = H256::from("063377054c25f98bc538ac8dd2cf9064dd5d253a725ece0628a34e2f84803bd5");
		secret.reverse();
		let private = Private {
			network: Network::Mainnet,
			secret: secret,
			compressed: false,
		};

		assert_eq!(private, "fGjuuRDK2425kL9J4KPf3S74zv617zwhZQQ8mgQGEnBhP".into());
	}
}
