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

use ccrypto;
use ckey::{sign, Address, KeyPair, Message, Password, Public, Signature};

use super::crypto::Crypto;
use crate::account::Version;
use crate::{json, Error};

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
    /// Account metadata
    pub meta: String,
}

impl From<SafeAccount> for json::KeyFile {
    fn from(account: SafeAccount) -> Self {
        Self {
            id: From::from(account.id),
            version: account.version.into(),
            address: Some(account.address.into()),
            crypto: account.crypto.into(),
            meta: Some(account.meta.into()),
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
        meta: String,
    ) -> Result<Self, ccrypto::Error> {
        Ok(SafeAccount {
            id,
            version: Version::V3,
            crypto: Crypto::with_secret(keypair.private(), password, iterations)?,
            address: keypair.address(),
            filename: None,
            meta,
        })
    }

    /// Create a new `SafeAccount` from the given `json`; if it was read from a
    /// file, the `filename` should be `Some` name. If it is as yet anonymous, then it
    /// can be left `None`.
    pub fn from_file(
        json: json::KeyFile,
        filename: Option<String>,
        password: Option<&Password>,
    ) -> Result<Self, Error> {
        let crypto = Crypto::from(json.crypto);
        let address = match (json.address, password) {
            (Some(address), Some(password)) => {
                let address = address.into();
                let decrypted_address = crypto.address(password)?;
                if decrypted_address != address {
                    Err(Error::InvalidKeyFile("Address field is invalid".to_string()))
                } else {
                    Ok(address)
                }
            }
            (None, Some(password)) => crypto.address(password),
            (Some(address), None) => Ok(address.into()),
            (None, None) => Err(Error::InvalidKeyFile("Cannot create account if address is not given".to_string())),
        }?;

        Ok(SafeAccount {
            id: json.id.into(),
            version: json.version.into(),
            address,
            crypto,
            filename,
            meta: json.meta.unwrap_or("{}".to_string()),
        })
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
    use std::str::FromStr;

    use ckey::{verify, Generator, Random};
    use crate::json;
    use crate::json::{Aes128Ctr, Cipher, Crypto, Kdf, KeyFile, Scrypt, Uuid};

    use super::*;

    #[test]
    fn sign_and_verify_public() {
        let keypair = Random.generate().unwrap();
        let password = &"hello world".into();
        let message = Message::default();
        let account = SafeAccount::create(&keypair, [0u8; 16], password, 10240, "{\"name\":\"Test\"}".to_string());
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
            SafeAccount::create(&keypair, [0u8; 16], first_password, i, "{\"name\":\"Test\"}".to_string()).unwrap();
        let new_account = account.change_password(first_password, sec_password, i).unwrap();
        assert!(account.sign(first_password, &message).is_ok());
        assert!(account.sign(sec_password, &message).is_err());
        assert!(new_account.sign(first_password, &message).is_err());
        assert!(new_account.sign(sec_password, &message).is_ok());
    }

    #[test]
    fn from_file() {
        let address = "6edddfc6349aff20bc6467ccf276c5b52487f7a8";
        let meta = "{\"a\": \"b\"}";
        let expected = KeyFile {
            id: Uuid::from_str("8777d9f6-7860-4b9b-88b7-0b57ee6b3a73").unwrap(),
            version: json::Version::V3,
            address: Some(address.into()),
            crypto: Crypto {
                cipher: Cipher::Aes128Ctr(Aes128Ctr {
                    iv: "b5a7ec855ec9e2c405371356855fec83".into(),
                }),
                ciphertext: "7203da0676d141b138cd7f8e1a4365f59cc1aa6978dc5443f364ca943d7cb4bc".into(),
                kdf: Kdf::Scrypt(Scrypt {
                    n: 262144,
                    dklen: 32,
                    p: 1,
                    r: 8,
                    salt: "1e8642fdf1f87172492c1412fc62f8db75d796cdfa9c53c3f2b11e44a2a1b209".into(),
                }),
                mac: "46325c5d4e8c991ad2683d525c7854da387138b6ca45068985aa4959fa2b8c8f".into(),
            },
            meta: Some(meta.to_string()),
        };

        let safe_account = SafeAccount::from_file(expected, None, None).unwrap();
        assert_eq!(Address::from_str(address), Ok(safe_account.address));
        assert_eq!(meta, safe_account.meta);
    }
}
