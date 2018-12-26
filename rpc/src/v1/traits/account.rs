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

use ckey::{Password, PlatformAddress, Signature};
use jsonrpc_core::Result;
use primitives::H256;

use super::super::types::{SendTransactionResult, UnsignedTransaction};

build_rpc_trait! {
    pub trait Account {
        /// Gets a list of accounts
        # [rpc(name = "account_getList")]
        fn get_account_list(&self) -> Result<Vec<PlatformAddress>>;

        /// Creates a new account
        # [rpc(name = "account_create")]
        fn create_account(&self, Option<Password>) -> Result<PlatformAddress>;

        /// Imports a private key
        # [rpc(name = "account_importRaw")]
        fn create_account_from_secret(&self, H256, Option<Password>) -> Result<PlatformAddress>;

        /// Unlocks the specified account for use.
        # [rpc(name = "account_unlock")]
        fn unlock(&self, PlatformAddress, Password, Option<u64>) -> Result<()>;

        /// Calculates the account's signature for a given message
        # [rpc(name = "account_sign")]
        fn sign(&self, H256, PlatformAddress, Option<Password>) -> Result<Signature>;

        /// Sends a transaction with a signature of the account
        # [rpc(name = "account_sendTransaction")]
        fn send_transaction(&self, UnsignedTransaction, PlatformAddress, Option<Password>) -> Result<SendTransactionResult>;

        /// Changes the account's password
        # [rpc(name = "account_changePassword")]
        fn change_password(&self, PlatformAddress, Password, Password) -> Result<()>;
    }
}
