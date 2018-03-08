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

use std::{fmt, ops};
use secp256k1::key;
use secp256k1::{Message as SecpMessage, RecoveryId, RecoverableSignature, Error as SecpError, Signature as SecpSignature};
use hex::ToHex;
use crypto::{blake256, ripemd160};
use hash::{H264, H520};
use {AccountId, Error, Signature, Message, SECP256K1};

/// Secret public key
pub enum Public {
	/// Normal version of public key
	Normal(H520),
	/// Compressed version of public key
	Compressed(H264),
}

impl Public {
	pub fn from_slice(data: &[u8]) -> Result<Self, Error> {
		match data.len() {
			33 => {
				let mut public = H264::default();
				public.copy_from_slice(data);
				Ok(Public::Compressed(public))
			},
			65 => {
				let mut public = H520::default();
				public.copy_from_slice(data);
				Ok(Public::Normal(public))
			},
			_ => Err(Error::InvalidPublic)
		}
	}

	pub fn account_id(&self) -> AccountId {
		ripemd160(blake256(self.as_ref()))
	}
}

impl ops::Deref for Public {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		match *self {
			Public::Normal(ref hash) => &**hash,
			Public::Compressed(ref hash) => &**hash,
		}
	}
}

impl PartialEq for Public {
	fn eq(&self, other: &Self) -> bool {
		let s_slice: &[u8] = self;
		let o_slice: &[u8] = other;
		s_slice == o_slice
	}
}

impl fmt::Debug for Public {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Public::Normal(ref hash) => writeln!(f, "normal: {}", hash.to_hex()),
			Public::Compressed(ref hash) => writeln!(f, "compressed: {}", hash.to_hex()),
		}
	}
}

impl fmt::Display for Public {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		self.to_hex().fmt(f)
	}
}
