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

extern crate codechain_crypto as crypto;
#[macro_use]
extern crate lazy_static;
extern crate bech32;
extern crate heapsize;
extern crate primitives;
extern crate rand;
extern crate rlp;
extern crate rustc_hex;
extern crate rustc_serialize;
extern crate secp256k1;
extern crate serde;
extern crate serde_json;

mod address;
#[cfg(feature = "ecdsa")]
mod ecdsa;
mod error;
mod exchange;
mod keypair;
mod network;
mod private;
mod random;
#[cfg(feature = "schnorr")]
mod schnorr;

pub use address::FullAddress;
#[cfg(feature = "ecdsa")]
pub use ecdsa::{
    recover_ecdsa as recover, sign_ecdsa as sign, verify_ecdsa as verify, verify_ecdsa_address as verify_address,
    ECDSASignature as Signature, ECDSASignatureData as SignatureData, ECDSA_SIGNATURE_LENGTH as SIGNATURE_LENGTH,
};
pub use error::Error;
pub use exchange::exchange;
pub use keypair::{public_to_address, KeyPair};
pub use network::Network;
use primitives::{H160, H256, H512};
pub use private::Private;
pub use random::Random;
pub use rustc_serialize::hex;
#[cfg(feature = "schnorr")]
pub use schnorr::{
    recover_schnorr as recover, sign_schnorr as sign, verify_schnorr as verify,
    verify_schnorr_address as verify_address, SchnorrSignature as Signature, SchnorrSignatureData as SignatureData,
    SCHNORR_SIGNATURE_LENGTH as SIGNATURE_LENGTH,
};

/// 32 bytes long signable message
pub type Message = H256;

pub type Address = H160;
pub type Secret = H256;
pub type Public = H512;

lazy_static! {
    pub static ref SECP256K1: secp256k1::Secp256k1 = secp256k1::Secp256k1::new();
}

/// Uninstantiatable error type for infallible generators.
#[derive(Debug)]
pub enum Void {}

/// Generates new keypair.
pub trait Generator {
    type Error;

    /// Should be called to generate new keypair.
    fn generate(&mut self) -> Result<KeyPair, Self::Error>;
}
