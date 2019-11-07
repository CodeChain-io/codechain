// Copyright 2019. Kodebox, Inc.
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

use ckey::{
    sign, sign_schnorr, Error as KeyError, KeyPair, Message, Private, Public, SchnorrSignature, Secret, Signature,
};
use vrf::openssl::{Error as VRFError, ECVRF};
use vrf::VRF;

/// An opaque wrapper for secret.
pub struct DecryptedAccount {
    secret: Secret,
}

impl DecryptedAccount {
    pub fn new(secret: Secret) -> DecryptedAccount {
        DecryptedAccount {
            secret,
        }
    }

    /// Sign a message.
    pub fn sign(&self, message: &Message) -> Result<Signature, KeyError> {
        sign(&Private::from(self.secret), message)
    }

    /// Sign a message with Schnorr scheme.
    pub fn sign_schnorr(&self, message: &Message) -> Result<SchnorrSignature, KeyError> {
        sign_schnorr(&Private::from(self.secret), message)
    }

    ///  Generate VRF random hash output.
    pub fn vrf_hash(&self, message: &Message, vrf_inst: &mut ECVRF) -> Result<Vec<u8>, VRFError> {
        vrf_inst.prove(&Private::from(self.secret), message).and_then(|proof| vrf_inst.proof_to_hash(&proof))
    }

    /// Derive public key.
    pub fn public(&self) -> Result<Public, KeyError> {
        Ok(*KeyPair::from_private(Private::from(self.secret))?.public())
    }
}

impl Drop for DecryptedAccount {
    fn drop(&mut self) {
        self.secret = Secret::default();
    }
}
