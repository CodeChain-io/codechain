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

use std::cmp::PartialEq;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use primitives::H512;
use rustc_hex::{FromHex, ToHex};
use secp256k1::{key, schnorr, Error as SecpError, Message as SecpMessage};

use super::{public_to_address, Address, Error, Message, Private, Public, SECP256K1};

pub const SCHNORR_SIGNATURE_LENGTH: usize = 64;

pub type SchnorrSignatureData = H512;

pub struct SchnorrSignature([u8; 64]);

impl SchnorrSignature {
    /// Check if this is a "low" signature.
    pub fn is_low_s(&self) -> bool {
        true
    }

    pub fn is_unsigned(&self) -> bool {
        let signature_data: H512 = self.0.into();
        signature_data.is_zero()
    }
}

// manual implementation large arrays don't have trait impls by default.
// remove when integer generics exist
impl PartialEq for SchnorrSignature {
    fn eq(&self, other: &Self) -> bool {
        &self.0[..] == &other.0[..]
    }
}

// manual implementation required in Rust 1.13+, see `std::cmp::AssertParamIsEq`.
impl Eq for SchnorrSignature {}

// also manual for the same reason, but the pretty printing might be useful.
impl fmt::Debug for SchnorrSignature {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("Signature").field("r", &self.0[0..32].to_hex()).field("s", &self.0[32..64].to_hex()).finish()
    }
}

impl fmt::Display for SchnorrSignature {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.to_hex())
    }
}

impl FromStr for SchnorrSignature {
    type Err = SecpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.from_hex() {
            Ok(ref hex) if hex.len() == 64 => {
                let mut data = [0; 64];
                data.copy_from_slice(&hex[0..64]);
                Ok(SchnorrSignature(data))
            }
            _ => Err(SecpError::InvalidSignature),
        }
    }
}

impl Default for SchnorrSignature {
    fn default() -> Self {
        SchnorrSignature([0; 64])
    }
}

impl Hash for SchnorrSignature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        H512::from(self.0).hash(state);
    }
}

impl Clone for SchnorrSignature {
    fn clone(&self) -> Self {
        SchnorrSignature(self.0)
    }
}

impl From<[u8; 64]> for SchnorrSignature {
    fn from(s: [u8; 64]) -> Self {
        SchnorrSignature(s)
    }
}

impl Into<[u8; 64]> for SchnorrSignature {
    fn into(self) -> [u8; 64] {
        self.0
    }
}

impl From<SchnorrSignature> for H512 {
    fn from(s: SchnorrSignature) -> Self {
        H512::from(s.0)
    }
}

impl From<H512> for SchnorrSignature {
    fn from(bytes: H512) -> Self {
        SchnorrSignature(bytes.into())
    }
}

impl Deref for SchnorrSignature {
    type Target = [u8; 64];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SchnorrSignature {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub fn sign_schnorr(private: &Private, message: &Message) -> Result<SchnorrSignature, Error> {
    let context = &SECP256K1;
    let sec = key::SecretKey::from_slice(context, &private)?;
    let s = context.sign_schnorr(&SecpMessage::from_slice(&message[..])?, &sec)?;

    let mut data = [0; 64];
    data.copy_from_slice(&s.serialize()[0..64]);
    Ok(SchnorrSignature(data))
}

pub fn verify_schnorr(public: &Public, signature: &SchnorrSignature, message: &Message) -> Result<bool, Error> {
    let context = &SECP256K1;
    let pdata: [u8; 65] = {
        let mut temp = [4u8; 65];
        temp[1..65].copy_from_slice(&**public);
        temp
    };

    let publ = key::PublicKey::from_slice(context, &pdata)?;
    let sig = schnorr::Signature::deserialize(&signature.0);
    match context.verify_schnorr(&SecpMessage::from_slice(&message[..])?, &sig, &publ) {
        Ok(_) => Ok(true),
        Err(SecpError::IncorrectSignature) => Ok(false),
        Err(x) => Err(Error::from(x)),
    }
}

pub fn verify_schnorr_address(
    address: &Address,
    signature: &SchnorrSignature,
    message: &Message,
) -> Result<bool, Error> {
    let public = recover_schnorr(signature, message)?;
    let recovered_address = public_to_address(&public);
    Ok(address == &recovered_address)
}

pub fn recover_schnorr(signature: &SchnorrSignature, message: &Message) -> Result<Public, Error> {
    let context = &SECP256K1;

    let sig = schnorr::Signature::deserialize(&signature.0);
    let pubkey = context.recover_schnorr(&SecpMessage::from_slice(&message[..])?, &sig)?;
    let serialized = pubkey.serialize_vec(context, false);

    let mut public = Public::default();
    public.copy_from_slice(&serialized[1..65]);
    Ok(public)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::super::{Generator, Message, Random};
    use super::{recover_schnorr, sign_schnorr, verify_schnorr, verify_schnorr_address, SchnorrSignature};

    #[test]
    fn signature_to_and_from_str() {
        let keypair = Random.generate().unwrap();
        let message = Message::default();
        let signature = sign_schnorr(keypair.private(), &message).unwrap();
        let string = format!("{}", signature);
        let deserialized = SchnorrSignature::from_str(&string).unwrap();
        assert_eq!(signature, deserialized);
    }

    #[test]
    fn sign_and_recover_public() {
        let keypair = Random.generate().unwrap();
        let message = Message::default();
        let signature = sign_schnorr(keypair.private(), &message).unwrap();
        assert_eq!(keypair.public(), &recover_schnorr(&signature, &message).unwrap());
    }

    #[test]
    fn sign_and_verify_public() {
        let keypair = Random.generate().unwrap();
        let message = Message::default();
        let signature = sign_schnorr(keypair.private(), &message).unwrap();
        assert!(verify_schnorr(keypair.public(), &signature, &message).unwrap());
    }

    #[test]
    fn sign_and_verify_address() {
        let keypair = Random.generate().unwrap();
        let message = Message::default();
        let signature = sign_schnorr(keypair.private(), &message).unwrap();
        assert!(verify_schnorr_address(&keypair.address(), &signature, &message).unwrap());
    }
}
