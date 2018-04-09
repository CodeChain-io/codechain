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

use ccrypto::aes::{self, SymmetricCipherError};
use ccrypto::blake256_with_key;
use ctypes::hash::{H128, H256};
use ctypes::Secret;

pub type Nonce = u32;
type IV = H128;

#[derive(Clone, Debug, Hash, Eq, PartialOrd, PartialEq)]
pub struct Session {
    secret: Secret,
    nonce: Nonce,
}

type Error = SymmetricCipherError;

impl Session {
    pub fn new_with_zero_nonce(secret: Secret) -> Self {
        Self::new(secret, 0)
    }

    pub fn new(secret: Secret, nonce: Nonce) -> Self {
        Session {
            secret,
            nonce,
        }
    }

    pub fn is_expected_nonce(&self, nonce: &Nonce) -> bool {
        self.nonce() == nonce
    }

    pub fn secret(&self) -> &Secret {
        &self.secret
    }

    pub fn nonce(&self) -> &Nonce {
        &self.nonce
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        let iv = self.initialization_vector();
        Ok(aes::encrypt(&data, &self.secret, &iv)?)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        let iv = self.initialization_vector();
        Ok(aes::decrypt(&data, &self.secret, &iv)?)
    }

    pub fn sign(&self, data: &[u8]) -> H256 {
        let iv = self.initialization_vector();
        blake256_with_key(data, &iv)
    }

    pub fn initialization_vector(&self) -> H128 {
        // FIXME: This implementation is so naive.
        let mut iv: IV = IV::zero();
        iv[0] = (self.nonce & 0xFF) as u8;
        iv[3] = ((self.nonce >> 8) & 0xFF) as u8;
        iv[7] = ((self.nonce >> 16) & 0xFF) as u8;
        iv[13] = ((self.nonce >> 24) & 0xFF) as u8;
        iv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_and_decrypt_short_data() {
        let secret = Secret::random();
        const NONCE: Nonce = 1000;

        let session = Session::new(secret, NONCE);

        let data = Vec::from("some short data".as_bytes());

        let encrypted = session.encrypt(&data).ok().unwrap();
        let decrypted = session.decrypt(&encrypted).ok().unwrap();

        assert_eq!(data.len(), decrypted.len());
        assert_eq!(data, decrypted);
    }

    #[test]
    fn encrypt_and_decrypt_short_data_in_different_session_with_same_secret() {
        let secret = Secret::random();
        const NONCE: Nonce = 1000;

        let session1 = Session::new(secret, NONCE);
        let session2 = Session::new(secret, NONCE);

        let data = Vec::from("some short data".as_bytes());

        let encrypted = session1.encrypt(&data).ok().unwrap();
        let decrypted = session2.decrypt(&encrypted).ok().unwrap();

        assert_eq!(data.len(), decrypted.len());
        assert_eq!(data, decrypted);
    }

    #[test]
    fn encrypt_with_different_nonce() {
        let secret = Secret::random();
        const NONCE1: Nonce = 1000;
        const NONCE2: Nonce = 1001;

        let session1 = Session::new(secret, NONCE1);
        let session2 = Session::new(secret, NONCE2);

        let data = Vec::from("some short data".as_bytes());
        let encrypted1 = session1.encrypt(&data).ok().unwrap();
        let encrypted2 = session2.encrypt(&data).ok().unwrap();

        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn encrypt_with_different_secret() {
        let secret1 = Secret::random();
        let secret2 = Secret::random();
        debug_assert_ne!(secret1, secret2);
        const NONCE: Nonce = 1000;

        let session1 = Session::new(secret1, NONCE);
        let session2 = Session::new(secret2, NONCE);

        let data = Vec::from("some short data".as_bytes());
        let encrypted1 = session1.encrypt(&data).ok().unwrap();
        let encrypted2 = session2.encrypt(&data).ok().unwrap();

        assert_ne!(encrypted1, encrypted2);
    }
}
