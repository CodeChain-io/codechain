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

use ckey::{Address, Message, SchnorrSignature};
use ctypes::util::unexpected::OutOfBounds;
use primitives::H256;
use rlp::UntrustedRlp;

use crate::codechain_machine::CodeChainMachine;
use crate::consensus::validator_set::validator_list::ValidatorList;
use crate::consensus::validator_set::ValidatorSet;
use crate::consensus::EngineError;
use crate::error::{BlockError, Error};
use crate::header::Header;

pub struct EpochVerifier<F>
where
    F: Fn(&SchnorrSignature, &Message) -> Result<Address, Error> + Send + Sync, {
    subchain_validators: ValidatorList,
    recover: F,
}

impl<F> EpochVerifier<F>
where
    F: Fn(&SchnorrSignature, &Message) -> Result<Address, Error> + Send + Sync,
{
    pub fn new(subchain_validators: ValidatorList, recover: F) -> Self {
        Self {
            subchain_validators,
            recover,
        }
    }
}

impl<F> super::super::EpochVerifier<CodeChainMachine> for EpochVerifier<F>
where
    F: Fn(&SchnorrSignature, &Message) -> Result<Address, Error> + Send + Sync,
{
    fn verify_light(&self, header: &Header) -> Result<(), Error> {
        let message = header.hash();

        let mut addresses = HashSet::new();
        let header_precommits_field = &header.seal().get(2).ok_or(BlockError::InvalidSeal)?;
        for rlp in UntrustedRlp::new(header_precommits_field).iter() {
            let signature: SchnorrSignature = rlp.as_val()?;
            let address = (self.recover)(&signature, &message)?;

            if !self.subchain_validators.contains_address(header.parent_hash(), &address) {
                return Err(EngineError::BlockNotAuthorized(address.to_owned()).into())
            }
            addresses.insert(address);
        }

        let n = addresses.len();
        let threshold = self.subchain_validators.len() * 2 / 3;
        if n > threshold {
            Ok(())
        } else {
            Err(EngineError::BadSealFieldSize(OutOfBounds {
                min: Some(threshold),
                max: None,
                found: n,
            })
            .into())
        }
    }

    fn check_finality_proof(&self, proof: &[u8]) -> Option<Vec<H256>> {
        let header: Header = ::rlp::decode(proof);
        self.verify_light(&header).ok().map(|_| vec![header.hash()])
    }
}
