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

use std::error;
use std::fmt;

use ccrypto::aes;
use ccrypto::blake256_with_key;
use ctypes::Secret;
use ctypes::hash::{H128, H256};
use rcrypto::symmetriccipher::SymmetricCipherError;

pub type SharedSecret = Secret;
pub type Nonce = u32;
type IV = H128;

#[derive(Clone)]
pub struct Session {
    secret: SharedSecret,
    nonce: Option<Nonce>,
}

#[derive(Debug)]
pub enum Error {
    CryptoError(SymmetricCipherError),
    NotReady,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::CryptoError(ref err) => write!(f, "CryptoError {:?}", err),
            &Error::NotReady => write!(f, "NotReady"),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            &Error::CryptoError(SymmetricCipherError::InvalidLength) => "Invalid length",
            &Error::CryptoError(SymmetricCipherError::InvalidPadding) => "Invalid padding",
            &Error::NotReady => "Not ready",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &Error::CryptoError(_) => None,
            &Error::NotReady => None,
        }
    }
}

impl From<SymmetricCipherError> for Error {
    fn from(err: SymmetricCipherError) -> Error {
        Error::CryptoError(err)
    }
}

impl Session {
    pub fn new(secret: SharedSecret) -> Self {
        Session {
            secret,
            nonce: None,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.nonce.is_some()
    }

    pub fn set_ready(&mut self, nonce: Nonce) {
        debug_assert!(!self.is_ready());
        self.nonce = Some(nonce);
    }

    pub fn is_expected_nonce(&self, nonce: &Nonce) -> bool {
        self.is_ready() && self.nonce == Some(*nonce)
    }

    pub fn secret(&self) -> &SharedSecret {
        &self.secret
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        if !self.is_ready() {
            return Err(Error::NotReady)
        }
        if let Some(iv) = self.initialization_vector() {
            Ok(aes::encrypt(&data, &self.secret, &iv)?)
        } else {
            Err(Error::NotReady)
        }
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        if !self.is_ready() {
            return Err(Error::NotReady)
        }
        if let Some(iv) = self.initialization_vector() {
            Ok(aes::decrypt(&data, &self.secret, &iv)?)
        } else {
            Err(Error::NotReady)
        }
    }

    pub fn sign(&self, data: &[u8]) -> Option<H256> {
        self.initialization_vector()
            .map(|iv| blake256_with_key(data, &iv))
    }

    fn initialization_vector(&self) -> Option<H128> {
        self.nonce.map(|nonce| {
            // FIXME: This implementation is so naive.
            let mut iv: IV = IV::zero();
            iv[0] = (nonce & 0xFF) as u8;
            iv[3] = ((nonce >> 8) & 0xFF) as u8;
            iv[7] = ((nonce >> 16) & 0xFF) as u8;
            iv[13] = ((nonce >> 24) & 0xFF) as u8;
            iv
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_is_not_ready() {
        let secret = SharedSecret::random();
        let session = Session::new(secret);

        assert!(!session.is_ready());
    }

    #[test]
    fn ready_with_nonce() {
        let secret = SharedSecret::random();
        let mut session = Session::new(secret);

        assert!(!session.is_ready());

        const NONCE: Nonce = 1000;
        session.set_ready(NONCE);

        assert!(session.is_ready());

        assert!(session.is_expected_nonce(&NONCE));
    }

    #[test]
    fn is_expected_nonce_must_return_false_on_new_session() {
        let secret = SharedSecret::random();
        let session = Session::new(secret);

        assert!(!session.is_ready());
        assert!(!session.is_expected_nonce(&10000));
    }

    #[test]
    fn encrypt_and_decrypt_short_data() {
        let secret = SharedSecret::random();
        const NONCE: Nonce = 1000;

        let mut session = Session::new(secret);
        session.set_ready(NONCE);

        let data = Vec::from("some short data".as_bytes());

        let encrypted = session.encrypt(&data).ok().unwrap();
        let decrypted = session.decrypt(&encrypted).ok().unwrap();

        assert_eq!(data.len(), decrypted.len());
        assert_eq!(data, decrypted);
    }

    #[test]
    fn encrypt_and_decrypt_short_data_in_different_session_with_same_secret() {
        let secret = SharedSecret::random();
        const NONCE: Nonce = 1000;

        let mut session1 = Session::new(secret);
        session1.set_ready(NONCE);

        let mut session2 = Session::new(secret);
        session2.set_ready(NONCE);

        let data = Vec::from("some short data".as_bytes());

        let encrypted = session1.encrypt(&data).ok().unwrap();
        let decrypted = session2.decrypt(&encrypted).ok().unwrap();

        assert_eq!(data.len(), decrypted.len());
        assert_eq!(data, decrypted);
    }

    #[test]
    fn encrypt_with_different_nonce() {
        let secret = SharedSecret::random();
        const NONCE1: Nonce = 1000;
        const NONCE2: Nonce = 1001;

        let mut session1 = Session::new(secret);
        session1.set_ready(NONCE1);

        let mut session2 = Session::new(secret);
        session2.set_ready(NONCE2);

        let data = Vec::from("some short data".as_bytes());
        let encrypted1 = session1.encrypt(&data).ok().unwrap();
        let encrypted2 = session2.encrypt(&data).ok().unwrap();

        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn encrypt_with_different_secret() {
        let secret1 = SharedSecret::random();
        let secret2 = SharedSecret::random();
        debug_assert_ne!(secret1, secret2);
        const NONCE: Nonce = 1000;

        let mut session1 = Session::new(secret1);
        session1.set_ready(NONCE);

        let mut session2 = Session::new(secret2);
        session2.set_ready(NONCE);

        let data = Vec::from("some short data".as_bytes());
        let encrypted1 = session1.encrypt(&data).ok().unwrap();
        let encrypted2 = session2.encrypt(&data).ok().unwrap();

        assert_ne!(encrypted1, encrypted2);
    }
}
