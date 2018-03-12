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

use std::sync::Arc;

use bytes::Bytes;
use super::machine::Machine;

/// Seal type.
#[derive(Debug, PartialEq, Eq)]
pub enum Seal {
    /// Proposal seal; should be broadcasted, but not inserted into blockchain.
    Proposal(Vec<Bytes>),
    /// Regular block seal; should be part of the blockchain.
    Regular(Vec<Bytes>),
    /// Engine does generate seal for this block right now.
    None,
}

/// A consensus mechanism for the chain.
pub trait ConsensusEngine<M: Machine>: Sync + Send {
    /// The name of this engine.
    fn name(&self) -> &str;

    /// Get access to the underlying state machine.
    fn machine(&self) -> &M;

    /// The number of additional header fields required for this engine.
    fn seal_fields(&self, _header: &M::Header) -> usize { 0 }

    /// None means that it requires external input (e.g. PoW) to seal a block.
    /// Some(true) means the engine is currently prime for seal generation (i.e. node is the current validator).
    /// Some(false) means that the node might seal internally but is not qualified now.
    fn seals_internally(&self) -> Option<bool> { None }

    /// Attempt to seal the block internally.
    ///
    /// If `Some` is returned, then you get a valid seal.
    ///
    /// This operation is synchronous and may (quite reasonably) not be available, in which None will
    /// be returned.
    ///
    /// It is fine to require access to state or a full client for this function, since
    /// light clients do not generate seals.
    fn generate_seal(&self, _block: &M::LiveBlock, _parent: &M::Header) -> Seal { Seal::None }

    /// Verify a locally-generated seal of a header.
    ///
    /// If this engine seals internally,
    /// no checks have to be done here, since all internally generated seals
    /// should be valid.
    ///
    /// Externally-generated seals (e.g. PoW) will need to be checked for validity.
    ///
    /// It is fine to require access to state or a full client for this function, since
    /// light clients do not generate seals.
    fn verify_local_seal(&self, header: &M::Header) -> Result<(), M::Error>;

    /// Phase 1 quick block verification. Only does checks that are cheap. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_basic(&self, _header: &M::Header) -> Result<(), M::Error> { Ok(()) }

    /// Phase 2 verification. Perform costly checks such as transaction signatures. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_unordered(&self, _header: &M::Header) -> Result<(), M::Error> { Ok(()) }

    /// Phase 3 verification. Check block information against parent. Returns either a null `Ok` or a general error detailing the problem with import.
    fn verify_block_family(&self, _header: &M::Header, _parent: &M::Header) -> Result<(), M::Error> { Ok(()) }

    /// Phase 4 verification. Verify block header against potentially external data.
    /// Should only be called when `register_client` has been called previously.
    fn verify_block_external(&self, _header: &M::Header) -> Result<(), M::Error> { Ok(()) }

    /// Trigger next step of the consensus engine.
    fn step(&self) {}

    /// Block transformation functions, before the transactions.
    fn on_new_block(
        &self,
        _block: &mut M::LiveBlock,
    ) -> Result<(), M::Error> {
        Ok(())
    }

    /// Block transformation functions, after the transactions.
    fn on_close_block(&self, _block: &mut M::LiveBlock) -> Result<(), M::Error> {
        Ok(())
    }
}

/// Results of a query of whether an epoch change occurred at the given block.
pub enum EpochChange {
    /// Cannot determine until more data is passed.
    Unsure,
    /// No epoch change.
    No,
    /// The epoch will change, with proof.
    Yes(Proof),
}

/// Proof generated on epoch change.
pub enum Proof {
    /// Known proof (extracted from signal)
    Known(Vec<u8>)
}

