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

use std::fmt;
use std::sync::Arc;

use ckeys::{
    public_to_address, Error as KeysError, Generator, KeyPair, Message, Private, Public, Random, SchnorrSignature,
};
use ckeystore::accounts_dir::MemoryDirectory;
use ckeystore::{Error as KeystoreError, KeyStore, SimpleSecretStore};
use ctypes::Address;
use parking_lot::RwLock;

/// Signing error
#[derive(Debug)]
pub enum SignError {
    /// Account does not exist.
    NotFound,
    /// Key error.
    KeysError(KeysError),
    /// Keystore error.
    KeystoreError(KeystoreError),
    /// Inappropriate chain
    InappropriateChain,
}

impl From<KeysError> for SignError {
    fn from(e: KeysError) -> Self {
        SignError::KeysError(e)
    }
}

impl From<KeystoreError> for SignError {
    fn from(e: KeystoreError) -> Self {
        SignError::KeystoreError(e)
    }
}

impl fmt::Display for SignError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            SignError::NotFound => write!(f, "Account does not exist"),
            SignError::KeysError(e) => write!(f, "{}", e),
            SignError::KeystoreError(e) => write!(f, "{}", e),
            SignError::InappropriateChain => write!(f, "Inappropriate chain"),
        }
    }
}

pub struct AccountProvider {
    keystore: RwLock<KeyStore>,
}

impl AccountProvider {
    pub fn new(keystore: KeyStore) -> Arc<Self> {
        Arc::new(Self {
            keystore: RwLock::new(keystore),
        })
    }

    /// Creates not disk backed provider.
    pub fn transient_provider() -> Arc<Self> {
        Arc::new(Self {
            keystore: RwLock::new(KeyStore::open(Box::new(MemoryDirectory::default())).unwrap()),
        })
    }

    pub fn new_account_and_public(&self) -> Result<(Address, Public), SignError> {
        let acc = Random.generate().expect("secp context has generation capabilities; qed");
        let private = acc.private().clone();
        let public = acc.public().clone();
        let address = public_to_address(&public);
        self.keystore.write().insert_account(*private, "password")?;
        Ok((address, public))
    }

    pub fn insert_account(&self, private: Private) -> Result<Address, SignError> {
        let acc = KeyPair::from_private(private)?;
        let private = acc.private().clone();
        let public = acc.public().clone();
        let address = public_to_address(&public);
        self.keystore.write().insert_account(*private, "password")?;
        Ok(address)
    }

    pub fn sign(&self, address: Address, message: Message) -> Result<SchnorrSignature, SignError> {
        let signature = self.keystore.read().sign(&address, "password", &message)?;
        Ok(signature)
    }

    pub fn has_account(&self, address: Address) -> Result<bool, SignError> {
        let has = self.keystore.read().has_account(&address)?;
        Ok(has)
    }

    pub fn get_list(&self) -> Result<Vec<Address>, SignError> {
        let addresses = self.keystore.read().accounts()?;
        Ok(addresses)
    }
}
