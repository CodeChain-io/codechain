// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

use super::crypto::Crypto;
use account::Version;
use ccrypto;
use ckey::{sign, Address, KeyPair, Message, Password, Public, Signature};
use {json, Error};

/// Account representation.
#[derive(Debug, PartialEq, Clone)]
pub struct SafeAccount {
    /// Account ID
    pub id: [u8; 16],
    /// Account version
    pub version: Version,
    /// Account address
    pub address: Address,
    /// Account private key derivation definition.
    pub crypto: Crypto,
    /// Account filename
    pub filename: Option<String>,
    /// Account name
    pub name: String,
    /// Account metadata
    pub meta: String,
}

impl Into<json::KeyFile> for SafeAccount {
    fn into(self) -> json::KeyFile {
        json::KeyFile {
            id: From::from(self.id),
            version: self.version.into(),
            address: self.address.into(),
            crypto: self.crypto.into(),
            name: Some(self.name.into()),
            meta: Some(self.meta.into()),
        }
    }
}

impl SafeAccount {
    /// Create a new account
    pub fn create(
        keypair: &KeyPair,
        id: [u8; 16],
        password: &Password,
        iterations: u32,
        name: String,
        meta: String,
    ) -> Result<Self, ccrypto::Error> {
        Ok(SafeAccount {
            id,
            version: Version::V1,
            crypto: Crypto::with_secret(keypair.private(), password, iterations)?,
            address: keypair.address(),
            filename: None,
            name,
            meta,
        })
    }

    /// Create a new `SafeAccount` from the given `json`; if it was read from a
    /// file, the `filename` should be `Some` name. If it is as yet anonymous, then it
    /// can be left `None`.
    pub fn from_file(json: json::KeyFile, filename: Option<String>) -> Self {
        SafeAccount {
            id: json.id.into(),
            version: json.version.into(),
            address: json.address.into(),
            crypto: json.crypto.into(),
            filename,
            name: json.name.unwrap_or(String::new()),
            meta: json.meta.unwrap_or("{}".to_string()),
        }
    }

    /// Sign a message.
    pub fn sign(&self, password: &Password, message: &Message) -> Result<Signature, Error> {
        let secret = self.crypto.secret(password)?;
        sign(&secret.into(), message).map_err(From::from)
    }

    /// Derive public key.
    pub fn public(&self, password: &Password) -> Result<Public, Error> {
        let secret = self.crypto.secret(password)?;
        Ok(KeyPair::from_private(secret.into())?.public().clone())
    }

    /// Change account's password.
    pub fn change_password(
        &self,
        old_password: &Password,
        new_password: &Password,
        iterations: u32,
    ) -> Result<Self, Error> {
        let secret = self.crypto.secret(old_password)?;
        let result = SafeAccount {
            id: self.id.clone(),
            version: self.version.clone(),
            crypto: Crypto::with_secret(&secret, new_password, iterations)?,
            address: self.address,
            filename: self.filename.clone(),
            name: self.name.clone(),
            meta: self.meta.clone(),
        };
        Ok(result)
    }

    /// Check if password matches the account.
    pub fn check_password(&self, password: &Password) -> bool {
        self.crypto.secret(password).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::SafeAccount;
    use ckey::{verify, Generator, Message, Random};

    #[test]
    fn sign_and_verify_public() {
        let keypair = Random.generate().unwrap();
        let password = &"hello world".into();
        let message = Message::default();
        let account = SafeAccount::create(&keypair, [0u8; 16], password, 10240, "Test".to_string(), "{}".to_string());
        let signature = account.unwrap().sign(password, &message).unwrap();
        assert!(verify(keypair.public(), &signature, &message).unwrap());
    }

    #[test]
    fn change_password() {
        let keypair = Random.generate().unwrap();
        let first_password = &"hello world".into();
        let sec_password = &"this is sparta".into();
        let i = 10240;
        let message = Message::default();
        let account =
            SafeAccount::create(&keypair, [0u8; 16], first_password, i, "Test".to_string(), "{}".to_string()).unwrap();
        let new_account = account.change_password(first_password, sec_password, i).unwrap();
        assert!(account.sign(first_password, &message).is_ok());
        assert!(account.sign(sec_password, &message).is_err());
        assert!(new_account.sign(first_password, &message).is_err());
        assert!(new_account.sign(sec_password, &message).is_ok());
    }
}
