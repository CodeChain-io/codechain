use heapsize::HeapSizeOf;

use codechain_types::{Address, H256};
use super::ValidatorSet;

/// Validator set containing a known set of addresses.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ValidatorList {
    validators: Vec<Address>,
}

impl ValidatorList {
    pub fn new(validators: Vec<Address>) -> Self {
        ValidatorList {
            validators
        }
    }

    /// Convert into inner representation.
    pub fn into_inner(self) -> Vec<Address> {
        self.validators
    }
}

impl ::std::ops::Deref for ValidatorList {
    type Target = [Address];

    fn deref(&self) -> &[Address] { &self.validators }
}

impl From<Vec<Address>> for ValidatorList {
    fn from(validators: Vec<Address>) -> Self {
        ValidatorList {
            validators,
        }
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
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use codechain_types::Address;
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
