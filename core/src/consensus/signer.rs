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

use ckey::{Address, Password, Public, Signature};
use primitives::H256;

use crate::account_provider::{AccountProvider, SignError};

/// Everything that an Engine needs to sign messages.
pub struct EngineSigner {
    account_provider: Arc<AccountProvider>,
    signer: Option<(Address, Public)>,
    password: Option<Password>,
}

impl Default for EngineSigner {
    fn default() -> Self {
        EngineSigner {
            account_provider: AccountProvider::transient_provider(),
            signer: Default::default(),
            password: Default::default(),
        }
    }
}

impl EngineSigner {
    /// Set up the signer to sign with given address and password.
    pub fn set(&mut self, ap: Arc<AccountProvider>, address: Address, password: Option<Password>) {
        let public = ap.public(&address, password.clone()).expect("The address must be registered in AccountProvier");
        self.account_provider = ap;
        self.signer = Some((address, public));
        self.password = password;
        cdebug!(ENGINE, "Setting Engine signer to {}", address);
    }

    /// Sign a consensus message hash.
    pub fn sign(&self, hash: H256) -> Result<Signature, SignError> {
        self.account_provider.sign(
            self.signer.map(|(address, _public)| address).unwrap_or_else(Default::default),
            self.password.clone(),
            hash,
        )
    }

    /// Public Key of signer.
    pub fn public(&self) -> Option<&Public> {
        self.signer.as_ref().map(|(_address, public)| public)
    }

    /// Check if the given address is the signing address.
    pub fn is_address(&self, a: &Address) -> bool {
        self.signer.map_or(false, |(address, _public)| *a == address)
    }

    /// Check if the signing address was set.
    pub fn is_some(&self) -> bool {
        self.signer.is_some()
    }
}
