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

mod mem_pool;
#[cfg_attr(feature = "cargo-clippy", allow(clippy::module_inception))]
mod miner;
mod sealing_queue;
mod stratum;
mod work_notify;

use ckey::{Address, Password, PlatformAddress};
use cstate::{FindActionHandler, TopStateView};
use ctypes::transaction::IncompleteTransaction;
use cvm::ChainTimeInfo;
use primitives::{Bytes, H256};

pub use self::miner::{AuthoringParams, Miner, MinerOptions};
pub use self::stratum::{Config as StratumConfig, Error as StratumError, Stratum};
use crate::account_provider::{AccountProvider, SignError};
use crate::block::ClosedBlock;
use crate::client::{
    AccountData, BlockChain, BlockProducer, ImportSealedBlock, MiningBlockChainClient, RegularKey, RegularKeyOwner,
    ResealTimer,
};
use crate::consensus::EngineType;
use crate::error::Error;
use crate::transaction::{SignedTransaction, UnverifiedTransaction};
use crate::BlockId;

/// Miner client API
pub trait MinerService: Send + Sync {
    /// Type representing chain state
    type State: TopStateView + 'static;

    /// Returns miner's status.
    fn status(&self) -> MinerStatus;

    /// Get current authoring parameters.
    fn authoring_params(&self) -> AuthoringParams;

    /// Set the author that we will seal blocks as.
    fn set_author(&self, author: Address, password: Option<Password>) -> Result<(), SignError>;

    /// Set the extra_data that we will seal blocks with.
    fn set_extra_data(&self, extra_data: Bytes);

    /// Get current minimal fee for tranasctions accepted to queue.
    fn minimal_fee(&self) -> u64;

    /// Set minimal fee of transactions to be accepted for mining.
    fn set_minimal_fee(&self, min_fee: u64);

    /// Get current transactions limit in queue.
    fn transactions_limit(&self) -> usize;

    /// Set maximal number of transactions kept in the queue (both current and future).
    fn set_transactions_limit(&self, limit: usize);

    /// Called when blocks are imported to chain, updates transactions queue.
    fn chain_new_blocks<C>(&self, chain: &C, imported: &[H256], invalid: &[H256], enacted: &[H256], retracted: &[H256])
    where
        C: AccountData + BlockChain + BlockProducer + ImportSealedBlock + RegularKeyOwner + ResealTimer;

    /// PoW chain - can produce work package
    fn can_produce_work_package(&self) -> bool;

    /// Get the type of consensus engine.
    fn engine_type(&self) -> EngineType;

    /// Returns true if we had to prepare new pending block.
    fn prepare_work_sealing<C>(&self, &C) -> bool
    where
        C: AccountData + BlockChain + BlockProducer + RegularKeyOwner + ChainTimeInfo + FindActionHandler;

    /// New chain head event. Restart mining operation.
    fn update_sealing<C>(&self, chain: &C, parent_block: BlockId, allow_empty_block: bool)
    where
        C: AccountData
            + BlockChain
            + BlockProducer
            + ImportSealedBlock
            + RegularKeyOwner
            + ResealTimer
            + ChainTimeInfo
            + FindActionHandler;

    /// Submit `seal` as a valid solution for the header of `pow_hash`.
    /// Will check the seal, but not actually insert the block into the chain.
    fn submit_seal<C: ImportSealedBlock>(&self, chain: &C, pow_hash: H256, seal: Vec<Bytes>) -> Result<(), Error>;

    /// Get the sealing work package and if `Some`, apply some transform.
    fn map_sealing_work<C, F, T>(&self, client: &C, f: F) -> Option<T>
    where
        C: AccountData + BlockChain + BlockProducer + RegularKeyOwner + ChainTimeInfo + FindActionHandler,
        F: FnOnce(&ClosedBlock) -> T,
        Self: Sized;

    /// Imports transactions to mem pool.
    fn import_external_tranasctions<C: MiningBlockChainClient>(
        &self,
        client: &C,
        transactions: Vec<UnverifiedTransaction>,
    ) -> Vec<Result<TransactionImportResult, Error>>;

    /// Imports own (node owner) transaction to mem pool.
    fn import_own_transaction<C: MiningBlockChainClient>(
        &self,
        chain: &C,
        tx: SignedTransaction,
    ) -> Result<TransactionImportResult, Error>;

    /// Imports incomplete (node owner) transaction to mem pool.
    fn import_incomplete_transaction<C: MiningBlockChainClient + RegularKey + RegularKeyOwner>(
        &self,
        chain: &C,
        account_provider: &AccountProvider,
        tx: IncompleteTransaction,
        platform_address: PlatformAddress,
        passphrase: Option<Password>,
        seq: Option<u64>,
    ) -> Result<(H256, u64), Error>;

    /// Get a list of all pending transactions in the mem pool.
    fn ready_transactions(&self) -> Vec<SignedTransaction>;

    /// Get a list of all future transactions.
    fn future_transactions(&self) -> Vec<SignedTransaction>;

    /// Start sealing.
    fn start_sealing<C: MiningBlockChainClient>(&self, client: &C);

    /// Stop sealing.
    fn stop_sealing(&self);
}

/// Mining status
#[derive(Debug)]
pub struct MinerStatus {
    /// Number of transactions in queue with state `pending` (ready to be included in block)
    pub transactions_in_pending_queue: usize,
    /// Number of transactions in queue with state `future` (not yet ready to be included in block)
    pub transactions_in_future_queue: usize,
    /// Number of transactions included in currently mined block
    pub tranasction_in_pending_block: usize,
}

/// Represents the result of importing tranasction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionImportResult {
    /// Tranasction was imported to current queue.
    Current,
    /// Transaction was imported to future queue.
    Future,
}
