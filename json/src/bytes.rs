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

//! Lenient bytes json deserialization for test json files.

use rustc_hex::{FromHex, FromHexError, ToHex};
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

/// Lenient bytes json deserialization for test json files.
#[derive(Default, Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    /// Creates bytes struct.
    pub fn new(v: Vec<u8>) -> Self {
        Bytes(v)
    }

    /// Convert back to vector
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }

    pub fn without_prefix(&self) -> BytesWithoutPrefix {
        BytesWithoutPrefix(self)
    }
}

impl<'a> From<&'a str> for Bytes {
    fn from(s: &'a str) -> Self {
        FromStr::from_str(s).unwrap_or_else(|_| panic!("invalid string literal for {}: '{}'", stringify!(Self), s))
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        Bytes(bytes)
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(b: Bytes) -> Self {
        b.0
    }
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}


impl FromStr for Bytes {
    type Err = FromHexError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = if value.starts_with("0x") {
            &value[2..]
        } else {
            value
        };
        match FromHex::from_hex(value) {
            Ok(bytes) => Ok(Bytes(bytes)),
            Err(FromHexError::InvalidHexLength) => {
                let zero_padded = format!("0{}", value);
                FromHex::from_hex(zero_padded.as_str()).map(Bytes)
            }
            Err(e) => Err(e),
        }
    }
}

impl<'a> Deserialize<'a> for Bytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>, {
        deserializer.deserialize_any(BytesVisitor)
    }
}

struct BytesVisitor;

impl<'a> Visitor<'a> for BytesVisitor {
    type Value = Bytes;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a hex encoded string of bytes")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: Error, {
        Bytes::from_str(value).map_err(E::custom)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: Error, {
        self.visit_str(value.as_ref())
    }
}

impl Serialize for Bytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        let mut serialized = "0x".to_string();
        serialized.push_str(self.0.to_hex().as_ref());
        serializer.serialize_str(serialized.as_ref())
    }
}

pub struct BytesWithoutPrefix<'a>(&'a Bytes);

impl<'a> Serialize for BytesWithoutPrefix<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        let BytesWithoutPrefix(Bytes(vec)) = self;
        serializer.serialize_str(vec.to_hex().as_ref())
    }
}

#[cfg(test)]
mod test {
    use crate::bytes::Bytes;
    use serde_json;
    use std::result::Result;

    macro_rules! assert_ok {
        ($test_str: expr, $expected: expr) => {{
            let res: Result<Bytes, _> = serde_json::from_str($test_str);
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), $expected);
        }};
    }

    macro_rules! assert_err {
        ($test_str: expr) => {{
            let res: Result<Bytes, _> = serde_json::from_str($test_str);
            assert!(res.is_err());
        }};
    }

    #[test]
    fn bytes_deserialization() {
        // Hexadecimal
        assert_ok!(r#""0x12""#, Bytes(vec![0x12]));
        assert_ok!(r#""0x0123""#, Bytes(vec![0x01, 0x23]));
        assert_ok!(r#""1234""#, Bytes(vec![0x12, 0x34]));

        // can handle odd digits
        assert_ok!(r#""0x123""#, Bytes(vec![0x01, 0x23]));
        assert_ok!(r#""123""#, Bytes(vec![0x01, 0x23]));
        assert_ok!(r#""0""#, Bytes(vec![0x00]));

        // it is zero-padded hexadecimal
        assert_ok!(r#""0123""#, Bytes(vec![0x01, 0x23]));

        // Zero length
        assert_ok!(r#""""#, Bytes(vec![]));
        // Zero length with prefix
        assert_ok!(r#""0x""#, Bytes(vec![]));

        // contains whitespace
        assert_ok!(r#""12 34""#, Bytes(vec![0x12, 0x34]));

        // Not a hex
        assert_err!(r#""0xgg""#);
    }

    #[test]
    fn bytes_serialize() {
        let bytes = Bytes(vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
        let serialized = serde_json::to_string(&bytes).unwrap();
        assert_eq!(serialized, r#""0x0123456789abcdef""#);
    }

    #[test]
    fn bytes_serialize_without_prefix() {
        let bytes = Bytes(vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
        let serialized = serde_json::to_string(&bytes.without_prefix()).unwrap();
        assert_eq!(serialized, r#""0123456789abcdef""#);
    }

    #[test]
    fn bytes_into() {
        let bytes = Bytes(vec![0xff, 0x11]);
        let v: Vec<u8> = bytes.into();
        assert_eq!(vec![0xff, 0x11], v);
    }
}
