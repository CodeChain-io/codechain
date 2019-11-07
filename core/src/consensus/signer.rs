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

use ckey::{Address, Public, SchnorrSignature, Signature};
use ckeystore::DecryptedAccount;
use primitives::H256;
use vrf::openssl::ECVRF;

use crate::account_provider::{AccountProvider, Error as AccountProviderError};

/// Everything that an Engine needs to sign messages.
pub struct EngineSigner {
    account_provider: Arc<AccountProvider>,
    signer: Option<(Address, Public)>,
    decrypted_account: Option<DecryptedAccount>,
}

impl Default for EngineSigner {
    fn default() -> Self {
        EngineSigner {
            account_provider: AccountProvider::transient_provider(),
            signer: Default::default(),
            decrypted_account: Default::default(),
        }
    }
}

impl EngineSigner {
    /// Set up the signer to sign with given address and password.
    pub fn set(&mut self, ap: Arc<AccountProvider>, address: Address) {
        let public = {
            let account = ap.get_unlocked_account(&address).expect("The address must be registered in AccountProvider");
            account.public().expect("Cannot get public from account")
        };
        self.account_provider = ap;
        self.signer = Some((address, public));
        self.decrypted_account = None;
        cinfo!(ENGINE, "Setting Engine signer to {}", address);
    }

    // TODO: remove decrypted_account after some timeout
    pub fn set_to_keep_decrypted_account(&mut self, ap: Arc<AccountProvider>, address: Address) {
        let account =
            ap.get_unlocked_account(&address).expect("The address must be registered in AccountProvider").disclose();
        let public = account.public().expect("Cannot get public from account");

        self.account_provider = ap;
        self.signer = Some((address, public));
        self.decrypted_account = Some(account);
        cinfo!(ENGINE, "Setting Engine signer to {} (retaining)", address);
    }

    /// Sign a consensus message hash.
    pub fn sign(&self, hash: H256) -> Result<SchnorrSignature, AccountProviderError> {
        let address = self.signer.map(|(address, _public)| address).unwrap_or_else(Default::default);
        let result = match &self.decrypted_account {
            Some(account) => account.sign_schnorr(&hash)?,
            None => {
                let account = self.account_provider.get_unlocked_account(&address)?;
                account.sign_schnorr(&hash)?
            }
        };
        Ok(result)
    }

    /// Generate a vrf random hash.
    pub fn vrf_hash(&self, hash: H256, vrf_inst: &mut ECVRF) -> Result<Vec<u8>, AccountProviderError> {
        Ok(match &self.decrypted_account {
            Some(account) => account.vrf_hash(&hash, vrf_inst)?,
            None => {
                let address = self.signer.map(|(address, _)| address).unwrap_or_default();
                self.account_provider
                    .get_unlocked_account(&address)
                    .and_then(|account| account.vrf_hash(&hash, vrf_inst).map_err(From::from))?
            }
        })
    }

    /// Sign a message hash with ECDSA.
    pub fn sign_ecdsa(&self, hash: H256) -> Result<Signature, AccountProviderError> {
        let address = self.signer.map(|(address, _public)| address).unwrap_or_else(Default::default);
        let result = match &self.decrypted_account {
            Some(account) => account.sign(&hash)?,
            None => {
                let account = self.account_provider.get_unlocked_account(&address)?;
                account.sign(&hash)?
            }
        };
        Ok(result)
    }

    /// Public Key of signer.
    pub fn public(&self) -> Option<&Public> {
        self.signer.as_ref().map(|(_address, public)| public)
    }

    /// Address of signer.
    pub fn address(&self) -> Option<&Address> {
        self.signer.as_ref().map(|(address, _)| address)
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
