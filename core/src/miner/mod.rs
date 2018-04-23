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
mod miner;
mod transaction_queue;

use cbytes::Bytes;
use ctypes::{Address, H256, U256};

pub use self::miner::{Miner, MinerOptions};
use super::account_provider::SignError;
use super::client::{AccountData, BlockChain, BlockProducer, MiningBlockChainClient, SealedBlockImporter};
use super::error::Error;
use super::state::StateInfo;
use super::transaction::{SignedTransaction, UnverifiedTransaction};

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

    /// Get the extra_data that we will seal blocks with.
    fn extra_data(&self) -> Bytes;

    /// Set the extra_data that we will seal blocks with.
    fn set_extra_data(&self, extra_data: Bytes);

    /// Set info necessary to sign consensus messages.
    fn set_engine_signer(&self, address: Address) -> Result<(), SignError>;

    /// Get current minimal fee for transactions accepted to queue.
    fn minimal_fee(&self) -> U256;

    /// Set minimal fee of transaction to be accepted for mining.
    fn set_minimal_fee(&self, min_fee: U256);

    /// Get current transactions limit in queue.
    fn transactions_limit(&self) -> usize;

    /// Set maximal number of transactions kept in the queue (both current and future).
    fn set_transactions_limit(&self, limit: usize);

    /// Called when blocks are imported to chain, updates transactions queue.
    fn chain_new_blocks<C>(&self, chain: &C, imported: &[H256], invalid: &[H256], enacted: &[H256], retracted: &[H256])
    where
        C: AccountData + BlockChain + BlockProducer + SealedBlockImporter;

    /// New chain head event. Restart mining operation.
    fn update_sealing<C>(&self, chain: &C)
    where
        C: AccountData + BlockChain + BlockProducer + SealedBlockImporter;

    /// Submit `seal` as a valid solution for the header of `pow_hash`.
    /// Will check the seal, but not actually insert the block into the chain.
    fn submit_seal<C: SealedBlockImporter>(&self, chain: &C, pow_hash: H256, seal: Vec<Bytes>) -> Result<(), Error>;

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
    fn ready_transactions(&self) -> Vec<SignedTransaction>;

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
