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

use std::sync::{Arc, Weak};

use ckey::{public_to_address, Address, Public};
use ctypes::util::unexpected::OutOfBounds;
use ctypes::BlockHash;
use parking_lot::RwLock;

use super::{RoundRobinValidator, ValidatorSet};
use crate::client::ConsensusClient;
use crate::consensus::bit_set::BitSet;
use crate::consensus::stake::{get_validators, Validator};
use crate::consensus::EngineError;

/// Validator set containing a known set of public keys.
pub struct DynamicValidator {
    initial_list: RoundRobinValidator,
    client: RwLock<Option<Weak<dyn ConsensusClient>>>,
}

impl DynamicValidator {
    pub fn new(initial_validators: Vec<Public>) -> Self {
        DynamicValidator {
            initial_list: RoundRobinValidator::new(initial_validators),
            client: Default::default(),
        }
    }

    fn validators(&self, parent: BlockHash) -> Option<Vec<Validator>> {
        let client: Arc<dyn ConsensusClient> =
            self.client.read().as_ref().and_then(Weak::upgrade).expect("Client is not initialized");
        let block_id = parent.into();
        let term_id = client.current_term_id(block_id).expect(
            "valdators() is called when creating a block or verifying a block.
            Minor creates a block only when the parent block is imported.
            The n'th block is verified only when the parent block is imported.",
        );
        if term_id == 0 {
            return None
        }
        let state = client.state_at(block_id)?;
        let validators = get_validators(&state).unwrap();
        if validators.is_empty() {
            None
        } else {
            let mut validators: Vec<_> = validators.into();
            validators.reverse();
            Some(validators)
        }
    }

    fn validators_pubkey(&self, parent: BlockHash) -> Option<Vec<Public>> {
        self.validators(parent).map(|validators| validators.into_iter().map(|val| *val.pubkey()).collect())
    }
}

impl ValidatorSet for DynamicValidator {
    fn contains(&self, parent: &BlockHash, public: &Public) -> bool {
        if let Some(validators) = self.validators_pubkey(*parent) {
            validators.into_iter().any(|pubkey| pubkey == *public)
        } else {
            self.initial_list.contains(parent, public)
        }
    }

    fn contains_address(&self, parent: &BlockHash, address: &Address) -> bool {
        if let Some(validators) = self.validators_pubkey(*parent) {
            validators.into_iter().any(|pubkey| public_to_address(&pubkey) == *address)
        } else {
            self.initial_list.contains_address(parent, address)
        }
    }

    fn get(&self, parent: &BlockHash, index: usize) -> Public {
        if let Some(validators) = self.validators_pubkey(*parent) {
            let n_validators = validators.len();
            *validators.get(index % n_validators).unwrap()
        } else {
            self.initial_list.get(parent, index)
        }
    }

    fn get_index(&self, parent: &BlockHash, public: &Public) -> Option<usize> {
        if let Some(validators) = self.validators_pubkey(*parent) {
            validators.into_iter().enumerate().find(|(_index, pubkey)| pubkey == public).map(|(index, _)| index)
        } else {
            self.initial_list.get_index(parent, public)
        }
    }

    fn get_index_by_address(&self, parent: &BlockHash, address: &Address) -> Option<usize> {
        if let Some(validators) = self.validators_pubkey(*parent) {
            validators
                .into_iter()
                .enumerate()
                .find(|(_index, pubkey)| public_to_address(pubkey) == *address)
                .map(|(index, _)| index)
        } else {
            self.initial_list.get_index_by_address(parent, address)
        }
    }

    fn next_block_proposer(&self, parent: &BlockHash, view: u64) -> Option<Address> {
        if let Some(validators) = self.validators_pubkey(*parent) {
            let n_validators = validators.len();
            let index = view as usize % n_validators;
            Some(public_to_address(validators.get(index).unwrap()))
        } else {
            self.initial_list.next_block_proposer(parent, view)
        }
    }

    fn count(&self, parent: &BlockHash) -> usize {
        if let Some(validators) = self.validators(*parent) {
            validators.len()
        } else {
            self.initial_list.count(parent)
        }
    }

    fn normalized_voting_power(
        &self,
        height: u64,
        parent: &BlockHash,
        index: usize,
        total_power: u64,
    ) -> Result<u64, EngineError> {
        if let Some(validators) = self.validators(*parent) {
            let validator = validators.get(index).ok_or_else(|| EngineError::ValidatorNotExist {
                height,
                index,
            })?;
            let signer_delegation = validator.delegation();
            let total_delegation: u64 = validators.iter().map(Validator::delegation).sum();
            let normalized_power = ((signer_delegation * total_power) as f64) / (total_delegation as f64);
            Ok(normalized_power as u64)
        } else {
            self.initial_list.normalized_voting_power(height, parent, index, total_power)
        }
    }

    fn check_enough_votes(&self, parent: &BlockHash, votes: &BitSet) -> Result<(), EngineError> {
        if let Some(validators) = self.validators(*parent) {
            let mut voted_delegation = 0u64;
            let n_validators = validators.len();
            for index in votes.true_index_iter() {
                assert!(index < n_validators);
                let validator = validators.get(index).ok_or_else(|| {
                    EngineError::ValidatorNotExist {
                        height: 0, // FIXME
                        index,
                    }
                })?;
                voted_delegation += validator.delegation();
            }
            let total_delegation: u64 = validators.iter().map(Validator::delegation).sum();
            if voted_delegation * 3 > total_delegation * 2 {
                Ok(())
            } else {
                let threshold = total_delegation as usize * 2 / 3;
                Err(EngineError::BadSealFieldSize(OutOfBounds {
                    min: Some(threshold),
                    max: Some(total_delegation as usize),
                    found: voted_delegation as usize,
                }))
            }
        } else {
            self.initial_list.check_enough_votes(parent, votes)
        }
    }

    /// Allows blockchain state access.
    fn register_client(&self, client: Weak<dyn ConsensusClient>) {
        self.initial_list.register_client(Weak::clone(&client));
        let mut client_lock = self.client.write();
        assert!(client_lock.is_none());
        *client_lock = Some(client);
    }

    fn addresses(&self, parent: &BlockHash) -> Vec<Address> {
        if let Some(validators) = self.validators_pubkey(*parent) {
            validators.iter().map(public_to_address).collect()
        } else {
            self.initial_list.addresses(parent)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use ckey::Public;

    use super::super::ValidatorSet;
    use super::DynamicValidator;
    use crate::client::{ConsensusClient, TestBlockChainClient};

    #[test]
    fn validator_set() {
        let a1 = Public::from_str("34959b60d54703e9dfe36afb1e9950a4abe34d666cbb64c92969013bc9cc74063f9e4680d9d48c4597ee623bd4b507a1b2f43a9c5766a06463f85b73a94c51d1").unwrap();
        let a2 = Public::from_str("8c5a25bfafceea03073e2775cfb233a46648a088c12a1ca18a5865534887ccf60e1670be65b5f8e29643f463fdf84b1cbadd6027e71d8d04496570cb6b04885d").unwrap();
        let set = DynamicValidator::new(vec![a1, a2]);
        let test_client: Arc<dyn ConsensusClient> = Arc::new({
            let mut client = TestBlockChainClient::new();
            client.term_id = Some(1);
            client
        });
        set.register_client(Arc::downgrade(&test_client));
        assert!(set.contains(&Default::default(), &a1));
        assert_eq!(set.get(&Default::default(), 0), a1);
        assert_eq!(set.get(&Default::default(), 1), a2);
        assert_eq!(set.get(&Default::default(), 2), a1);
    }
}
