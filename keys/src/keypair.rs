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
use secp256k1::key;
use codechain_types::{H264, H520};
use network::Network;
use {Public, Error, SECP256K1, Address, Private, Secret};

pub struct KeyPair {
    private: Private,
    public: Public,
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(self.private.fmt(f));
        writeln!(f, "public: {:?}", self.public)
    }
}

impl fmt::Display for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "private: {}", self.private)?;
        writeln!(f, "public: {}", self.public)
    }
}

impl KeyPair {
    pub fn private(&self) -> &Private {
        &self.private
    }

    pub fn public(&self) -> &Public {
        &self.public
    }

    pub fn from_private(private: Private) -> Result<KeyPair, Error> {
        let context = &SECP256K1;
        let s: key::SecretKey = try!(key::SecretKey::from_slice(context, &*private.secret));
        let pub_key = try!(key::PublicKey::from_secret_key(context, &s));
        let serialized = pub_key.serialize_vec(context, private.compressed);

        let public = if private.compressed {
            let mut public = H264::default();
            public.copy_from_slice(&serialized[0..33]);
            Public::Compressed(public)
        } else {
            let mut public = H520::default();
            public.copy_from_slice(&serialized[0..65]);
            Public::Normal(public)
        };

        let keypair = KeyPair {
            private: private,
            public: public,
        };

        Ok(keypair)
    }

    pub fn from_keypair(sec: key::SecretKey, public: key::PublicKey, network: Network) -> Self {
        let context = &SECP256K1;
        let serialized = public.serialize_vec(context, false);
        let mut secret = Secret::default();
        secret.copy_from_slice(&sec[0..32]);
        let mut public = H520::default();
        public.copy_from_slice(&serialized[0..65]);

        KeyPair {
            private: Private {
                network: network,
                secret: secret,
                compressed: false,
            },
            public: Public::Normal(public),
        }
    }

    pub fn address(&self) -> Address {
        Address {
            network: self.private.network,
            version: 1,
            account_id: self.public.account_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crypto::blake256;
    use Public;
    use super::KeyPair;

    /// Tests from:
/// https://github.com/bitcoin/bitcoin/blob/a6a860796a44a2805a58391a009ba22752f64e32/src/test/key_tests.cpp
    const SECRET_0: &'static str = "5KSCKP8NUyBZPCCQusxRwgmz9sfvJQEgbGukmmHepWw5Bzp95mu";
    const SECRET_1: &'static str = "5HxWvvfubhXpYYpS3tJkw6fq9jE9j18THftkZjHHfmFiWtmAbrj";
    const SECRET_2: &'static str = "5KC4ejrDjv152FGwP386VD1i2NYc5KkfSMyv1nGy1VGDxGHqVY3";
    const SECRET_1C: &'static str = "Kwr371tjA9u2rFSMZjTNun2PXXP3WPZu2afRHTcta6KxEUdm1vEw";
    const SECRET_2C: &'static str = "L3Hq7a8FEQwJkW1M2GNKDW28546Vp5miewcCzSqUD9kCAXrJdS3g";
    const ADDRESS_0: &'static str = "16meyfSoQV6twkAAxPe51RtMVz7PGRmWna";
    const ADDRESS_1: &'static str = "1QFqqMUD55ZV3PJEJZtaKCsQmjLT6JkjvJ";
    const ADDRESS_2: &'static str = "1F5y5E5FMc5YzdJtB9hLaUe43GDxEKXENJ";
    const ADDRESS_1C: &'static str = "1NoJrossxPBKfCHuJXT4HadJrXRE9Fxiqs";
    const ADDRESS_2C: &'static str = "1CRj2HyM1CXWzHAXLQtiGLyggNT9WQqsDs";

    fn check_addresses(secret: &'static str, address: &'static str) -> bool {
        let kp = KeyPair::from_private(secret.into()).unwrap();
        kp.address() == address.into()
    }

    fn check_compressed(secret: &'static str, compressed: bool) -> bool {
        let kp = KeyPair::from_private(secret.into()).unwrap();
        kp.private().compressed == compressed
    }

    #[test] #[ignore]
    fn test_keypair_address() {
        assert!(check_addresses(SECRET_0, ADDRESS_0));
        assert!(check_addresses(SECRET_1, ADDRESS_1));
        assert!(check_addresses(SECRET_2, ADDRESS_2));
        assert!(check_addresses(SECRET_1C, ADDRESS_1C));
        assert!(check_addresses(SECRET_2C, ADDRESS_2C));
    }

    #[test] #[ignore]
    fn test_keypair_is_compressed() {
        assert!(check_compressed(SECRET_0, false));
        assert!(check_compressed(SECRET_1, false));
        assert!(check_compressed(SECRET_2, false));
        assert!(check_compressed(SECRET_1C, true));
        assert!(check_compressed(SECRET_2C, true));
    }
}
