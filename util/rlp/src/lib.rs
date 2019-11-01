// Copyright 2015-2017 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Recursive Length Prefix serialization crate.
//!
//! Allows encoding, decoding, and view onto rlp-slice
//!
//!# What should you use when?
//!
//!### Use `encode` function when:
//! * You want to encode something inline.
//! * You do not work on big set of data.
//! * You want to encode whole data structure at once.
//!
//!### Use `decode` function when:
//! * You want to decode something inline.
//! * You do not work on big set of data.
//! * You want to decode whole rlp at once.
//!
//!### Use `RlpStream` when:
//! * You want to encode something in portions.
//! * You encode a big set of data.
//!
//!### Use `Rlp` when:
//! * You need to handle data corruption errors.
//! * You are working on input data.
//! * You want to get view onto rlp-slice.
//! * You don't want to decode whole rlp at once.

extern crate elastic_array;
extern crate primitives;
extern crate rustc_hex;

mod error;
mod impls;
mod rlpin;
mod stream;
mod traits;

use elastic_array::ElasticArray1024;
use std::borrow::Borrow;

pub use error::DecoderError;
pub use rlpin::{PayloadInfo, Prototype, Rlp, RlpIterator};
pub use stream::RlpStream;
pub use traits::{Compressible, Decodable, Encodable};

/// The RLP encoded empty data (used to mean "null value").
pub const NULL_RLP: [u8; 1] = [0x80; 1];
/// The RLP encoded empty list.
pub const EMPTY_LIST_RLP: [u8; 1] = [0xC0; 1];

/// Shortcut function to decode trusted rlp
///
/// ```rust
/// extern crate rlp;
///
/// fn main () {
/// 	let data = vec![0x83, b'c', b'a', b't'];
/// 	let animal: String = rlp::decode(&data).unwrap();
/// 	assert_eq!(animal, "cat".to_string());
/// }
/// ```
pub fn decode<T>(bytes: &[u8]) -> Result<T, DecoderError>
where
    T: Decodable, {
    let rlp = Rlp::new(bytes);
    rlp.as_val()
}

pub fn decode_list<T>(bytes: &[u8]) -> Vec<T>
where
    T: Decodable, {
    let rlp = Rlp::new(bytes);
    rlp.as_list().expect("trusted rlp should be valid")
}

/// Shortcut function to encode structure into rlp.
///
/// ```rust
/// extern crate rlp;
///
/// fn main () {
/// 	let animal = "cat";
/// 	let out = rlp::encode(&animal).into_vec();
/// 	assert_eq!(out, vec![0x83, b'c', b'a', b't']);
/// }
/// ```
pub fn encode<E>(object: &E) -> ElasticArray1024<u8>
where
    E: Encodable, {
    let mut stream = RlpStream::new();
    stream.append_single_value(object);
    stream.drain()
}

pub fn encode_list<E, K>(object: &[K]) -> ElasticArray1024<u8>
where
    E: Encodable,
    K: Borrow<E>, {
    let mut stream = RlpStream::new();
    stream.append_list(object);
    stream.drain()
}
