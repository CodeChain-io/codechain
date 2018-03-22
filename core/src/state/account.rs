// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Single account in the system.

use std::fmt;
use std::sync::Arc;
use std::collections::{HashMap, BTreeMap};
use ccrypto::blake256;
use ctypes::{H256, U256, Address};
use cbytes::Bytes;
use rlp::*;

use std::cell::Cell;

/// Single account in the system.
#[derive(RlpEncodable, RlpDecodable)]
pub struct Account {
	// Balance of the account.
	balance: U256,
	// Nonce of the account.
	nonce: U256,
}

impl Account {
	#[cfg(test)]
	/// General constructor.
	pub fn new(balance: U256, nonce: U256) -> Account {
		Account {
			balance: balance,
			nonce: nonce,
		}
	}

	/// Create a new account with the given balance.
	pub fn new_basic(balance: U256, nonce: U256) -> Account {
		Account {
			balance: balance,
			nonce: nonce,
		}
	}

	/// Create a new account from RLP.
	pub fn from_rlp(rlp: &[u8]) -> Account {
		::rlp::decode(rlp)
	}

	/// return the balance associated with this account.
	pub fn balance(&self) -> &U256 { &self.balance }

	/// return the nonce associated with this account.
	pub fn nonce(&self) -> &U256 { &self.nonce }

	/// Check if account has zero nonce, balance.
	pub fn is_null(&self) -> bool {
		self.balance.is_zero() &&
		self.nonce.is_zero()
	}

	/// Check if account is basic (Has no code).
	pub fn is_basic(&self) -> bool {
		true
	}

	/// Increment the nonce of the account by one.
	pub fn inc_nonce(&mut self) {
		self.nonce = self.nonce + U256::from(1u8);
	}

	/// Increase account balance.
	pub fn add_balance(&mut self, x: &U256) {
		self.balance = self.balance + *x;
	}

	/// Decrease account balance.
	/// Panics if balance is less than `x`
	pub fn sub_balance(&mut self, x: &U256) {
		assert!(self.balance >= *x);
		self.balance = self.balance - *x;
	}

	/// Export to RLP.
	pub fn rlp(&self) -> Bytes {
		let mut stream = RlpStream::new_list(2);
		stream.append(&self.nonce);
		stream.append(&self.balance);
		stream.out()
	}

	/// Clone basic account data
	pub fn clone_basic(&self) -> Account {
		Account {
			balance: self.balance.clone(),
			nonce: self.nonce.clone(),
		}
	}

	/// Replace self with the data from other account.
	/// Basic account data and all modifications are overwritten
	/// with new values.
	pub fn overwrite_with(&mut self, other: Account) {
		self.balance = other.balance;
		self.nonce = other.nonce;
	}
}

impl fmt::Debug for Account {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_struct("Account")
			.field("balance", &self.balance)
			.field("nonce", &self.nonce)
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use rlp_compress::{compress, decompress, snapshot_swapper};
	use ethereum_types::{H256, Address};
	use memorydb::MemoryDB;
	use cbytes::Bytes;
	use super::*;
	use account_db::*;

	#[test]
	fn account_compress() {
		let raw = Account::new_basic(2.into(), 4.into()).rlp();
		let compact_vec = compress(&raw, snapshot_swapper());
		assert!(raw.len() > compact_vec.len());
		let again_raw = decompress(&compact_vec, snapshot_swapper());
		assert_eq!(raw, again_raw.into_vec());
    }

	#[test]
	fn rlpio() {
		let a = Account::new(69u8.into(), 0u8.into());
		let b = Account::from_rlp(&a.rlp());
		assert_eq!(a.balance(), b.balance());
		assert_eq!(a.nonce(), b.nonce());
	}

	#[test]
	fn new_account() {
		let a = Account::new(69u8.into(), 0u8.into());
		assert_eq!(a.rlp().to_hex(), "f8448045a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
		assert_eq!(*a.balance(), 69u8.into());
		assert_eq!(*a.nonce(), 0u8.into());
	}

	#[test]
	fn create_account() {
		let a = Account::new(69u8.into(), 0u8.into());
		assert_eq!(a.rlp().to_hex(), "f8448045a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
	}

}
