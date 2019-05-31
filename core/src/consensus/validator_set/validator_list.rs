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

use std::collections::HashSet;
use std::sync::{Arc, Weak};

use ckey::{public_to_address, Address, Public};
use ctypes::util::unexpected::OutOfBounds;
use parking_lot::RwLock;
use primitives::H256;

use super::super::BitSet;
use super::ValidatorSet;
use crate::client::ConsensusClient;
use crate::consensus::EngineError;
use crate::types::BlockId;

/// Validator set containing a known set of public keys.
pub struct ValidatorList {
    validators: Vec<Public>,
    addresses: HashSet<Address>,
    client: RwLock<Option<Weak<ConsensusClient>>>,
}

impl ValidatorList {
    pub fn new(validators: Vec<Public>) -> Self {
        let addresses = validators.iter().map(public_to_address).collect();
        ValidatorList {
            validators,
            addresses,
            client: Default::default(),
        }
    }
}

impl ValidatorSet for ValidatorList {
    fn contains(&self, _bh: &H256, public: &Public) -> bool {
        self.validators.contains(public)
    }

    fn contains_address(&self, _bh: &H256, address: &Address) -> bool {
        self.addresses.contains(address)
    }

    fn get(&self, _bh: &H256, nonce: usize) -> Public {
        let validator_n = self.validators.len();
        assert_ne!(0, validator_n, "Cannot operate with an empty validator set.");
        *self.validators.get(nonce % validator_n).expect("There are validator_n authorities; taking number modulo validator_n gives number in validator_n range; qed")
    }

    fn get_index(&self, _bh: &H256, public: &Public) -> Option<usize> {
        self.validators.iter().position(|v| v == public)
    }

    fn get_index_by_address(&self, _bh: &H256, address: &Address) -> Option<usize> {
        self.validators.iter().position(|v| public_to_address(v) == *address)
    }

    fn next_block_proposer(&self, parent: &H256, view: u64) -> Option<Address> {
        let client: Arc<ConsensusClient> = self.client.read().as_ref().and_then(Weak::upgrade)?;
        client.block_header(&BlockId::from(*parent)).map(|header| {
            let proposer = header.author();
            let prev_proposer_idx =
                self.get_index_by_address(&parent, &proposer).expect("The proposer must be in the validator set");
            let proposer_nonce = prev_proposer_idx + 1 + view as usize;
            ctrace!(ENGINE, "Proposer nonce: {}", proposer_nonce);
            public_to_address(&self.get(&parent, proposer_nonce))
        })
    }

    fn count(&self, _bh: &H256) -> usize {
        self.validators.len()
    }

    fn check_enough_votes(&self, parent: &H256, votes: &BitSet) -> Result<(), EngineError> {
        let validator_count = self.count(parent);
        let voted = votes.count();
        if voted * 3 > validator_count * 2 {
            Ok(())
        } else {
            let threshold = validator_count * 2 / 3;
            Err(EngineError::BadSealFieldSize(OutOfBounds {
                min: Some(threshold),
                max: None,
                found: voted,
            }))
        }
    }

    fn register_client(&self, client: Weak<ConsensusClient>) {
        *self.client.write() = Some(client);
    }

    fn addresses(&self, _parent: &H256) -> Vec<Address> {
        self.validators.iter().map(public_to_address).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ckey::Public;

    use super::super::ValidatorSet;
    use super::ValidatorList;

    #[test]
    fn validator_set() {
        let a1 = Public::from_str("34959b60d54703e9dfe36afb1e9950a4abe34d666cbb64c92969013bc9cc74063f9e4680d9d48c4597ee623bd4b507a1b2f43a9c5766a06463f85b73a94c51d1").unwrap();
        let a2 = Public::from_str("8c5a25bfafceea03073e2775cfb233a46648a088c12a1ca18a5865534887ccf60e1670be65b5f8e29643f463fdf84b1cbadd6027e71d8d04496570cb6b04885d").unwrap();
        let set = ValidatorList::new(vec![a1, a2]);
        assert!(set.contains(&Default::default(), &a1));
        assert_eq!(set.get(&Default::default(), 0), a1);
        assert_eq!(set.get(&Default::default(), 1), a2);
        assert_eq!(set.get(&Default::default(), 2), a1);
    }
}
