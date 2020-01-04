// Copyright 2019 Kodebox, Inc.
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

use ccore::{BlockChainClient, EngineInfo, MiningBlockChainClient, SignedTransaction};
use cjson::bytes::Bytes;
use ckey::{Address, PlatformAddress};
use ctypes::{Tracker, TxHash};
use rlp::Rlp;

use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Mempool;
use super::super::types::PendingTransactions;
pub struct MempoolClient<C> {
    client: Arc<C>,
}

impl<C> MempoolClient<C> {
    pub fn new(client: Arc<C>) -> Self {
        MempoolClient {
            client,
        }
    }
}

impl<C> Mempool for MempoolClient<C>
where
    C: BlockChainClient + MiningBlockChainClient + EngineInfo + 'static,
{
    fn send_signed_transaction(&self, raw: Bytes) -> Result<TxHash> {
        Rlp::new(&raw.into_vec())
            .as_val()
            .map_err(|e| errors::rlp(&e))
            .and_then(|tx| SignedTransaction::try_new(tx).map_err(errors::transaction_core))
            .and_then(|signed| {
                let hash = signed.hash();
                match self.client.queue_own_transaction(signed) {
                    Ok(_) => Ok(hash),
                    Err(e) => Err(errors::transaction_core(e)),
                }
            })
            .map(Into::into)
    }

    fn get_transaction_results_by_tracker(&self, tracker: Tracker) -> Result<Vec<bool>> {
        Ok(self
            .client
            .error_hints_by_tracker(&tracker)
            .into_iter()
            .map(|(_hash, error_hint)| error_hint.is_none())
            .collect())
    }

    fn get_error_hint(&self, transaction_hash: TxHash) -> Result<Option<String>> {
        Ok(self.client.error_hint(&transaction_hash))
    }

    fn delete_all_pending_transactions(&self) -> Result<()> {
        self.client.delete_all_pending_transactions();
        Ok(())
    }

    fn get_pending_transactions(
        &self,
        from: Option<u64>,
        to: Option<u64>,
        future_included: Option<bool>,
    ) -> Result<PendingTransactions> {
        if future_included.unwrap_or(false) {
            Ok(self.client.future_ready_transactions(from.unwrap_or(0)..to.unwrap_or(::std::u64::MAX)).into())
        } else {
            Ok(self.client.ready_transactions(from.unwrap_or(0)..to.unwrap_or(::std::u64::MAX)).into())
        }
    }

    fn get_pending_transactions_count(
        &self,
        from: Option<u64>,
        to: Option<u64>,
        future_included: Option<bool>,
    ) -> Result<usize> {
        if future_included.unwrap_or(false) {
            Ok(self.client.future_included_count_pending_transactions(from.unwrap_or(0)..to.unwrap_or(::std::u64::MAX)))
        } else {
            Ok(self.client.count_pending_transactions(from.unwrap_or(0)..to.unwrap_or(::std::u64::MAX)))
        }
    }


    fn get_banned_accounts(&self) -> Result<Vec<PlatformAddress>> {
        let malicious_user_vec = self.client.get_malicious_users();
        let network_id = self.client.network_id();
        Ok(malicious_user_vec.into_iter().map(|address| PlatformAddress::new_v1(network_id, address)).collect())
    }

    fn unban_accounts(&self, prisoner_list: Vec<PlatformAddress>) -> Result<()> {
        let prisoner_vec: Vec<Address> = prisoner_list.into_iter().map(PlatformAddress::into_address).collect();

        self.client.release_malicious_users(prisoner_vec);
        Ok(())
    }

    fn ban_accounts(&self, prisoner_list: Vec<PlatformAddress>) -> Result<()> {
        let prisoner_vec: Vec<Address> = prisoner_list.into_iter().map(PlatformAddress::into_address).collect();

        self.client.imprison_malicious_users(prisoner_vec);
        Ok(())
    }

    fn get_immune_accounts(&self) -> Result<Vec<PlatformAddress>> {
        let immune_user_vec = self.client.get_immune_users();
        let network_id = self.client.network_id();
        Ok(immune_user_vec.into_iter().map(|address| PlatformAddress::new_v1(network_id, address)).collect())
    }

    fn register_immune_accounts(&self, immune_user_list: Vec<PlatformAddress>) -> Result<()> {
        let immune_user_vec: Vec<Address> = immune_user_list.into_iter().map(PlatformAddress::into_address).collect();

        self.client.register_immune_users(immune_user_vec);
        Ok(())
    }
}
