// Copyright 2018-2019 Kodebox, Inc.
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

use ccore::{AccountProvider, MinerService, MiningBlockChainClient, RegularKey, RegularKeyOwner, Seq};
use ckey::{NetworkId, Password, PlatformAddress, Signature};
use ctypes::transaction::IncompleteTransaction;
use jsonrpc_core::Result;
use parking_lot::Mutex;
use primitives::H256;

use super::super::errors::{self, account_provider};
use super::super::traits::Account;
use super::super::types::{SendTransactionResult, UnsignedTransaction};

pub struct AccountClient<C, M>
where
    C: MiningBlockChainClient + Seq + RegularKey + RegularKeyOwner,
    M: MinerService, {
    account_provider: Arc<AccountProvider>,
    network_id: NetworkId,
    client: Arc<C>,
    miner: Arc<M>,
}

impl<C, M> AccountClient<C, M>
where
    C: MiningBlockChainClient + Seq + RegularKey + RegularKeyOwner,
    M: MinerService,
{
    pub fn new(account_provider: Arc<AccountProvider>, client: Arc<C>, miner: Arc<M>, network_id: NetworkId) -> Self {
        AccountClient {
            account_provider,
            network_id,
            client,
            miner,
        }
    }
}

impl<C, M> Account for AccountClient<C, M>
where
    C: MiningBlockChainClient + Seq + RegularKey + RegularKeyOwner + 'static,
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
        self.account_provider
            .get_account(&address, passphrase.as_ref())
            .and_then(|account| Ok(account.sign(&message_digest)?))
            .map_err(account_provider)
    }

    fn send_transaction(
        &self,
        tx: UnsignedTransaction,
        platform_address: PlatformAddress,
        passphrase: Option<Password>,
    ) -> Result<SendTransactionResult> {
        lazy_static! {
            static ref LOCK: Mutex<()> = Mutex::new(());
        }
        let _guard = LOCK.lock();
        let (tx, seq): (IncompleteTransaction, Option<u64>) = ::std::result::Result::from(tx)?;

        let (hash, seq) = self
            .miner
            .import_incomplete_transaction(
                self.client.as_ref(),
                self.account_provider.as_ref(),
                tx,
                platform_address,
                passphrase,
                seq,
            )
            .map_err(errors::core)?;

        Ok(SendTransactionResult {
            hash,
            seq,
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
