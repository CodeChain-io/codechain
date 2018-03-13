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

use codechain_types::{Address, H256};
use keys::{Private, Signature, Error as KeyError};

/// Everything that an Engine needs to sign messages.
pub struct EngineSigner {
    address: Address,
    private: Private,
}

impl EngineSigner {
    pub fn new(address: Address, private: Private) -> Self {
        EngineSigner {
            address,
            private,
        }
    }

    /// Sign a consensus message hash.
    pub fn sign(&self, hash: H256) -> Result<Signature, KeyError> {
        self.private.sign(&hash)
    }

    /// Signing address.
    pub fn address(&self) -> Address {
        self.address.clone()
    }

    /// Check if the given address is the signing address.
    pub fn is_address(&self, address: &Address) -> bool {
        self.address == *address
    }
}


