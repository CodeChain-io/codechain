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

use ctypes::{Address, Public, U256};
use error::Error;
use trie;

use super::TopBackend;

pub trait TopState<B>
where
    B: TopBackend, {
    /// Remove an existing account.
    fn kill_account(&mut self, account: &Address);

    fn account_exists(&self, a: &Address) -> trie::Result<bool>;

    fn account_exists_and_not_null(&self, a: &Address) -> trie::Result<bool>;
    fn account_exists_and_has_nonce(&self, a: &Address) -> trie::Result<bool>;

    /// Add `incr` to the balance of account `a`.
    fn add_balance(&mut self, a: &Address, incr: &U256) -> trie::Result<()>;
    /// Subtract `decr` from the balance of account `a`.
    fn sub_balance(&mut self, a: &Address, decr: &U256) -> trie::Result<()>;
    /// Subtracts `by` from the balance of `from` and adds it to that of `to`.
    fn transfer_balance(&mut self, from: &Address, to: &Address, by: &U256) -> Result<(), Error>;

    /// Increment the nonce of account `a` by 1.
    fn inc_nonce(&mut self, a: &Address) -> trie::Result<()>;

    /// Set the regular key of account `a`
    fn set_regular_key(&mut self, a: &Address, key: &Public) -> Result<(), Error>;
}
