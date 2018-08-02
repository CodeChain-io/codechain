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

use super::super::client::{BlockInfo, TransactionInfo};
use super::super::consensus::CodeChainEngine;
use super::super::error::Error;
use super::super::header::Header;
use super::verification;

/// Should be used to verify blocks.
pub trait Verifier<C>: Send + Sync
where
    C: BlockInfo + TransactionInfo, {
    /// Verify a block relative to its parent and uncles.
    fn verify_block_family(
        &self,
        block: &[u8],
        header: &Header,
        parent: &Header,
        engine: &CodeChainEngine,
        do_full: Option<verification::FullFamilyParams<C>>,
    ) -> Result<(), Error>;

    /// Do a final verification check for an enacted header vs its expected counterpart.
    fn verify_block_final(&self, expected: &Header, got: &Header) -> Result<(), Error>;
    /// Verify a block, inspecing external state.
    fn verify_block_external(&self, header: &Header, engine: &CodeChainEngine) -> Result<(), Error>;
}
