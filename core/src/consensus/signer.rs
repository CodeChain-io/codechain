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

use ckeys::{sign_ecdsa, ECDSASignature, Error as KeyError, Private};
use ctypes::{Address, H256};

/// Everything that an Engine needs to sign messages.
pub struct EngineSigner {
    address: Option<Address>,
    private: Option<Private>,
}

impl Default for EngineSigner {
    fn default() -> Self {
        EngineSigner {
            address: Default::default(),
            private: Default::default(),
        }
    }
}

impl EngineSigner {
    /// Set up the signer to sign with given address and password.
    pub fn set(&mut self, address: Address, private: Private) {
        self.address = Some(address);
        self.private = Some(private);
        debug!(target: "poa", "Setting Engine signer to {}", address);
    }

    /// Sign a consensus message hash.
    pub fn sign(&self, hash: H256) -> Result<ECDSASignature, KeyError> {
        if let Some(ref p) = self.private {
            sign_ecdsa(&p, &hash)
        } else {
            Err(KeyError::InvalidPrivate)
        }
    }

    /// Signing address.
    pub fn address(&self) -> Option<Address> {
        self.address.clone()
    }

    /// Check if the given address is the signing address.
    pub fn is_address(&self, address: &Address) -> bool {
        self.address.map_or(false, |a| a == *address)
    }

    /// Check if the signing address was set.
    pub fn is_some(&self) -> bool {
        self.address.is_some()
    }
}
