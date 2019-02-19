// Copyright 2015-2017 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::error::Error as StdError;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
/// Error concerning the RLP decoder.
pub enum DecoderError {
    /// Data has additional bytes at the end of the valid RLP fragment.
    RlpIsTooBig {
        expected: usize,
        got: usize,
    },
    /// Data has too few bytes for valid RLP.
    RlpIsTooShort {
        expected: usize,
        got: usize,
    },
    /// Expect an encoded list, RLP was something else.
    RlpExpectedToBeList,
    /// Expect encoded data, RLP was something else.
    RlpExpectedToBeData,
    /// Expected a different size list.
    RlpIncorrectListLen {
        expected: usize,
        got: usize,
    },
    /// Data length number has a prefixed zero byte, invalid for numbers.
    RlpDataLenWithZeroPrefix,
    /// List length number has a prefixed zero byte, invalid for numbers.
    RlpListLenWithZeroPrefix,
    /// Non-canonical (longer than necessary) representation used for data or list.
    RlpInvalidIndirection,
    /// Declared length is inconsistent with data specified after.
    RlpInconsistentLengthAndData {
        max: usize,
        index: usize,
    },
    /// Declared length is invalid and results in overflow
    RlpInvalidLength {
        expected: usize,
        got: usize,
    },
    /// A string MUST NOT be null terminated.
    RlpNullTerminatedString,
    /// Custom rlp decoding error.
    Custom(&'static str),
}

impl StdError for DecoderError {
    fn description(&self) -> &str {
        "builder error"
    }
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self, f)
    }
}
