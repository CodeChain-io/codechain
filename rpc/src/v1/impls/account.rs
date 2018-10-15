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

use ccore::{
    AccountProvider, AccountProviderError, MinerService, MiningBlockChainClient, Nonce, RegularKey, RegularKeyOwner,
};
use ckey::{NetworkId, Password, PlatformAddress, Signature};
use ctypes::parcel::IncompleteParcel;
use jsonrpc_core::Result;
use parking_lot::Mutex;
use primitives::{H256, U256};

use super::super::errors::{self, account_provider};
use super::super::traits::Account;
use super::super::types::{SendParcelResult, UnsignedParcel};

pub struct AccountClient<C, M>
where
    C: MiningBlockChainClient + Nonce + RegularKey + RegularKeyOwner,
    M: MinerService, {
    account_provider: Arc<AccountProvider>,
    network_id: NetworkId,
    client: Arc<C>,
    miner: Arc<M>,
}

impl<C, M> AccountClient<C, M>
where
    C: MiningBlockChainClient + Nonce + RegularKey + RegularKeyOwner,
    M: MinerService,
{
    pub fn new(ap: &Arc<AccountProvider>, client: Arc<C>, miner: Arc<M>, network_id: NetworkId) -> Self {
        AccountClient {
            account_provider: ap.clone(),
            network_id,
            client,
            miner,
        }
    }
}

impl<C, M> Account for AccountClient<C, M>
where
    C: MiningBlockChainClient + Nonce + RegularKey + RegularKeyOwner + 'static,
    M: MinerService + 'static,
{
    fn get_account_list(&self) -> Result<Vec<PlatformAddress>> {
        self.account_provider
            .get_list()
            .map(|addresses| {
                addresses.into_iter().map(|address| PlatformAddress::new_v1(self.network_id, address)).collect()
            })
            .map_err(account_provider)
    }

    fn create_account(&self, passphrase: Option<Password>) -> Result<PlatformAddress> {
        let (address, _) =
            self.account_provider.new_account_and_public(&passphrase.unwrap_or_default()).map_err(account_provider)?;
        Ok(PlatformAddress::new_v1(self.network_id, address))
    }

    fn create_account_from_secret(&self, secret: H256, passphrase: Option<Password>) -> Result<PlatformAddress> {
        self.account_provider
            .insert_account(secret.into(), &passphrase.unwrap_or_default())
            .map(|address| PlatformAddress::new_v1(self.network_id, address))
            .map_err(account_provider)
    }

    fn sign(&self, message_digest: H256, address: PlatformAddress, passphrase: Option<Password>) -> Result<Signature> {
        let address = address.try_into_address().map_err(errors::core)?;
        self.account_provider.sign(address, passphrase, message_digest).map(|sig| sig.into()).map_err(account_provider)
    }

    fn send_parcel(
        &self,
        parcel: UnsignedParcel,
        platform_address: PlatformAddress,
        passphrase: Option<Password>,
    ) -> Result<SendParcelResult> {
        lazy_static! {
            static ref LOCK: Mutex<()> = Mutex::new(());
        }
        let _guard = LOCK.lock();
        let (parcel, nonce): (IncompleteParcel, Option<U256>) =
            ::std::result::Result::from(parcel).map_err(AccountProviderError::KeyError).map_err(account_provider)?;

        let (hash, nonce) = self
            .miner
            .import_incomplete_parcel(
                self.client.as_ref(),
                self.account_provider.as_ref(),
                parcel,
                platform_address,
                passphrase,
                nonce,
            )
            .map_err(errors::core)?;

        Ok(SendParcelResult {
            hash,
            nonce,
        })
    }

    fn change_password(&self, address: PlatformAddress, old_password: Password, new_password: Password) -> Result<()> {
        self.account_provider
            .change_password(address.into_address(), &old_password, &new_password)
            .map_err(account_provider)
    }

    fn unlock(&self, address: PlatformAddress, password: Password, duration: Option<u64>) -> Result<()> {
        const DEFAULT_DURATION: u64 = 300;
        match duration {
            Some(0) => {
                let address = address.try_into_address().map_err(errors::core)?;
                self.account_provider
                    .unlock_account_permanently(address, password)
                    .map_err(Into::into)
                    .map_err(account_provider)?
            }
            Some(secs) => {
                let address = address.try_into_address().map_err(errors::core)?;
                self.account_provider
                    .unlock_account_timed(address, password, Duration::from_secs(secs))
                    .map_err(Into::into)
                    .map_err(account_provider)?
            }
            None => {
                let address = address.try_into_address().map_err(errors::core)?;
                self.account_provider
                    .unlock_account_timed(address, password, Duration::from_secs(DEFAULT_DURATION))
                    .map_err(Into::into)
                    .map_err(account_provider)?
            }
        };
        Ok(())
    }
}
