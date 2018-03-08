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

use rand::os::OsRng;
use network::Network;
use {KeyPair, SECP256K1, Error};

pub trait Generator {
	fn generate(&self) -> Result<KeyPair, Error>;
}

pub struct Random {
	network: Network
}

impl Random {
	pub fn new(network: Network) -> Self {
		Random {
			network: network,
		}
	}
}

impl Generator for Random {
	fn generate(&self) -> Result<KeyPair, Error> {
		let context = &SECP256K1;
		let mut rng = OsRng::new().map_err(|_| Error::FailedKeyGeneration)?;
		let (secret, public) = context.generate_keypair(&mut rng)?;
		Ok(KeyPair::from_keypair(secret, public, self.network))
	}
}
