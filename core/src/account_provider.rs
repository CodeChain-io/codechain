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

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use ckeys::{
    public_to_address, sign_ecdsa, ECDSASignature, Error as KeysError, Generator, KeyPair, Message, Private, Public,
    Random,
};
use ctypes::Address;
use parking_lot::RwLock;

/// Signing error
#[derive(Debug)]
pub enum SignError {
    /// Account does not exist.
    NotFound,
    /// Key error.
    KeysError(KeysError),
    /// Inappropriate chain
    InappropriateChain,
}

impl From<KeysError> for SignError {
    fn from(e: KeysError) -> Self {
        SignError::KeysError(e)
    }
}

impl fmt::Display for SignError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            SignError::NotFound => write!(f, "Account does not exist"),
            SignError::KeysError(e) => write!(f, "{}", e),
            SignError::InappropriateChain => write!(f, "Inappropriate chain"),
        }
    }
}

pub struct AccountProvider {
    secrets: RwLock<HashMap<Address, Private>>,
}

impl AccountProvider {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            secrets: RwLock::new(HashMap::new()),
        })
    }

    pub fn new_account_and_public(&self) -> (Address, Public) {
        let acc = Random.generate().expect("secp context has generation capabilities; qed");
        let private = acc.private().clone();
        let public = acc.public().clone();
        let address = public_to_address(&public);
        self.secrets.write().insert(address, private);
        (address, public)
    }

    pub fn insert_account(&self, private: Private) -> Result<Address, KeysError> {
        let acc = KeyPair::from_private(private)?;
        let private = acc.private().clone();
        let public = acc.public().clone();
        let address = public_to_address(&public);
        self.secrets.write().insert(address, private);
        Ok(address)
    }

    pub fn sign(&self, address: Address, message: Message) -> Result<ECDSASignature, SignError> {
        if let Some(private) = self.secrets.read().get(&address) {
            sign_ecdsa(private, &message).map_err(Into::into)
        } else {
            Err(SignError::NotFound)
        }
    }

    pub fn has_account(&self, address: Address) -> bool {
        self.secrets.read().contains_key(&address)
    }
}
