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

mod local_parcels;
mod mem_pool;
mod miner;
mod sealing_queue;
mod work_notify;

use ckey::Address;
use primitives::{Bytes, H256, U256};

pub use self::miner::{Miner, MinerOptions};
use super::account_provider::SignError;
use super::block::ClosedBlock;
use super::client::{AccountData, BlockChain, BlockProducer, ImportSealedBlock, MiningBlockChainClient};
use super::consensus::EngineType;
use super::error::Error;
use super::parcel::{SignedParcel, UnverifiedParcel};
use super::state::TopStateInfo;

/// Miner client API
pub trait MinerService: Send + Sync {
    /// Type representing chain state
    type State: TopStateInfo + 'static;

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
    fn set_engine_signer(&self, address: Address, password: String) -> Result<(), SignError>;

    /// Get current minimal fee for parcels accepted to queue.
    fn minimal_fee(&self) -> U256;

    /// Set minimal fee of parcel to be accepted for mining.
    fn set_minimal_fee(&self, min_fee: U256);

    /// Get current parcels limit in queue.
    fn parcels_limit(&self) -> usize;

    /// Set maximal number of parcels kept in the queue (both current and future).
    fn set_parcels_limit(&self, limit: usize);

    /// Called when blocks are imported to chain, updates parcels queue.
    fn chain_new_blocks<C>(&self, chain: &C, imported: &[H256], invalid: &[H256], enacted: &[H256], retracted: &[H256])
    where
        C: AccountData + BlockChain + BlockProducer + ImportSealedBlock;

    /// PoW chain - can produce work package
    fn can_produce_work_package(&self) -> bool;

    /// Get the type of consensus engine.
    fn engine_type(&self) -> EngineType;

    /// New chain head event. Restart mining operation.
    fn update_sealing<C>(&self, chain: &C)
    where
        C: AccountData + BlockChain + BlockProducer + ImportSealedBlock;

    /// Submit `seal` as a valid solution for the header of `pow_hash`.
    /// Will check the seal, but not actually insert the block into the chain.
    fn submit_seal<C: ImportSealedBlock>(&self, chain: &C, pow_hash: H256, seal: Vec<Bytes>) -> Result<(), Error>;

    /// Get the sealing work package and if `Some`, apply some transform.
    fn map_sealing_work<C, F, T>(&self, client: &C, f: F) -> Option<T>
    where
        C: AccountData + BlockChain + BlockProducer,
        F: FnOnce(&ClosedBlock) -> T,
        Self: Sized;

    /// Imports parcels to mem pool.
    fn import_external_parcels<C: MiningBlockChainClient>(
        &self,
        client: &C,
        parcels: Vec<UnverifiedParcel>,
    ) -> Vec<Result<ParcelImportResult, Error>>;

    /// Imports own (node owner) parcel to mem pool.
    fn import_own_parcel<C: MiningBlockChainClient>(
        &self,
        chain: &C,
        parcel: SignedParcel,
    ) -> Result<ParcelImportResult, Error>;

    /// Get a list of all pending parcels in the mem pool.
    fn ready_parcels(&self) -> Vec<SignedParcel>;

    /// Get a list of all future parcels.
    fn future_parcels(&self) -> Vec<SignedParcel>;
}

/// Mining status
#[derive(Debug)]
pub struct MinerStatus {
    /// Number of parcels in queue with state `pending` (ready to be included in block)
    pub parcels_in_pending_queue: usize,
    /// Number of parcels in queue with state `future` (not yet ready to be included in block)
    pub parcels_in_future_queue: usize,
    /// Number of parcels included in currently mined block
    pub parcels_in_pending_block: usize,
}

/// Represents the result of importing parcel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParcelImportResult {
    /// Parcel was imported to current queue.
    Current,
    /// Parcel was imported to future queue.
    Future,
}
