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

extern crate codechain_bytes;
extern crate codechain_crypto as crypto;
extern crate codechain_types;
#[macro_use]
extern crate lazy_static;
extern crate bech32;
extern crate heapsize;
extern crate rand;
extern crate rlp;
extern crate rustc_hex;
extern crate rustc_serialize;
extern crate secp256k1;

mod address;
mod error;
mod exchange;
mod keypair;
mod network;
mod private;
mod random;
mod signature;

pub use error::Error;
pub use exchange::exchange;
pub use keypair::{public_to_address, KeyPair};
pub use network::Network;
pub use private::Private;
pub use random::Random;
pub use rustc_serialize::hex;
pub use signature::{recover_ecdsa, sign_ecdsa, ECDSASignature};

use codechain_types::H256;

/// 32 bytes long signable message
pub type Message = H256;

pub use codechain_types::{Address, Public, Secret};

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
