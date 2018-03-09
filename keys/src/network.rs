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

use rlp::{UntrustedRlp, RlpStream, Encodable, Decodable, DecoderError};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Network {
	Mainnet,
	Testnet,
}

impl Decodable for Network {
	fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
		let network: u8 = d.as_val()?;
		match network {
			0 => Ok(Network::Mainnet),
			1 => Ok(Network::Testnet),
			_ => Err(DecoderError::Custom("Unknown network"))
		}
	}
}

impl Encodable for Network {
	fn rlp_append(&self, s: &mut RlpStream) {
		s.append(&(*self as u8));
	}
}

