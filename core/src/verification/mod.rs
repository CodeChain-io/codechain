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

mod canon_verifier;
mod noop_verifier;
pub mod queue;
#[cfg_attr(feature = "cargo-clippy", allow(clippy::module_inception))]
mod verification;
mod verifier;

pub use self::canon_verifier::CanonVerifier;
pub use self::noop_verifier::NoopVerifier;
pub use self::queue::{BlockQueue, Config as QueueConfig};
pub use self::verification::*;
pub use self::verifier::Verifier;

use crate::client::{BlockInfo, TransactionInfo};

/// Verifier type.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum VerifierType {
    /// Verifies block normally.
    Canon,
    /// Verifies block normally, but skips seal verification.
    CanonNoSeal,
    /// Does not verify block at all.
    /// Used in tests.
    Noop,
}

impl VerifierType {
    /// Check if seal verification is enabled for this verifier type.
    pub fn verifying_seal(self) -> bool {
        match self {
            VerifierType::Canon => true,
            VerifierType::Noop | VerifierType::CanonNoSeal => false,
        }
    }
}

impl Default for VerifierType {
    fn default() -> Self {
        VerifierType::Canon
    }
}

/// Create a new verifier based on type.
pub fn new<C: BlockInfo + TransactionInfo>(v: VerifierType) -> Box<Verifier<C>> {
    match v {
        VerifierType::Canon | VerifierType::CanonNoSeal => Box::new(CanonVerifier),
        VerifierType::Noop => Box::new(NoopVerifier),
    }
}
