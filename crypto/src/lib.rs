extern crate codechain_types;
extern crate crypto as rcrypto;
extern crate siphasher;

pub use rcrypto::digest::Digest;
use std::hash::Hasher;
use rcrypto::sha1::Sha1;
use rcrypto::ripemd160::Ripemd160;
use siphasher::sip::SipHasher24;
use codechain_types::hash::{H160};

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

/// SipHash-2-4
#[inline]
pub fn siphash24(key0: u64, key1: u64, input: &[u8]) -> u64 {
	let mut hasher = SipHasher24::new_with_keys(key0, key1);
	hasher.write(input);
	hasher.finish()
}

#[cfg(test)]
mod tests {
	use super::{ripemd160, sha1, siphash24};

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
	fn test_siphash24() {
		let expected = 0x74f839c593dc67fd_u64;
		let result = siphash24(0x0706050403020100_u64, 0x0F0E0D0C0B0A0908_u64, &[0; 1]);
		assert_eq!(result, expected);
	}
}
