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

use ctypes::{CommonParams, Header};

use super::{verification, Verifier};
use crate::client::BlockChainTrait;
use crate::consensus::CodeChainEngine;
use crate::error::Error;

/// A no-op verifier -- this will verify everything it's given immediately.
pub struct NoopVerifier;

impl<C: BlockChainTrait> Verifier<C> for NoopVerifier {
    fn verify_block_family(
        &self,
        _block: &[u8],
        _: &Header,
        _t: &Header,
        _: &CodeChainEngine,
        _: Option<verification::FullFamilyParams<C>>,
        _common_params: &CommonParams,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn verify_block_final(&self, _expected: &Header, _got: &Header) -> Result<(), Error> {
        Ok(())
    }

    fn verify_block_external(&self, _header: &Header, _engine: &CodeChainEngine) -> Result<(), Error> {
        Ok(())
    }
}
