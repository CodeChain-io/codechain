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
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::CacheableItem;

/// Single account in the system.
// Don't forget to sync the field list with PodAccount.
#[derive(Clone)]
pub struct Account {
    // Balance of the account.
    balance: u64,
    // Seq of the account.
    seq: u64,
    // Regular key of the account.
    regular_key: Option<Public>,
}

impl Account {
    pub fn new(balance: u64, seq: u64) -> Account {
        Account {
            balance,
            seq,
            regular_key: None,
        }
    }

    pub fn new_with_key(balance: u64, seq: u64, regular_key: Option<Public>) -> Self {
        Self {
            balance,
            seq,
            regular_key,
        }
    }

    pub fn is_active(&self) -> bool {
        !self.is_null()
    }

    /// return the balance associated with this account.
    pub fn balance(&self) -> u64 {
        self.balance
    }

    /// return the seq associated with this account.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// return the regular key associated with this account.
    pub fn regular_key(&self) -> Option<Public> {
        self.regular_key
    }

    /// Increment the seq of the account by one.
    pub fn inc_seq(&mut self) {
        self.seq += 1;
    }

    /// Increase account balance.
    pub fn add_balance(&mut self, x: u64) {
        self.balance += x;
    }

    /// Decrease account balance.
    /// Panics if balance is less than `x`
    pub fn sub_balance(&mut self, x: u64) {
        assert!(self.balance >= x);
        self.balance -= x;
    }

    #[cfg(test)]
    pub fn set_balance(&mut self, x: u64) {
        self.balance = x;
    }

    #[cfg(test)]
    pub fn set_seq(&mut self, x: u64) {
        self.seq = x;
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

impl Default for Account {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl CacheableItem for Account {
    type Address = ckey::Address;

    /// Check if account has zero seq, balance.
    fn is_null(&self) -> bool {
        self.balance == 0 && self.seq == 0
    }
}

const PREFIX: u8 = super::ADDRESS_PREFIX;

impl Encodable for Account {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(4);
        s.append(&PREFIX);
        s.append(&self.balance);
        s.append(&self.seq);
        s.append(&self.regular_key);
    }
}

impl Decodable for Account {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let item_count = rlp.item_count()?;
        if item_count != 4 {
            return Err(DecoderError::RlpInvalidLength {
                expected: 4,
                got: item_count,
            })
        }

        let prefix = rlp.val_at::<u8>(0)?;
        if PREFIX != prefix {
            cdebug!(STATE, "{} is not an expected prefix for account", prefix);
            return Err(DecoderError::Custom("Unexpected prefix"))
        }
        Ok(Self {
            balance: rlp.val_at(1)?,
            seq: rlp.val_at(2)?,
            regular_key: rlp.val_at(3)?,
        })
    }
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Account").field("balance", &self.balance).field("seq", &self.seq).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hex::ToHex;

    #[test]
    fn rlpio() {
        let a = Account::new(69, 0);
        let b = ::rlp::decode::<Account>(&a.rlp_bytes());
        assert_eq!(a.balance(), b.balance());
        assert_eq!(a.seq(), b.seq());

        let mut a = Account::new(69, 0);
        a.set_regular_key(&Public::default());
        let b = ::rlp::decode::<Account>(&a.rlp_bytes());
        assert_eq!(a.balance(), b.balance());
        assert_eq!(a.seq(), b.seq());
        assert_eq!(a.regular_key(), b.regular_key());
    }

    #[test]
    fn new_account() {
        let a = Account::new(69, 0);
        assert_eq!(a.rlp_bytes().to_hex(), "c4434580c0");
        assert_eq!(69, a.balance());
        assert_eq!(0, a.seq());
        assert_eq!(a.regular_key(), None);
    }

    #[test]
    fn balance() {
        let mut a = Account::new(69, 0);
        a.add_balance(1);
        assert_eq!(70, a.balance());
        a.sub_balance(2);
        assert_eq!(68, a.balance());
    }

    #[test]
    #[should_panic]
    fn negative_balance() {
        let mut a = Account::new(69, 0);
        a.sub_balance(70);
    }

    #[test]
    fn seq() {
        let mut a = Account::new(69, 0);
        a.inc_seq();
        assert_eq!(1, a.seq());
        a.inc_seq();
        assert_eq!(2, a.seq())
    }

    #[test]
    fn overwrite() {
        let mut a0 = Account::new(69, 0);
        let a = &mut a0;
        let mut b = Account::new(79, 1);
        b.set_regular_key(&Public::default());
        *a = b;
        assert_eq!(79, a.balance());
        assert_eq!(1, a.seq());
        assert_eq!(a.regular_key(), Some(Public::default()));
    }

    #[test]
    fn is_null() {
        let mut a = Account::new(69, 0);
        assert!(!a.is_null());
        a.sub_balance(69);
        assert!(a.is_null());
        a.inc_seq();
        assert!(!a.is_null());
    }

}
