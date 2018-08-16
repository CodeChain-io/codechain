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

use std::sync::Arc;
use std::time::Duration;

use ccore::AccountProvider;
use ckey::{NetworkId, Password, PlatformAddress, Signature};
use jsonrpc_core::Result;
use primitives::H256;

use super::super::errors::account_provider;
use super::super::traits::Account;

pub struct AccountClient {
    account_provider: Arc<AccountProvider>,
    network_id: NetworkId,
}

impl AccountClient {
    pub fn new(ap: &Arc<AccountProvider>, network_id: NetworkId) -> Self {
        AccountClient {
            account_provider: ap.clone(),
            network_id,
        }
    }
}

impl Account for AccountClient {
    fn get_account_list(&self) -> Result<Vec<PlatformAddress>> {
        self.account_provider
            .get_list()
            .map(|addresses| {
                addresses.into_iter().map(|address| PlatformAddress::create(0, self.network_id, address)).collect()
            })
            .map_err(account_provider)
    }

    fn create_account(&self, passphrase: Option<Password>) -> Result<PlatformAddress> {
        let (address, _) =
            self.account_provider.new_account_and_public(&passphrase.unwrap_or_default()).map_err(account_provider)?;
        Ok(PlatformAddress::create(0, self.network_id, address))
    }

    fn create_account_from_secret(&self, secret: H256, passphrase: Option<Password>) -> Result<PlatformAddress> {
        self.account_provider
            .insert_account(secret.into(), &passphrase.unwrap_or_default())
            .map(|address| PlatformAddress::create(0, self.network_id, address))
            .map_err(account_provider)
    }

    fn remove_account(&self, address: PlatformAddress, passphrase: Option<Password>) -> Result<()> {
        self.account_provider
            .remove_account(address.into_address(), &passphrase.unwrap_or_default())
            .map_err(account_provider)
    }

    fn sign(&self, message_digest: H256, address: PlatformAddress, passphrase: Option<Password>) -> Result<Signature> {
        self.account_provider
            .sign(address.into_address(), Some(passphrase.unwrap_or_default()), message_digest)
            .map(|sig| sig.into())
            .map_err(account_provider)
    }

    fn change_password(&self, address: PlatformAddress, old_password: Password, new_password: Password) -> Result<()> {
        self.account_provider
            .change_password(address.into_address(), &old_password, &new_password)
            .map_err(account_provider)
    }

    fn unlock(&self, address: PlatformAddress, password: Password, duration: Option<u64>) -> Result<()> {
        const DEFAULT_DURATION: u64 = 300;
        match duration {
            Some(0) => self
                .account_provider
                .unlock_account_permanently(address.into_address(), password)
                .map_err(Into::into)
                .map_err(account_provider)?,
            Some(secs) => self
                .account_provider
                .unlock_account_timed(address.into_address(), password, Duration::from_secs(secs))
                .map_err(Into::into)
                .map_err(account_provider)?,
            None => self
                .account_provider
                .unlock_account_timed(address.into_address(), password, Duration::from_secs(DEFAULT_DURATION))
                .map_err(Into::into)
                .map_err(account_provider)?,
        };
        Ok(())
    }
}
