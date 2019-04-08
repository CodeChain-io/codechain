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
use primitives::H256;

use ckey::PlatformAddress;
use jsonrpc_core::Result;

use super::super::types::PendingTransactions;

build_rpc_trait! {
    pub trait Mempool {
        /// Sends signed transaction, returning its hash.
        # [rpc(name = "mempool_sendSignedTransaction")]
        fn send_signed_transaction(&self, Bytes) -> Result<H256>;

        /// Gets transaction results with given transaction tracker.
        # [rpc(name = "mempool_getTransactionResultsByTracker")]
        fn get_transaction_results_by_tracker(&self, H256) -> Result<Vec<bool>>;

        /// Gets a hint to find out why the transaction failed.
        # [rpc(name = "mempool_getErrorHint")]
        fn get_error_hint(&self, H256) -> Result<Option<String>>;

        /// Gets transactions in the current mem pool.
        # [rpc(name = "mempool_getPendingTransactions")]
        fn get_pending_transactions(&self, Option<u64>, Option<u64>) -> Result<PendingTransactions>;

       /// Gets the count of transactions in the current mem pool.
        # [rpc(name = "mempool_getPendingTransactionsCount")]
        fn get_pending_transactions_count(&self, Option<u64>, Option<u64>) -> Result<usize>;

        #[rpc(name = "mempool_getBannedAccounts")]
        fn get_banned_accounts(&self) -> Result<Vec<PlatformAddress>>;

        #[rpc(name = "mempool_unbanAccounts")]
        fn unban_accounts(&self, Vec<PlatformAddress>) -> Result<()>;

        #[rpc(name = "mempool_banAccounts")]
        fn ban_accounts(&self, Vec<PlatformAddress>) -> Result<()>;

        #[rpc(name = "mempool_getImmuneAccounts")]
        fn get_immune_accounts(&self) -> Result<Vec<PlatformAddress>>;

        #[rpc(name = "mempool_registerImmuneAccounts")]
        fn register_immune_accounts(&self, Vec<PlatformAddress>) -> Result<()>;
    }
}
