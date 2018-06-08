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

use cjson;
use ctypes::{Bytes, Public, U256};
use rlp::RlpStream;

use super::state::Account;

#[derive(Debug, Clone, PartialEq, Eq)]
/// An account, expressed as Plain-Old-Data (hence the name).
/// Does not have a DB overlay cache, code hash or anything like that.
pub struct PodAccount {
    /// The balance of the account.
    pub balance: U256,
    /// The nonce of the account.
    pub nonce: U256,
    /// Regular key of the account.
    pub regular_key: Option<Public>,
}

impl PodAccount {
    /// Convert Account to a PodAccount.
    /// NOTE: This will silently fail unless the account is fully cached.
    #[allow(dead_code)]
    pub fn from_account(acc: &Account) -> PodAccount {
        PodAccount {
            balance: *acc.balance(),
            nonce: *acc.nonce(),
            regular_key: acc.regular_key(),
        }
    }

    /// Returns the RLP for this account.
    pub fn rlp(&self) -> Bytes {
        // Don't forget to sync the field list with Account.
        let mut stream = RlpStream::new_list(4);
        const PREFIX: u8 = 'C' as u8;
        stream.append(&PREFIX);
        stream.append(&self.balance);
        stream.append(&self.nonce);
        stream.append(&self.regular_key);
        stream.out()
    }
}

impl From<cjson::spec::Account> for PodAccount {
    fn from(a: cjson::spec::Account) -> Self {
        PodAccount {
            balance: a.balance.map_or_else(U256::zero, Into::into),
            nonce: a.nonce.map_or_else(U256::zero, Into::into),
            regular_key: None,
        }
    }
}

impl fmt::Display for PodAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(bal={}; nonce={})", self.balance, self.nonce,)
    }
}
