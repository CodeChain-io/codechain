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

use self::validator_list::RoundRobinValidator;
use super::BitSet;
use crate::client::ConsensusClient;
use crate::consensus::EngineError;
use ckey::{Address, Public};
use ctypes::BlockHash;
use std::sync::Weak;

mod dynamic_validator;
pub mod validator_list;

pub use self::dynamic_validator::DynamicValidator;

/// A validator set.
pub trait ValidatorSet: Send + Sync {
    /// Checks if a given public key is a validator,
    /// using underlying, default call mechanism.
    fn contains(&self, parent: &BlockHash, public: &Public) -> bool;

    /// Checks if a given address is a validator.
    fn contains_address(&self, parent: &BlockHash, address: &Address) -> bool;

    /// Draws a validator from index modulo number of validators.
    fn get(&self, parent: &BlockHash, index: usize) -> Public;

    /// Draws a validator from nonce modulo number of validators.
    fn get_index(&self, parent: &BlockHash, public: &Public) -> Option<usize>;

    /// Draws a validator index from validator address.
    fn get_index_by_address(&self, parent: &BlockHash, address: &Address) -> Option<usize>;

    fn next_block_proposer(&self, parent: &BlockHash, view: u64) -> Option<Address>;

    /// Returns the current number of validators.
    fn count(&self, parent: &BlockHash) -> usize;

    fn check_enough_votes(&self, parent: &BlockHash, votes: &BitSet) -> Result<(), EngineError>;

    /// Allows blockchain state access.
    fn register_client(&self, _client: Weak<dyn ConsensusClient>) {}

    fn addresses(&self, _parent: &BlockHash) -> Vec<Address>;
}
