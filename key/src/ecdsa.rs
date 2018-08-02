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

use primitives::{H256, H520};
use rustc_hex::{FromHex, ToHex};
use secp256k1::{key, Error as SecpError, Message as SecpMessage, RecoverableSignature, RecoveryId};

use super::{public_to_address, Address, Error, Message, Private, Public, SECP256K1};

pub const ECDSA_SIGNATURE_LENGTH: usize = 65;

pub type ECDSASignatureData = H520;

/// Signature encoded as RSV components
#[repr(C)]
pub struct ECDSASignature([u8; 65]);

impl ECDSASignature {
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
        ECDSASignature(sig)
    }

    /// Check if this is a "low" signature.
    pub fn is_low_s(&self) -> bool {
        H256::from_slice(self.s()) <= "7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0".into()
    }

    /// Check if each component of the signature is in range.
    pub fn is_valid(&self) -> bool {
        self.v() <= 1
            && H256::from_slice(self.r()) < "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141".into()
            && H256::from_slice(self.r()) >= 1.into()
            && H256::from_slice(self.s()) < "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141".into()
            && H256::from_slice(self.s()) >= 1.into()
    }
}

// manual implementation large arrays don't have trait impls by default.
// remove when integer generics exist
impl PartialEq for ECDSASignature {
    fn eq(&self, other: &Self) -> bool {
        &self.0[..] == &other.0[..]
    }
}

// manual implementation required in Rust 1.13+, see `std::cmp::AssertParamIsEq`.
impl Eq for ECDSASignature {}

// also manual for the same reason, but the pretty printing might be useful.
impl fmt::Debug for ECDSASignature {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("Signature")
            .field("r", &self.0[0..32].to_hex())
            .field("s", &self.0[32..64].to_hex())
            .field("v", &self.0[64..65].to_hex())
            .finish()
    }
}

impl fmt::Display for ECDSASignature {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.to_hex())
    }
}

impl FromStr for ECDSASignature {
    type Err = SecpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.from_hex() {
            Ok(ref hex) if hex.len() == 65 => {
                let mut data = [0; 65];
                data.copy_from_slice(&hex[0..65]);
                Ok(ECDSASignature(data))
            }
            _ => Err(SecpError::InvalidSignature),
        }
    }
}

impl Default for ECDSASignature {
    fn default() -> Self {
        ECDSASignature([0; 65])
    }
}

impl Hash for ECDSASignature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        H520::from(self.0).hash(state);
    }
}

impl Clone for ECDSASignature {
    fn clone(&self) -> Self {
        ECDSASignature(self.0)
    }
}

impl From<[u8; 65]> for ECDSASignature {
    fn from(s: [u8; 65]) -> Self {
        ECDSASignature(s)
    }
}

impl Into<[u8; 65]> for ECDSASignature {
    fn into(self) -> [u8; 65] {
        self.0
    }
}

impl From<ECDSASignature> for H520 {
    fn from(s: ECDSASignature) -> Self {
        H520::from(s.0)
    }
}

impl From<H520> for ECDSASignature {
    fn from(bytes: H520) -> Self {
        ECDSASignature(bytes.into())
    }
}

impl Deref for ECDSASignature {
    type Target = [u8; 65];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ECDSASignature {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}


pub fn sign_ecdsa(private: &Private, message: &Message) -> Result<ECDSASignature, Error> {
    let context = &SECP256K1;
    let sec = key::SecretKey::from_slice(context, &private)?;
    let s = context.sign_recoverable(&SecpMessage::from_slice(&message[..])?, &sec)?;
    let (rec_id, data) = s.serialize_compact(context);
    let mut data_arr = [0; 65];

    // no need to check if s is low, it always is
    data_arr[0..64].copy_from_slice(&data[0..64]);
    data_arr[64] = rec_id.to_i32() as u8;
    Ok(ECDSASignature(data_arr))
}

pub fn verify_ecdsa(public: &Public, signature: &ECDSASignature, message: &Message) -> Result<bool, Error> {
    let context = &SECP256K1;
    let rsig =
        RecoverableSignature::from_compact(context, &signature[0..64], RecoveryId::from_i32(signature[64] as i32)?)?;
    let sig = rsig.to_standard(context);

    let pdata: [u8; 65] = {
        let mut temp = [4u8; 65];
        temp[1..65].copy_from_slice(&**public);
        temp
    };

    let publ = key::PublicKey::from_slice(context, &pdata)?;
    match context.verify(&SecpMessage::from_slice(&message[..])?, &sig, &publ) {
        Ok(_) => Ok(true),
        Err(SecpError::IncorrectSignature) => Ok(false),
        Err(x) => Err(Error::from(x)),
    }
}

pub fn verify_ecdsa_address(address: &Address, signature: &ECDSASignature, message: &Message) -> Result<bool, Error> {
    let public = recover_ecdsa(signature, message)?;
    let recovered_address = public_to_address(&public);
    Ok(address == &recovered_address)
}

pub fn recover_ecdsa(signature: &ECDSASignature, message: &Message) -> Result<Public, Error> {
    let context = &SECP256K1;
    let rsig =
        RecoverableSignature::from_compact(context, &signature[0..64], RecoveryId::from_i32(signature[64] as i32)?)?;
    let pubkey = context.recover(&SecpMessage::from_slice(&message[..])?, &rsig)?;
    let serialized = pubkey.serialize_vec(context, false);

    let mut public = Public::default();
    public.copy_from_slice(&serialized[1..65]);
    Ok(public)
}


#[cfg(test)]
mod tests {
    use super::super::{Generator, Message, Random};
    use super::{recover_ecdsa, sign_ecdsa, verify_ecdsa, verify_ecdsa_address, ECDSASignature};
    use std::str::FromStr;

    #[test]
    fn signature_to_and_from_str() {
        let keypair = Random.generate().unwrap();
        let message = Message::default();
        let signature = sign_ecdsa(keypair.private(), &message).unwrap();
        let string = format!("{}", signature);
        let deserialized = ECDSASignature::from_str(&string).unwrap();
        assert_eq!(signature, deserialized);
    }

    #[test]
    fn sign_and_recover_public() {
        let keypair = Random.generate().unwrap();
        let message = Message::default();
        let signature = sign_ecdsa(keypair.private(), &message).unwrap();
        assert_eq!(keypair.public(), &recover_ecdsa(&signature, &message).unwrap());
    }

    #[test]
    fn sign_and_verify_public() {
        let keypair = Random.generate().unwrap();
        let message = Message::default();
        let signature = sign_ecdsa(keypair.private(), &message).unwrap();
        assert!(verify_ecdsa(keypair.public(), &signature, &message).unwrap());
    }

    #[test]
    fn sign_and_verify_address() {
        let keypair = Random.generate().unwrap();
        let message = Message::default();
        let signature = sign_ecdsa(keypair.private(), &message).unwrap();
        assert!(verify_ecdsa_address(&keypair.address(), &signature, &message).unwrap());
    }
}
