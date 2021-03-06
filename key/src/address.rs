// Copyright 2018-2019 Kodebox, Inc.
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

use primitives::{remove_0x_prefix, H160};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use std::cmp;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

#[derive(Clone, Copy, Default, Debug, Deserialize, Serialize, Eq)]
pub struct Address(H160);

impl Address {
    pub fn random() -> Self {
        Address(H160::random())
    }
}

impl Deref for Address {
    type Target = H160;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Address {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0)
    }
}

impl PartialOrd for Address {
    fn partial_cmp(&self, m: &Address) -> Option<cmp::Ordering> {
        self.0.partial_cmp(&m.0)
    }
}

impl Ord for Address {
    fn cmp(&self, m: &Address) -> cmp::Ordering {
        self.0.cmp(&m.0)
    }
}

impl PartialEq for Address {
    fn eq(&self, m: &Address) -> bool {
        self.0.eq(&m.0)
    }
}

impl Hash for Address {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Encodable for Address {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.0.rlp_append(s);
    }
}

impl Decodable for Address {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let data = H160::decode(rlp)?;
        Ok(Address(data))
    }
}

impl FromStr for Address {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = remove_0x_prefix(s);
        let a = H160::from_str(s).map_err(|_| format!("Invalid address {}", s))?;
        Ok(Address(a))
    }
}

impl From<H160> for Address {
    fn from(s: H160) -> Self {
        Address(s)
    }
}

impl From<u64> for Address {
    fn from(s: u64) -> Self {
        Address(H160::from(s))
    }
}

impl From<[u8; 20]> for Address {
    fn from(s: [u8; 20]) -> Self {
        Address(H160::from(s))
    }
}

impl From<&'static str> for Address {
    fn from(s: &'static str) -> Self {
        s.parse().unwrap_or_else(|_| panic!("invalid string literal for {}: '{}'", stringify!(Self), s))
    }
}

impl From<Address> for [u8; 20] {
    fn from(a: Address) -> Self {
        a.0.into()
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn rlp_default() {
        rlp_encode_and_decode_test!(Address::default());
    }

    #[test]
    fn rlp() {
        rlp_encode_and_decode_test!(Address::from("abcdef124567890abcdef124567890abcdef1245"));
    }

    #[test]
    fn rlp_random() {
        rlp_encode_and_decode_test!(Address::random());
    }
}
