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

use codechain_types::{Address, H256};

use self::validator_list::ValidatorList;
use super::codechain_machine::CodeChainMachine;
use super::engine::EpochChange;
use super::error::Error;
use super::header::{BlockNumber, Header};

pub mod validator_list;

/// A validator set.
pub trait ValidatorSet: Send + Sync {
    /// Checks if a given address is a validator,
    /// using underlying, default call mechanism.
    fn contains(&self, parent: &H256, address: &Address) -> bool;

    /// Draws an validator nonce modulo number of validators.
    fn get(&self, parent: &H256, nonce: usize) -> Address;

    /// Returns the current number of validators.
    fn count(&self, parent: &H256) -> usize;

    /// Signalling that a new epoch has begun.
    ///
    /// The caller provided here may not generate proofs.
    ///
    /// `first` is true if this is the first block in the set.
    fn on_epoch_begin(&self, _first: bool, _header: &Header) -> Result<(), Error> {
        Ok(())
    }

    /// Extract genesis epoch data from the genesis state and header.
    fn genesis_epoch_data(&self, _header: &Header) -> Result<Vec<u8>, String> { Ok(Vec::new()) }

    /// Whether this block is the last one in its epoch.
    ///
    /// Indicates that the validator set changed at the given block in a manner
    /// that doesn't require finality.
    ///
    /// `first` is true if this is the first block in the set.
    fn is_epoch_end(&self, first: bool, chain_head: &Header) -> Option<Vec<u8>>;

    /// Whether the given block signals the end of an epoch, but change won't take effect
    /// until finality.
    ///
    /// Engine should set `first` only if the header is genesis. Multiplexing validator
    /// sets can set `first` to internal changes.
    fn signals_epoch_end(
        &self,
        first: bool,
        header: &Header,
    ) -> EpochChange;

    /// Recover the validator set from the given proof, the block number, and
    /// whether this header is first in its set.
    ///
    /// May fail if the given header doesn't kick off an epoch or
    /// the proof is invalid.
    ///
    /// Returns the set, along with a flag indicating whether finality of a specific
    /// hash should be proven.
    fn epoch_set(&self, first: bool, machine: &CodeChainMachine, number: BlockNumber, proof: &[u8])
        -> Result<(ValidatorList, Option<H256>), Error>;
}

