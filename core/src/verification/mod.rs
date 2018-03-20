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

mod noop_verifier;
mod verification;
mod verifier;

use super::client::BlockInfo;

/// Verifier type.
#[derive(Debug, PartialEq, Clone)]
pub enum VerifierType {
    /// Verifies block normally.
    Canon,
    /// Verifies block normally, but skips seal verification.
    CanonNoSeal,
    /// Does not verify block at all.
    /// Used in tests.
    Noop,
}

impl Default for VerifierType {
    // FIXME: Change the default verifier to Canon once it is implemented.
    fn default() -> Self {
        VerifierType::Noop
    }
}

/// Create a new verifier based on type.
pub fn new<C: BlockInfo>(v: VerifierType) -> Box<Verifier<C>> {
    match v {
        VerifierType::Canon | VerifierType::CanonNoSeal => unimplemented!(),
        VerifierType::Noop => Box::new(NoopVerifier),
    }
}

pub use self::noop_verifier::NoopVerifier;
pub use self::verification::*;
pub use self::verifier::Verifier;

