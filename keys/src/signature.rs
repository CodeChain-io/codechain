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

use std::ops::{Deref, DerefMut};
use std::cmp::PartialEq;
use std::fmt;
use std::str::FromStr;
use std::hash::{Hash, Hasher};
use secp256k1::{Message as SecpMessage, RecoverableSignature, RecoveryId, Error as SecpError};
use rustc_hex::{ToHex, FromHex};
use codechain_types::{H520, H256};
use {Error, Message, Public, SECP256K1};

/// Signature encoded as RSV components
#[repr(C)]
pub struct Signature(pub [u8; 65]);

impl Signature {
    /// Get a slice into the 'r' portion of the data.
    pub fn r(&self) -> &[u8] {
        &self.0[0..32]
    }

    /// Get a slice into the 's' portion of the data.
    pub fn s(&self) -> &[u8] {
        &self.0[32..64]
    }

    /// Get the recovery byte.
    pub fn v(&self) -> u8 {
        self.0[64]
    }

    /// Create a signature object from the sig.
    pub fn from_rsv(r: &H256, s: &H256, v: u8) -> Self {
        let mut sig = [0u8; 65];
        sig[0..32].copy_from_slice(&r);
        sig[32..64].copy_from_slice(&s);
        sig[64] = v;
        Signature(sig)
    }

    /// Check if this is a "low" signature.
    pub fn is_low_s(&self) -> bool {
        H256::from_slice(self.s()) <= "7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0".into()
    }

    /// Check if each component of the signature is in range.
    pub fn is_valid(&self) -> bool {
        self.v() <= 1 &&
            H256::from_slice(self.r()) < "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141".into() &&
            H256::from_slice(self.r()) >= 1.into() &&
            H256::from_slice(self.s()) < "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141".into() &&
            H256::from_slice(self.s()) >= 1.into()
    }

    pub fn recover(&self, message: &Message) -> Result<Public, Error> {
        let context = &SECP256K1;
        let rsig = RecoverableSignature::from_compact(context, &self[0..64], RecoveryId::from_i32(self[64] as i32)?)?;
        let pubkey = context.recover(&SecpMessage::from_slice(&message[..])?, &rsig)?;
        let serialized = pubkey.serialize_vec(context, false);

        let mut public = H520::default();
        public.copy_from_slice(&serialized[0..65]);
        Ok(Public::Normal(public))
    }
}

// manual implementation large arrays don't have trait impls by default.
// remove when integer generics exist
impl PartialEq for Signature {
    fn eq(&self, other: &Self) -> bool {
        &self.0[..] == &other.0[..]
    }
}

// manual implementation required in Rust 1.13+, see `std::cmp::AssertParamIsEq`.
impl Eq for Signature { }

// also manual for the same reason, but the pretty printing might be useful.
impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("Signature")
            .field("r", &self.0[0..32].to_hex())
            .field("s", &self.0[32..64].to_hex())
            .field("v", &self.0[64..65].to_hex())
            .finish()
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.to_hex())
    }
}

impl FromStr for Signature {
    type Err = SecpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.from_hex() {
            Ok(ref hex) if hex.len() == 65 => {
                let mut data = [0; 65];
                data.copy_from_slice(&hex[0..65]);
                Ok(Signature(data))
            },
            _ => Err(SecpError::InvalidSignature)
        }
    }
}

impl Default for Signature {
    fn default() -> Self {
        Signature([0; 65])
    }
}

impl Hash for Signature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        H520::from(self.0).hash(state);
    }
}

impl Clone for Signature {
    fn clone(&self) -> Self {
        Signature(self.0)
    }
}

impl From<[u8; 65]> for Signature {
    fn from(s: [u8; 65]) -> Self {
        Signature(s)
    }
}

impl Into<[u8; 65]> for Signature {
    fn into(self) -> [u8; 65] {
        self.0
    }
}

impl From<Signature> for H520 {
    fn from(s: Signature) -> Self {
        H520::from(s.0)
    }
}

impl From<H520> for Signature {
    fn from(bytes: H520) -> Self {
        Signature(bytes.into())
    }
}

impl Deref for Signature {
    type Target = [u8; 65];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Signature {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

