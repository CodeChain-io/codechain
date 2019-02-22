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

use rcrypto::scrypt::{scrypt, ScryptParams};

use crate::error::ScryptError;
use crate::{Password, KEY_LENGTH, KEY_LENGTH_AES};

// Do not move Password. It will make debugger print the password.
pub fn derive_key(pass: &Password, salt: &[u8; 32], n: u32, p: u32, r: u32) -> Result<(Vec<u8>, Vec<u8>), ScryptError> {
    // sanity checks
    let log_n = (32 - n.leading_zeros() - 1) as u8;
    if u32::from(log_n) >= r * 16 {
        return Err(ScryptError::InvalidN)
    }

    if u64::from(p) > ((u64::from(u32::max_value()) - 1) * 32) / (128 * u64::from(r)) {
        return Err(ScryptError::InvalidP)
    }

    let mut derived_key = vec![0u8; KEY_LENGTH];
    let scrypt_params = ScryptParams::new(log_n, r, p);
    scrypt(pass.as_bytes(), salt, &scrypt_params, &mut derived_key);
    let derived_right_bits = &derived_key[0..KEY_LENGTH_AES];
    let derived_left_bits = &derived_key[KEY_LENGTH_AES..KEY_LENGTH];
    Ok((derived_right_bits.to_vec(), derived_left_bits.to_vec()))
}
