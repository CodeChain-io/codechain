//! Bitcoin keys.

extern crate base58;
extern crate codechain_bytes;
extern crate codechain_crypto as crypto;
extern crate codechain_types;
#[macro_use]
extern crate lazy_static;
extern crate rand;
extern crate rustc_serialize;
extern crate secp256k1;

pub mod generator;
mod address;
mod display;
mod keypair;
mod error;
mod network;
mod private;
mod public;
mod signature;

pub use address::Address;
pub use codechain_types::hash;
pub use display::DisplayLayout;
pub use error::Error;
use hash::{H160, H256};
pub use keypair::KeyPair;
pub use network::Network;
pub use private::Private;
pub use public::Public;
pub use rustc_serialize::hex;
pub use signature::{CompactSignature, Signature};

/// 20 bytes long hash derived from public `ripemd160(blake256(public))`
pub type AccountId = H160;
/// 32 bytes long secret key
pub type Secret = H256;
/// 32 bytes long signable message
pub type Message = H256;

lazy_static! {
	pub static ref SECP256K1: secp256k1::Secp256k1 = secp256k1::Secp256k1::new();
}
