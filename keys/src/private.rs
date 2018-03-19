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
use std::ops::Deref;
use std::str::FromStr;

use codechain_types::H256;
use hex::ToHex;
use secp256k1::key;

use super::Error;

#[derive(Clone, PartialEq, Eq)]
pub struct Private(H256);

impl Private {
    pub fn from_slice(key: &[u8]) -> Self {
        assert_eq!(32, key.len(), "Caller should provide 32-byte length slice");

        let mut h = H256::default();
        h.copy_from_slice(&key[0..32]);
        Private(h)
    }
}

impl ToHex for Private {
    fn to_hex(&self) -> String {
        self.0.to_hex()
    }
}

impl fmt::Debug for Private {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Secret: 0x{:x}{:x}..{:x}{:x}", self.0[0], self.0[1], self.0[30], self.0[31])
    }
}

impl From<H256> for Private {
    fn from(s: H256) -> Self {
        Private::from_slice(&s)
    }
}

impl FromStr for Private {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(H256::from_str(s).map_err(|e| Error::Custom(format!("{:?}", e)))?.into())
    }
}

impl From<&'static str> for Private {
    fn from(s: &'static str) -> Self {
        s.parse().expect(&format!("invalid string literal for {}: '{}'", stringify!(Self), s))
    }
}

impl From<key::SecretKey> for Private {
    fn from(key: key::SecretKey) -> Self {
        Self::from_slice(&key[0..32])
    }
}

impl Deref for Private {
    type Target = H256;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

