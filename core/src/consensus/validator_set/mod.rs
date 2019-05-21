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

use std::sync::{Arc, Weak};

use ckey::{public_to_address, Address, Public};
use ctypes::BlockNumber;
use primitives::{Bytes, H256};

use self::validator_list::ValidatorList;
use crate::client::ConsensusClient;

pub mod validator_list;

/// Creates a validator set from validator public keys.
pub fn new_validator_set(validators: Vec<Public>) -> Arc<ValidatorSet> {
    Arc::new(ValidatorList::new(validators))
}

/// A validator set.
pub trait ValidatorSet: Send + Sync {
    /// Checks if a given public key is a validator,
    /// using underlying, default call mechanism.
    fn contains(&self, parent: &H256, public: &Public) -> bool;

    /// Checks if a given address is a validator.
    fn contains_address(&self, parent: &H256, address: &Address) -> bool;

    /// Draws a validator from nonce modulo number of validators.
    fn get(&self, parent: &H256, nonce: usize) -> Public;

    /// Draws a validator address from nonce modulo number of validators.
    fn get_address(&self, parent: &H256, nonce: usize) -> Address {
        public_to_address(&self.get(parent, nonce))
    }

    /// Draws a validator from nonce modulo number of validators.
    fn get_index(&self, parent: &H256, public: &Public) -> Option<usize>;

    /// Draws a validator index from validator address.
    fn get_index_by_address(&self, parent: &H256, address: &Address) -> Option<usize>;

    /// Returns the current number of validators.
    fn count(&self, parent: &H256) -> usize;

    /// Notifies about malicious behaviour.
    fn report_malicious(&self, _validator: &Address, _set_block: BlockNumber, _block: BlockNumber, _proof: Bytes) {}
    /// Notifies about benign misbehaviour.
    fn report_benign(&self, _validator: &Address, _set_block: BlockNumber, _block: BlockNumber) {}
    /// Allows blockchain state access.
    fn register_client(&self, _client: Weak<ConsensusClient>) {}

    fn addresses(&self, _parent: &H256) -> Vec<Address>;
}
