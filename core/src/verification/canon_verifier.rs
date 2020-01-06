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

use super::verification;
use super::Verifier;
use crate::client::BlockChainTrait;
use crate::consensus::CodeChainEngine;
use crate::error::Error;
use ctypes::{CommonParams, Header};

/// A canonial verifier -- this does full verification.
pub struct CanonVerifier;

impl<C: BlockChainTrait> Verifier<C> for CanonVerifier {
    fn verify_block_family(
        &self,
        block: &[u8],
        header: &Header,
        parent: &Header,
        engine: &dyn CodeChainEngine,
        do_full: Option<verification::FullFamilyParams<C>>,
        common_params: &CommonParams,
    ) -> Result<(), Error> {
        verification::verify_block_family(block, header, parent, engine, do_full, common_params)
    }

    fn verify_block_final(&self, expected: &Header, got: &Header) -> Result<(), Error> {
        verification::verify_block_final(expected, got)
    }

    fn verify_block_external(&self, header: &Header, engine: &dyn CodeChainEngine) -> Result<(), Error> {
        engine.verify_block_external(header)
    }
}
