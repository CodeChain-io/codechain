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

use ccore::{EngineInfo, MinerService, MiningBlockChainClient, SignedTransaction};
use cjson::bytes::Bytes;
use primitives::H256;
use rlp::UntrustedRlp;

use jsonrpc_core::Result;

use super::super::errors;
use super::super::traits::Mempool;
use super::super::types::PendingTransactions;

pub struct MempoolClient<C, M> {
    client: Arc<C>,
    miner: Arc<M>,
}

impl<C, M> MempoolClient<C, M> {
    pub fn new(client: Arc<C>, miner: Arc<M>) -> Self {
        MempoolClient {
            client,
            miner,
        }
    }
}

impl<C, M> Mempool for MempoolClient<C, M>
where
    C: MiningBlockChainClient + EngineInfo + 'static,
    M: MinerService + 'static,
{
    fn send_signed_transaction(&self, raw: Bytes) -> Result<H256> {
        UntrustedRlp::new(&raw.into_vec())
            .as_val()
            .map_err(|e| errors::rlp(&e))
            .and_then(|tx| SignedTransaction::try_new(tx).map_err(errors::transaction_core))
            .and_then(|signed| {
                let hash = signed.hash();
                self.miner.import_own_transaction(&*self.client, signed).map_err(errors::transaction_core).map(|_| hash)
            })
            .map(Into::into)
    }

    fn get_transaction_results_by_tracker(&self, tracker: H256) -> Result<Vec<bool>> {
        Ok(self
            .client
            .error_hints_by_tracker(&tracker)
            .into_iter()
            .map(|(_hash, error_hint)| error_hint.is_none())
            .collect())
    }

    fn get_error_hint(&self, transaction_hash: H256) -> Result<Option<String>> {
        Ok(self.client.error_hint(&transaction_hash))
    }

    fn get_pending_transactions(&self, from: Option<u64>, to: Option<u64>) -> Result<PendingTransactions> {
        Ok(self.client.ready_transactions(from.unwrap_or(0)..to.unwrap_or(::std::u64::MAX)).into())
    }

    fn get_pending_transactions_count(&self, from: Option<u64>, to: Option<u64>) -> Result<usize> {
        Ok(self.client.count_pending_transactions(from.unwrap_or(0)..to.unwrap_or(::std::u64::MAX)))
    }
}
