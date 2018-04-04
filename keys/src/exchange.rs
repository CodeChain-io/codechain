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

use std::result;

use secp256k1::{ecdh, key};

use super::{Error, Private, Public, SECP256K1, Secret};

pub fn exchange(public: &Public, private: &Private) -> result::Result<Secret, Error> {
    let public = {
        let mut public_buffer = [4u8; 65];
        (&mut public_buffer[1..65]).copy_from_slice(&public[0..64]);
        public_buffer
    };

    let public = key::PublicKey::from_slice(&SECP256K1, &public)?;
    let private = key::SecretKey::from_slice(&SECP256K1, &private)?;
    let shared = ecdh::SharedSecret::new_raw(&SECP256K1, &public, &private);

    Ok(Secret::from(&shared[0..32]))
}

#[cfg(test)]
mod tests {
    use super::exchange;
    use super::super::{Generator, KeyPair, Random};

    #[test]
    fn test_exchange_makes_same_private_key() {
        let k1: KeyPair = Random.generate().unwrap();
        let k2 = {
            let mut k2: KeyPair = Random.generate().unwrap();
            while k1 == k2 {
                k2 = Random.generate().unwrap();
            }
            k2
        };
        assert_ne!(k1, k2);

        let s1 = exchange(&k2.public(), &k1.private()).unwrap();
        let s2 = exchange(&k1.public(), &k2.private()).unwrap();
        assert_eq!(s1, s2);
    }
}
