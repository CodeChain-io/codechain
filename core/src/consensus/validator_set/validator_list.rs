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

use ctypes::{Address, H256};

use super::ValidatorSet;
use super::super::EpochChange;
use super::super::super::error::Error;
use super::super::super::codechain_machine::CodeChainMachine;
use super::super::super::header::Header;
use super::super::super::types::BlockNumber;

/// Validator set containing a known set of addresses.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ValidatorList {
    validators: Vec<Address>,
}

impl ValidatorList {
    pub fn new(validators: Vec<Address>) -> Self {
        ValidatorList { validators }
    }

    /// Convert into inner representation.
    pub fn into_inner(self) -> Vec<Address> {
        self.validators
    }
}

impl ::std::ops::Deref for ValidatorList {
    type Target = [Address];

    fn deref(&self) -> &[Address] {
        &self.validators
    }
}

impl From<Vec<Address>> for ValidatorList {
    fn from(validators: Vec<Address>) -> Self {
        ValidatorList { validators }
    }
}

impl HeapSizeOf for ValidatorList {
    fn heap_size_of_children(&self) -> usize {
        self.validators.heap_size_of_children()
    }
}

impl ValidatorSet for ValidatorList {
    fn contains(&self, _bh: &H256, address: &Address) -> bool {
        self.validators.contains(address)
    }

    fn get(&self, _bh: &H256, nonce: usize) -> Address {
        let validator_n = self.validators.len();

        if validator_n == 0 {
            panic!("Cannot operate with an empty validator set.");
        }

        self.validators.get(nonce % validator_n).expect("There are validator_n authorities; taking number modulo validator_n gives number in validator_n range; qed").clone()
    }

    fn count(&self, _bh: &H256) -> usize {
        self.validators.len()
    }

    fn is_epoch_end(&self, first: bool, _chain_head: &Header) -> Option<Vec<u8>> {
        match first {
            true => Some(Vec::new()), // allow transition to fixed list, and instantly
            false => None,
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
    use ctypes::Address;

    use super::super::ValidatorSet;
    use super::ValidatorList;

    #[test]
    fn validator_set() {
        let a1 = Address::from_str("cd1722f3947def4cf144679da39c4c32bdc35681").unwrap();
        let a2 = Address::from_str("0f572e5295c57f15886f9b263e2f6d2d6c7b5ec6").unwrap();
        let set = ValidatorList::new(vec![a1.clone(), a2.clone()]);
        assert!(set.contains(&Default::default(), &a1));
        assert_eq!(set.get(&Default::default(), 0), a1);
        assert_eq!(set.get(&Default::default(), 1), a2);
        assert_eq!(set.get(&Default::default(), 2), a1);
    }
}
