extern crate codechain_types as ctypes;
extern crate crypto as rcrypto;

pub mod aes;

use ctypes::hash::{H160, H256, H512};
use rcrypto::blake2b::Blake2b;
use rcrypto::sha1::Sha1;
use rcrypto::ripemd160::Ripemd160;

pub use rcrypto::digest::Digest;

/// Get the 256-bits BLAKE2b hash of the empty bytes string.
pub const BLAKE_EMPTY: H256 = H256([0x0e, 0x57, 0x51, 0xc0, 0x26, 0xe5, 0x43, 0xb2, 0xe8, 0xab, 0x2e, 0xb0, 0x60, 0x99, 0xda, 0xa1, 0xd1, 0xe5, 0xdf, 0x47, 0x77, 0x8f, 0x77, 0x87, 0xfa, 0xab, 0x45, 0xcd, 0xf1, 0x2f, 0xe3, 0xa8]);

/// Get the 256-bits BLAKE2b hash of the RLP encoding of empty data.
pub const BLAKE_NULL_RLP: H256 = H256([0x45, 0xb0, 0xcf, 0xc2, 0x20, 0xce, 0xec, 0x5b, 0x7c, 0x1c, 0x62, 0xc4, 0xd4, 0x19, 0x3d, 0x38, 0xe4, 0xeb, 0xa4, 0x8e, 0x88, 0x15, 0x72, 0x9c, 0xe7, 0x5f, 0x9c, 0x0a, 0xb0, 0xe4, 0xc1, 0xc0]);

/// Get the 256-bits BLAKE2b hash of the RLP encoding of empty list.
pub const BLAKE_EMPTY_LIST_RLP: H256 = H256([0xda, 0x22, 0x3b, 0x09, 0x96, 0x7c, 0x5b, 0xd2, 0x11, 0x07, 0x43, 0x30, 0x7e, 0x0a, 0xf6, 0xd3, 0x9f, 0x61, 0x72, 0x0a, 0xa7, 0x21, 0x8a, 0x64, 0x0a, 0x08, 0xee, 0xd1, 0x2d, 0xd5, 0x75, 0xc7]);

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

/// BLAKE256
pub fn blake256<T: AsRef<[u8]>>(s: T) -> H256 {
    let input = s.as_ref();
    let mut result = H256::default();
    let mut hasher = Blake2b::new(32);
    hasher.input(input);
    hasher.result(&mut *result);
    result
}

/// BLAKE512
pub fn blake512<T: AsRef<[u8]>>(s: T) -> H512 {
    let input = s.as_ref();
    let mut result = H512::default();
    let mut hasher = Blake2b::new(64);
    hasher.input(input);
    hasher.result(&mut *result);
    result
}

#[cfg(test)]
mod tests {
    use super::{BLAKE_EMPTY, BLAKE_NULL_RLP, BLAKE_EMPTY_LIST_RLP};
    use super::{blake256, blake512, ripemd160, sha1};

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
    fn test_blake256() {
        let expected = "324dcf027dd4a30a932c441f365a25e86b173defa4b8e58948253471b81b72cf".into();
        let result = blake256(b"hello");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_blake512() {
        let expected = "e4cfa39a3d37be31c59609e807970799caa68a19bfaa15135f165085e01d41a65ba1e1b146aeb6bd0092b49eac214c103ccfa3a365954bbbe52f74a2b3620c94".into();
        let result = blake512(b"hello");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_blake_empty() {
        let expected = BLAKE_EMPTY;
        let result = blake256([0u8; 0]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_blake_null_rlp() {
        let expected = BLAKE_NULL_RLP;
        let result = blake256([0x80]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_blake_empty_list_rlp() {
        let expected = BLAKE_EMPTY_LIST_RLP;
        let result = blake256([0xc0]);
        assert_eq!(result, expected);
    }
}
