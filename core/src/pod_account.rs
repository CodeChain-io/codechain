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

use cbytes::Bytes;
use cjson;
use ctypes::U256;
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
}

impl PodAccount {
    /// Convert Account to a PodAccount.
    /// NOTE: This will silently fail unless the account is fully cached.
    pub fn from_account(acc: &Account) -> PodAccount {
        PodAccount {
            balance: *acc.balance(),
            nonce: *acc.nonce(),
        }
    }

    /// Returns the RLP for this account.
    pub fn rlp(&self) -> Bytes {
        let mut stream = RlpStream::new_list(2);
        stream.append(&self.nonce);
        stream.append(&self.balance);
        stream.out()
    }
}

impl From<cjson::spec::Account> for PodAccount {
    fn from(a: cjson::spec::Account) -> Self {
        PodAccount {
            balance: a.balance.map_or_else(U256::zero, Into::into),
            nonce: a.nonce.map_or_else(U256::zero, Into::into),
        }
    }
}

impl fmt::Display for PodAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(bal={}; nonce={})", self.balance, self.nonce,)
    }
}
