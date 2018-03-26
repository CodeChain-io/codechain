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
use cbytes::{Bytes, ToPretty};
use rlp::*;

/// Single account in the system.
#[derive(Clone, RlpEncodable, RlpDecodable)]
pub struct Account {
    // Balance of the account.
    balance: U256,
    // Nonce of the account.
    nonce: U256,
}

impl Account {
    pub fn new(balance: U256, nonce: U256) -> Account {
        Account {
            balance: balance,
            nonce: nonce,
        }
    }

    /// Create a new account from RLP.
    pub fn from_rlp(rlp: &[u8]) -> Account {
        ::rlp::decode(rlp)
    }

    /// Export to RLP.
    pub fn rlp(&self) -> Bytes {
        ::rlp::encode(self).into_vec()
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
    use ctypes::{H256, Address};
    use memorydb::MemoryDB;
    use cbytes::Bytes;
    use super::*;

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
        assert_eq!(a.rlp().to_hex(), "c24580");
        assert_eq!(*a.balance(), 69u8.into());
        assert_eq!(*a.nonce(), 0u8.into());
    }

    // FIXME: Add tests for add_balance/sub_balance/inc_nonce

}
