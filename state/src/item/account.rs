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

use ckey::{self, Public};
use primitives::U256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::cache::CacheableItem;

/// Single account in the system.
// Don't forget to sync the field list with PodAccount.
#[derive(Clone)]
pub struct Account {
    // Balance of the account.
    balance: U256,
    // Nonce of the account.
    nonce: U256,
    // Regular key of the account.
    regular_key: Option<Public>,
}

impl Account {
    pub fn new(balance: U256, nonce: U256) -> Account {
        Account {
            balance,
            nonce,
            regular_key: None,
        }
    }

    pub fn new_with_key(balance: U256, nonce: U256, regular_key: Option<Public>) -> Self {
        Self {
            balance,
            nonce,
            regular_key,
        }
    }

    /// return the balance associated with this account.
    pub fn balance(&self) -> &U256 {
        &self.balance
    }

    /// return the nonce associated with this account.
    pub fn nonce(&self) -> &U256 {
        &self.nonce
    }

    /// return the regular key associated with this account.
    pub fn regular_key(&self) -> Option<Public> {
        self.regular_key
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

    /// Set the regular key of the account.
    /// Overwrite if the key already exists.
    pub fn set_regular_key(&mut self, key: &Public) {
        self.regular_key = Some(*key);
    }

    /// Remove the regular key of the account.
    pub fn remove_regular_key(&mut self) {
        self.regular_key = None;
    }
}

impl CacheableItem for Account {
    type Address = ckey::Address;

    /// Check if account has zero nonce, balance.
    fn is_null(&self) -> bool {
        self.balance.is_zero() && self.nonce.is_zero()
    }
}

const PREFIX: u8 = super::ADDRESS_PREFIX;

impl Encodable for Account {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&PREFIX);
        s.append(&self.balance);
        s.append(&self.nonce);
        s.append(&self.regular_key);
    }
}

impl Decodable for Account {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for account", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            balance: rlp.val_at(1)?,
            nonce: rlp.val_at(2)?,
            regular_key: rlp.val_at(3)?,
        })
    }
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Account").field("balance", &self.balance).field("nonce", &self.nonce).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hex::ToHex;

    #[test]
    fn rlpio() {
        let a = Account::new(69u8.into(), 0u8.into());
        let b = ::rlp::decode::<Account>(&a.rlp_bytes());
        assert_eq!(a.balance(), b.balance());
        assert_eq!(a.nonce(), b.nonce());

        let mut a = Account::new(69u8.into(), 0u8.into());
        a.set_regular_key(&Public::default());
        let b = ::rlp::decode::<Account>(&a.rlp_bytes());
        assert_eq!(a.balance(), b.balance());
        assert_eq!(a.nonce(), b.nonce());
        assert_eq!(a.regular_key(), b.regular_key());
    }

    #[test]
    fn new_account() {
        let a = Account::new(69u8.into(), 0u8.into());
        assert_eq!(a.rlp_bytes().to_hex(), "c4434580c0");
        assert_eq!(*a.balance(), 69u8.into());
        assert_eq!(*a.nonce(), 0u8.into());
        assert_eq!(a.regular_key(), None);
    }

    #[test]
    fn balance() {
        let mut a = Account::new(69u8.into(), 0u8.into());
        a.add_balance(&1u8.into());
        assert_eq!(*a.balance(), 70u8.into());
        a.sub_balance(&2u8.into());
        assert_eq!(*a.balance(), 68u8.into());
    }

    #[test]
    #[should_panic]
    fn negative_balance() {
        let mut a = Account::new(69u8.into(), 0u8.into());
        a.sub_balance(&70u8.into());
    }

    #[test]
    fn nonce() {
        let mut a = Account::new(69u8.into(), 0u8.into());
        a.inc_nonce();
        assert_eq!(*a.nonce(), 1u8.into());
        a.inc_nonce();
        assert_eq!(*a.nonce(), 2u8.into());
    }

    #[test]
    fn overwrite() {
        let mut a0 = Account::new(69u8.into(), 0u8.into());
        let a = &mut a0;
        let mut b = Account::new(79u8.into(), 1u8.into());
        b.set_regular_key(&Public::default());
        *a = b;
        assert_eq!(*a.balance(), 79u8.into());
        assert_eq!(*a.nonce(), 1u8.into());
        assert_eq!(a.regular_key(), Some(Public::default()));
    }

    #[test]
    fn is_null() {
        let mut a = Account::new(69u8.into(), 0u8.into());
        assert!(!a.is_null());
        a.sub_balance(&69u8.into());
        assert!(a.is_null());
        a.inc_nonce();
        assert!(!a.is_null());
    }

}
