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

extern crate codechain_crypto as crypto;
#[macro_use]
extern crate lazy_static;
extern crate bech32;
extern crate never_type;
extern crate parking_lot;
extern crate primitives;
extern crate rand;
extern crate rand_xorshift;
extern crate rlp;
extern crate rustc_hex;
extern crate rustc_serialize;
extern crate secp256k1;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

mod address;
mod ecdsa;
mod error;
mod exchange;
mod keypair;
mod network;
mod password;
mod platform_address;
mod private;
mod random;
mod schnorr;

pub use crate::address::Address;
pub use crate::ecdsa::{
    recover_ecdsa as recover, sign_ecdsa as sign, verify_ecdsa as verify, verify_ecdsa_address as verify_address,
    ECDSASignature as Signature, ECDSA_SIGNATURE_LENGTH as SIGNATURE_LENGTH,
};
pub use crate::error::Error;
pub use crate::exchange::exchange;
pub use crate::keypair::{public_to_address, KeyPair};
pub use crate::network::NetworkId;
pub use crate::password::Password;
pub use crate::platform_address::PlatformAddress;
pub use crate::private::Private;
pub use crate::random::Random;
pub use crate::schnorr::{
    recover_schnorr, sign_schnorr, verify_schnorr, verify_schnorr_address, SchnorrSignature, SCHNORR_SIGNATURE_LENGTH,
};
use primitives::{H256, H512};
pub use rustc_serialize::hex;

/// 32 bytes long signable message
pub type Message = H256;

pub type Secret = H256;
pub type Public = H512;

const SECP256K1_TAG_PUBKEY_UNCOMPRESSED: u8 = 0x04;

lazy_static! {
    pub static ref SECP256K1: secp256k1::Secp256k1 = Default::default();
}

// CodeChain is using uncompressed form of public keys without prefix, so the prefix is required
// for public keys to be used in openssl.
pub fn standard_uncompressed_pubkey(pubkey: &Public) -> [u8; 65] {
    let mut result = [0; 65];
    result[0] = SECP256K1_TAG_PUBKEY_UNCOMPRESSED;
    result[1..].copy_from_slice(pubkey);

    result
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
