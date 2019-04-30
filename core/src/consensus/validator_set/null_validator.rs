// Copyright 2019. Kodebox, Inc.
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


use ckey::{Address, Public};
use primitives::H256;

use super::validator_list::ValidatorList;
use super::ValidatorSet;
use crate::codechain_machine::CodeChainMachine;
use crate::error::Error;
use crate::header::Header;

/// Validator set containing a known set of public keys.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct NullValidator {}

impl ValidatorSet for NullValidator {
    fn contains(&self, _bh: &H256, _public: &Public) -> bool {
        true
    }

    fn contains_address(&self, _bh: &H256, _address: &Address) -> bool {
        true
    }

    fn get(&self, _parent: &H256, _nonce: usize) -> Public {
        unimplemented!()
    }

    fn get_address(&self, _parent: &H256, _nonce: usize) -> Address {
        unimplemented!()
    }

    fn get_index(&self, _parent: &H256, _public: &Public) -> Option<usize> {
        unimplemented!()
    }

    fn get_index_by_address(&self, _parent: &H256, _address: &Address) -> Option<usize> {
        unimplemented!()
    }

    fn count(&self, _parent: &H256) -> usize {
        unimplemented!()
    }

    fn is_epoch_end(&self, _first: bool, _chain_head: &Header) -> Option<Vec<u8>> {
        unimplemented!()
    }

    fn epoch_set(
        &self,
        _first: bool,
        _machine: &CodeChainMachine,
        _number: u64,
        _proof: &[u8],
    ) -> Result<(ValidatorList, Option<H256>), Error> {
        unimplemented!()
    }
}
