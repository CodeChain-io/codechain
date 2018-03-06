use codechain_types::{H256, Address};

mod validator_set;

/// A validator set.
pub trait ValidatorSet: Send + Sync {
    /// Checks if a given address is a validator,
    /// using underlying, default call mechanism.
    fn contains(&self, parent: &H256, address: &Address) -> bool;

    /// Draws an validator nonce modulo number of validators.
    fn get(&self, parent: &H256, nonce: usize) -> Address;

    /// Returns the current number of validators.
    fn count(&self, parent: &H256) -> usize;
}

