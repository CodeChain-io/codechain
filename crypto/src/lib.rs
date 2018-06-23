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

extern crate codechain_types as ctypes;
extern crate crypto as rcrypto;
#[macro_use]
extern crate quick_error;
extern crate ring;

pub mod aes;
mod blake;
pub mod error;
mod hash;
pub mod pbkdf2;
pub mod scrypt;

pub use error::Error;

pub const KEY_LENGTH: usize = 32;
pub const KEY_ITERATIONS: usize = 10240;
pub const KEY_LENGTH_AES: usize = KEY_LENGTH / 2;

pub use self::blake::*;

pub use self::hash::{keccak256, ripemd160, sha1};

pub fn derive_key_iterations(password: &str, salt: &[u8; 32], c: u32) -> (Vec<u8>, Vec<u8>) {
    let mut derived_key = [0u8; KEY_LENGTH];
    pbkdf2::sha256(c, pbkdf2::Salt(salt), pbkdf2::Secret(password.as_bytes()), &mut derived_key);
    let derived_right_bits = &derived_key[0..KEY_LENGTH_AES];
    let derived_left_bits = &derived_key[KEY_LENGTH_AES..KEY_LENGTH];
    (derived_right_bits.to_vec(), derived_left_bits.to_vec())
}

pub fn derive_mac(derived_left_bits: &[u8], cipher_text: &[u8]) -> Vec<u8> {
    let mut mac = vec![0u8; KEY_LENGTH_AES + cipher_text.len()];
    mac[0..KEY_LENGTH_AES].copy_from_slice(derived_left_bits);
    mac[KEY_LENGTH_AES..cipher_text.len() + KEY_LENGTH_AES].copy_from_slice(cipher_text);
    mac
}

pub fn is_equal(a: &[u8], b: &[u8]) -> bool {
    ring::constant_time::verify_slices_are_equal(a, b).is_ok()
}
