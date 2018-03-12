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

use codechain_types::H256;

use super::machine::Machine;

/// Verifier for all blocks within an epoch with self-contained state.
pub trait EpochVerifier<M: Machine>: Send + Sync {
    /// Lightly verify the next block header.
    /// This may not be a header belonging to a different epoch.
    fn verify_light(&self, header: &M::Header) -> Result<(), M::Error>;

    /// Perform potentially heavier checks on the next block header.
    fn verify_heavy(&self, header: &M::Header) -> Result<(), M::Error> {
        self.verify_light(header)
    }

    /// Check a finality proof against this epoch verifier.
    /// Returns `Some(hashes)` if the proof proves finality of these hashes.
    /// Returns `None` if the proof doesn't prove anything.
    fn check_finality_proof(&self, _proof: &[u8]) -> Option<Vec<H256>> {
        None
    }
}

/// Special "no-op" verifier for stateless, epoch-less engines.
pub struct NoOp;

impl<M: Machine> EpochVerifier<M> for NoOp {
    fn verify_light(&self, _header: &M::Header) -> Result<(), M::Error> { Ok(()) }
}

