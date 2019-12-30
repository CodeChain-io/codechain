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

use cjson::bytes::Bytes;
use ckey::PlatformAddress;
use ctypes::{Tracker, TxHash};
use jsonrpc_core::Result;

use super::super::types::PendingTransactions;

#[rpc(server)]
pub trait Mempool {
    /// Sends signed transaction, returning its hash.
    #[rpc(name = "mempool_sendSignedTransaction")]
    fn send_signed_transaction(&self, raw: Bytes) -> Result<TxHash>;

    /// Gets transaction results with given transaction tracker.
    #[rpc(name = "mempool_getTransactionResultsByTracker")]
    fn get_transaction_results_by_tracker(&self, tracker: Tracker) -> Result<Vec<bool>>;

    /// Gets a hint to find out why the transaction failed.
    #[rpc(name = "mempool_getErrorHint")]
    fn get_error_hint(&self, transaction_hash: TxHash) -> Result<Option<String>>;

    /// Deletes all pending transactions in the mem pool, including future queue.
    #[rpc(name = "mempool_deleteAllPendingTransactions")]
    fn delete_all_pending_transactions(&self) -> Result<()>;

    /// Gets transactions in the current mem pool. future_included is set to check whether append future queue or not.
    #[rpc(name = "mempool_getPendingTransactions")]
    fn get_pending_transactions(
        &self,
        from: Option<u64>,
        to: Option<u64>,
        future_included: Option<bool>,
    ) -> Result<PendingTransactions>;

    /// Gets the count of transactions in the current mem pool.
    #[rpc(name = "mempool_getPendingTransactionsCount")]
    fn get_pending_transactions_count(
        &self,
        from: Option<u64>,
        to: Option<u64>,
        future_included: Option<bool>,
    ) -> Result<usize>;

    #[rpc(name = "mempool_getBannedAccounts")]
    fn get_banned_accounts(&self) -> Result<Vec<PlatformAddress>>;

    #[rpc(name = "mempool_unbanAccounts")]
    fn unban_accounts(&self, prisoner_list: Vec<PlatformAddress>) -> Result<()>;

    #[rpc(name = "mempool_banAccounts")]
    fn ban_accounts(&self, prisoner_list: Vec<PlatformAddress>) -> Result<()>;

    #[rpc(name = "mempool_getImmuneAccounts")]
    fn get_immune_accounts(&self) -> Result<Vec<PlatformAddress>>;

    #[rpc(name = "mempool_registerImmuneAccounts")]
    fn register_immune_accounts(&self, immune_user_list: Vec<PlatformAddress>) -> Result<()>;
}
