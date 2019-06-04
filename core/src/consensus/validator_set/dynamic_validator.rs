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
use parking_lot::RwLock;
use primitives::H256;

use super::{ValidatorList, ValidatorSet};
use crate::client::ConsensusClient;
use crate::consensus::bit_set::BitSet;
use crate::consensus::EngineError;
use consensus::stake::{get_validators, Validators};

/// Validator set containing a known set of public keys.
pub struct DynamicValidator {
    initial_list: ValidatorList,
    client: RwLock<Option<Weak<ConsensusClient>>>,
}

impl DynamicValidator {
    pub fn new(initial_validators: Vec<Public>) -> Self {
        DynamicValidator {
            initial_list: ValidatorList::new(initial_validators),
            client: Default::default(),
        }
    }

    fn validators_at_term_begin(&self, parent: H256) -> Option<Validators> {
        let client: Arc<ConsensusClient> = self.client.read().as_ref().and_then(Weak::upgrade)?;
        let state = client.state_at_term_begin(parent.into())?;
        Some(get_validators(&state).unwrap())
    }

    fn validators(&self, parent: H256) -> Option<Validators> {
        let client: Arc<ConsensusClient> = self.client.read().as_ref().and_then(Weak::upgrade)?;
        let block_id = parent.into();
        if client.current_term_id(block_id)? == 0 {
            return None
        }
        let state = client.state_at(block_id)?;
        Some(get_validators(&state).unwrap())
    }

    fn validators_pubkey(&self, parent: H256) -> Option<Vec<Public>> {
        self.validators(parent).map(|validators| validators.pubkeys())
    }
}

impl ValidatorSet for DynamicValidator {
    fn contains(&self, parent: &H256, public: &Public) -> bool {
        if let Some(validators) = self.validators_pubkey(*parent) {
            validators.into_iter().any(|pubkey| pubkey == *public)
        } else {
            self.initial_list.contains(parent, public)
        }
    }

    fn contains_address(&self, parent: &H256, address: &Address) -> bool {
        if let Some(validators) = self.validators_pubkey(*parent) {
            validators.into_iter().any(|pubkey| public_to_address(&pubkey) == *address)
        } else {
            self.initial_list.contains_address(parent, address)
        }
    }

    fn get(&self, parent: &H256, nonce: usize) -> Public {
        if let Some(validators) = self.validators_pubkey(*parent) {
            let n_validators = validators.len();
            validators.into_iter().nth(nonce % n_validators).unwrap()
        } else {
            self.initial_list.get(parent, nonce)
        }
    }

    fn get_index(&self, parent: &H256, public: &Public) -> Option<usize> {
        if let Some(validators) = self.validators_pubkey(*parent) {
            validators.into_iter().enumerate().find(|(_index, pubkey)| pubkey == public).map(|(index, _)| index)
        } else {
            self.initial_list.get_index(parent, public)
        }
    }

    fn get_index_by_address(&self, parent: &H256, address: &Address) -> Option<usize> {
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

    fn next_block_proposer(&self, parent: &H256, view: u64) -> Option<Address> {
        if let Some(validators) = self.validators_pubkey(*parent) {
            let n_validators = validators.len();
            let nonce = view as usize % n_validators;
            Some(public_to_address(validators.get(n_validators - nonce - 1).unwrap()))
        } else {
            self.initial_list.next_block_proposer(parent, view)
        }
    }

    fn count(&self, parent: &H256) -> usize {
        if let Some(validators) = self.validators(*parent) {
            validators.len()
        } else {
            self.initial_list.count(parent)
        }
    }

    fn check_enough_votes(&self, parent: &H256, votes: &BitSet) -> Result<(), EngineError> {
        if let Some(validators_at_term_begin) = self.validators_at_term_begin(*parent) {
            let validators =
                self.validators(*parent).expect("The validator must exist in the middle of term").pubkeys();
            let mut weight = 0;
            for index in votes.true_index_iter() {
                let pubkey = validators.get(index).ok_or_else(|| {
                    EngineError::ValidatorNotExist {
                        height: 0, // FIXME
                        index,
                    }
                })?;
                weight += validators_at_term_begin.weight(pubkey).unwrap() as usize;
            }
            let total_weight = validators_at_term_begin.total_weight() as usize;
            if weight * 3 > total_weight * 2 {
                Ok(())
            } else {
                let threshold = total_weight * 2 / 3;
                Err(EngineError::BadSealFieldSize(OutOfBounds {
                    min: Some(threshold),
                    max: Some(total_weight),
                    found: weight,
                }))
            }
        } else {
            self.initial_list.check_enough_votes(parent, votes)
        }
    }

    /// Allows blockchain state access.
    fn register_client(&self, client: Weak<ConsensusClient>) {
        self.initial_list.register_client(Weak::clone(&client));
        let mut client_lock = self.client.write();
        assert!(client_lock.is_none());
        *client_lock = Some(client);
    }

    fn addresses(&self, parent: &H256) -> Vec<Address> {
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

    use ckey::Public;

    use super::super::ValidatorSet;
    use super::DynamicValidator;

    #[test]
    fn validator_set() {
        let a1 = Public::from_str("34959b60d54703e9dfe36afb1e9950a4abe34d666cbb64c92969013bc9cc74063f9e4680d9d48c4597ee623bd4b507a1b2f43a9c5766a06463f85b73a94c51d1").unwrap();
        let a2 = Public::from_str("8c5a25bfafceea03073e2775cfb233a46648a088c12a1ca18a5865534887ccf60e1670be65b5f8e29643f463fdf84b1cbadd6027e71d8d04496570cb6b04885d").unwrap();
        let set = DynamicValidator::new(vec![a1, a2]);
        assert!(set.contains(&Default::default(), &a1));
        assert_eq!(set.get(&Default::default(), 0), a1);
        assert_eq!(set.get(&Default::default(), 1), a2);
        assert_eq!(set.get(&Default::default(), 2), a1);
    }
}
