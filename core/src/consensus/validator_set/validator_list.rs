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

use heapsize::HeapSizeOf;
use std::collections::HashSet;

use ckey::{public_to_address, Address, Public};
use ctypes::BlockNumber;
use primitives::H256;

use super::super::EpochChange;
use super::ValidatorSet;
use crate::codechain_machine::CodeChainMachine;
use crate::error::Error;
use crate::header::Header;

/// Validator set containing a known set of public keys.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ValidatorList {
    validators: Vec<Public>,
    addresses: HashSet<Address>,
}

impl ValidatorList {
    pub fn new(validators: Vec<Public>) -> Self {
        let addresses = validators.iter().map(|public| public_to_address(public)).collect();
        ValidatorList {
            validators,
            addresses,
        }
    }
}

impl ::std::ops::Deref for ValidatorList {
    type Target = [Public];

    fn deref(&self) -> &[Public] {
        &self.validators
    }
}

impl From<Vec<Public>> for ValidatorList {
    fn from(validators: Vec<Public>) -> Self {
        let addresses = validators.iter().map(|public| public_to_address(public)).collect();
        ValidatorList {
            validators,
            addresses,
        }
    }
}

impl HeapSizeOf for ValidatorList {
    fn heap_size_of_children(&self) -> usize {
        self.validators.heap_size_of_children()
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

        if validator_n == 0 {
            panic!("Cannot operate with an empty validator set.");
        }

        *self.validators.get(nonce % validator_n).expect("There are validator_n authorities; taking number modulo validator_n gives number in validator_n range; qed")
    }

    fn get_address(&self, bh: &H256, nonce: usize) -> Address {
        public_to_address(&self.get(bh, nonce))
    }

    fn get_index(&self, _bh: &H256, public: &Public) -> Option<usize> {
        self.validators.iter().position(|v| v == public)
    }

    fn count(&self, _bh: &H256) -> usize {
        self.validators.len()
    }

    fn is_epoch_end(&self, first: bool, _chain_head: &Header) -> Option<Vec<u8>> {
        if first {
            Some(Vec::new()) // allow transition to fixed list, and instantly
        } else {
            None
        }
    }

    fn signals_epoch_end(&self, _: bool, _: &Header) -> EpochChange {
        EpochChange::No
    }

    fn epoch_set(
        &self,
        _first: bool,
        _: &CodeChainMachine,
        _: BlockNumber,
        _: &[u8],
    ) -> Result<(ValidatorList, Option<H256>), Error> {
        Ok((self.clone(), None))
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
