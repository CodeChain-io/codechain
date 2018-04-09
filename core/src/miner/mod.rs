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

mod local_transactions;
mod transaction_queue;

use ctypes::Address;

use super::client::MiningBlockChainClient;
use super::error::Error;
use super::state::StateInfo;
use super::transaction::{SignedTransaction, UnverifiedTransaction};
use super::types::BlockNumber;

/// Miner client API
pub trait MinerService: Send + Sync {
    /// Type representing chain state
    type State: StateInfo + 'static;

    /// Returns miner's status.
    fn status(&self) -> MinerStatus;

    /// Get the author that we will seal blocks as.
    fn author(&self) -> Address;

    /// Set the author that we will seal blocks as.
    fn set_author(&self, author: Address);

    /// Get current transactions limit in queue.
    fn transactions_limit(&self) -> usize;

    /// Set maximal number of transactions kept in the queue (both current and future).
    fn set_transactions_limit(&self, limit: usize);

    /// Imports transactions to transaction queue.
    fn import_external_transactions<C: MiningBlockChainClient>(
        &self,
        client: &C,
        transactions: Vec<UnverifiedTransaction>,
    ) -> Vec<Result<TransactionImportResult, Error>>;

    /// Imports own (node owner) transaction to queue.
    fn import_own_transaction<C: MiningBlockChainClient>(
        &self,
        chain: &C,
        transaction: SignedTransaction,
    ) -> Result<TransactionImportResult, Error>;

    /// Get a list of all pending transactions in the queue.
    fn pending_transactions(&self) -> Vec<SignedTransaction>;


    /// Get a list of all transactions that can go into the given block.
    fn ready_transactions(&self, best_block: BlockNumber, best_block_timestamp: u64) -> Vec<SignedTransaction>;

    /// Get a list of all future transactions.
    fn future_transactions(&self) -> Vec<SignedTransaction>;
}

/// Mining status
#[derive(Debug)]
pub struct MinerStatus {
    /// Number of transactions in queue with state `pending` (ready to be included in block)
    pub transactions_in_pending_queue: usize,
    /// Number of transactions in queue with state `future` (not yet ready to be included in block)
    pub transactions_in_future_queue: usize,
    /// Number of transactions included in currently mined block
    pub transactions_in_pending_block: usize,
}

/// Represents the result of importing transaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionImportResult {
    /// Transaction was imported to current queue.
    Current,
    /// Transaction was imported to future queue.
    Future,
}
