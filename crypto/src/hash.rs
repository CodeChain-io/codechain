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

use ctypes::{H160, H256};
use rcrypto::digest::Digest;
use rcrypto::ripemd160::Ripemd160;
use rcrypto::sha1::Sha1;
use rcrypto::sha3::Sha3;

/// RIPEMD160
#[inline]
pub fn ripemd160<T: AsRef<[u8]>>(s: T) -> H160 {
    let input = s.as_ref();
    let mut result = H160::default();
    let mut hasher = Ripemd160::new();
    hasher.input(input);
    hasher.result(&mut *result);
    result
}

/// SHA-1
#[inline]
pub fn sha1<T: AsRef<[u8]>>(s: T) -> H160 {
    let input = s.as_ref();
    let mut result = H160::default();
    let mut hasher = Sha1::new();
    hasher.input(input);
    hasher.result(&mut *result);
    result
}

/// KECCAK256
#[inline]
pub fn keccak256<T: AsRef<[u8]>>(s: T) -> H256 {
    let input = s.as_ref();
    let mut result = H256::default();
    let mut hasher = Sha3::keccak256();
    hasher.input(input);
    hasher.result(&mut result);
    result
}

#[cfg(test)]
mod tests {
    use super::{keccak256, ripemd160, sha1};

    #[test]
    fn test_ripemd160() {
        let expected = "108f07b8382412612c048d07d13f814118445acd".into();
        let result = ripemd160(b"hello");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_sha1() {
        let expected = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".into();
        let result = sha1(b"hello");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_keccak256() {
        let expected = "1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8".into();
        let result = keccak256(b"hello");
        assert_eq!(result, expected);
    }
}
