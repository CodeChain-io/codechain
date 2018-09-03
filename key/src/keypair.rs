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

use crypto::Blake;
use primitives::H160;
use rustc_hex::ToHex;
use secp256k1::key;

use super::{Address, Error, Private, Public, SECP256K1};

pub fn public_to_address(public: &Public) -> Address {
    H160::blake(public).into()
}

#[derive(Debug, Clone, PartialEq)]
/// secp256k1 key pair
pub struct KeyPair {
    private: Private,
    public: Public,
}

impl fmt::Display for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "secret:  {}", self.private.to_hex())?;
        writeln!(f, "public:  {}", self.public.to_hex())?;
        write!(f, "address: {}", self.address().to_hex())
    }
}

impl KeyPair {
    /// Create a pair from secret key
    pub fn from_private(private: Private) -> Result<KeyPair, Error> {
        let context = &SECP256K1;
        let s: key::SecretKey = key::SecretKey::from_slice(context, &private[..])?;
        let pub_key = key::PublicKey::from_secret_key(context, &s)?;
        let serialized = pub_key.serialize_vec(context, false);

        let mut public = Public::default();
        public.copy_from_slice(&serialized[1..65]);

        let keypair = KeyPair {
            private,
            public,
        };

        Ok(keypair)
    }

    pub fn from_keypair(sec: key::SecretKey, publ: key::PublicKey) -> Self {
        let context = &SECP256K1;
        let serialized = publ.serialize_vec(context, false);
        let private = Private::from(sec);
        let mut public = Public::default();
        public.copy_from_slice(&serialized[1..65]);

        KeyPair {
            private,
            public,
        }
    }

    pub fn private(&self) -> &Private {
        &self.private
    }

    pub fn public(&self) -> &Public {
        &self.public
    }

    pub fn address(&self) -> Address {
        public_to_address(&self.public)
    }
}
