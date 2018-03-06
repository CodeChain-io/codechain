extern crate codechain_types;
extern crate crypto as rcrypto;
extern crate siphasher;

pub use rcrypto::digest::Digest;
use std::hash::Hasher;
use rcrypto::blake2b::Blake2b;
use rcrypto::sha1::Sha1;
use rcrypto::ripemd160::Ripemd160;
use siphasher::sip::SipHasher24;
use codechain_types::hash::{H160, H256, H512};

#[cfg(test)]
extern crate codechain_bytes as bytes;

/// RIPEMD160
#[inline]
pub fn ripemd160(input: &[u8]) -> H160 {
	let mut result = H160::default();
	let mut hasher = Ripemd160::new();
	hasher.input(input);
	hasher.result(&mut *result);
	result
}

/// SHA-1
#[inline]
pub fn sha1(input: &[u8]) -> H160 {
	let mut result = H160::default();
	let mut hasher = Sha1::new();
	hasher.input(input);
	hasher.result(&mut *result);
	result
}

/// BLAKE256
pub fn blake256(input: &[u8]) -> H256 {
	let mut result = H256::default();
	let mut hasher = Blake2b::new(32);
	hasher.input(input);
	hasher.result(&mut *result);
	result
}

/// BLAKE512
pub fn blake512(input: &[u8]) -> H512 {
	let mut result = H512::default();
	let mut hasher = Blake2b::new(64);
	hasher.input(input);
	hasher.result(&mut *result);
	result
}

/// SipHash-2-4
#[inline]
pub fn siphash24(key0: u64, key1: u64, input: &[u8]) -> u64 {
	let mut hasher = SipHasher24::new_with_keys(key0, key1);
	hasher.write(input);
	hasher.finish()
}

#[cfg(test)]
mod tests {
	use super::{blake256, blake512, ripemd160, sha1, siphash24};

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
	fn test_siphash24() {
		let expected = 0x74f839c593dc67fd_u64;
		let result = siphash24(0x0706050403020100_u64, 0x0F0E0D0C0B0A0908_u64, &[0; 1]);
		assert_eq!(result, expected);
	}
}
