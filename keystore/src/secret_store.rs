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

use std::path::PathBuf;

use ckey::{Address, Password, Secret};

use crate::json::{OpaqueKeyFile, Uuid};
use crate::{DecryptedAccount, Error};


/// Simple Secret Store API
pub trait SimpleSecretStore: Send + Sync {
    /// Inserts new accounts to the store with given password.
    fn insert_account(&self, secret: Secret, password: &Password) -> Result<Address, Error>;
    /// Returns all accounts in this secret store.
    fn accounts(&self) -> Result<Vec<Address>, Error>;
    /// Check existance of account
    fn has_account(&self, account: &Address) -> Result<bool, Error>;
    /// Entirely removes account from the store and underlying storage.
    fn remove_account(&self, account: &Address) -> Result<(), Error>;
    /// Changes accounts password.
    fn change_password(&self, account: &Address, old_password: &Password, new_password: &Password)
        -> Result<(), Error>;
    /// Exports key details for account.
    fn export_account(&self, account: &Address, password: &Password) -> Result<OpaqueKeyFile, Error>;
    /// Returns a raw opaque Account that can be later used to sign a message.
    fn decrypt_account(&self, account: &Address, password: &Password) -> Result<DecryptedAccount, Error>;
}

/// Secret Store API
pub trait SecretStore: SimpleSecretStore {
    /// Imports existing JSON wallet
    fn import_wallet(&self, json: &[u8], password: &Password, gen_id: bool) -> Result<Address, Error>;

    /// Checks if password matches given account.
    fn test_password(&self, account: &Address, password: &Password) -> Result<bool, Error>;

    /// Copies account between stores.
    fn copy_account(
        &self,
        new_store: &SimpleSecretStore,
        account: &Address,
        password: &Password,
        new_password: &Password,
    ) -> Result<(), Error>;

    /// Returns uuid of an account.
    fn uuid(&self, account: &Address) -> Result<Uuid, Error>;
    /// Returns account's metadata.
    fn meta(&self, account: &Address) -> Result<String, Error>;

    /// Modifies account name.
    fn set_meta(&self, account: &Address, meta: String) -> Result<(), Error>;

    /// Returns local path of the store.
    fn local_path(&self) -> PathBuf;
}
