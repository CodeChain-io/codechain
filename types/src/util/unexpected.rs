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

//! Error utils

use std::fmt;

use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
/// Error indicating an expected value was not found.
pub struct Mismatch<T> {
    /// Value expected.
    pub expected: T,
    /// Value found.
    pub found: T,
}

impl<T: fmt::Display> fmt::Display for Mismatch<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("Expected {}, found {}", self.expected, self.found))
    }
}

impl<T> Encodable for Mismatch<T>
where
    T: Encodable,
{
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2).append(&self.expected).append(&self.found);
    }
}

impl<T> Decodable for Mismatch<T>
where
    T: Decodable,
{
    fn decode(rlp: &UntrustedRlp) -> Result<Mismatch<T>, DecoderError> {
        Ok(Mismatch {
            expected: rlp.val_at(0)?,
            found: rlp.val_at(1)?,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Error indicating value found is outside of a valid range.
pub struct OutOfBounds<T> {
    /// Minimum allowed value.
    pub min: Option<T>,
    /// Maximum allowed value.
    pub max: Option<T>,
    /// Value found.
    pub found: T,
}

impl<T: fmt::Display> fmt::Display for OutOfBounds<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match (self.min.as_ref(), self.max.as_ref()) {
            (Some(min), Some(max)) => format!("Min={}, Max={}", min, max),
            (Some(min), _) => format!("Min={}", min),
            (_, Some(max)) => format!("Max={}", max),
            (None, None) => "".into(),
        };

        f.write_fmt(format_args!("Value {} out of bounds. {}", self.found, msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rlp_encode_and_decode_mismatch() {
        rlp_encode_and_decode_test!(Mismatch::<u8> {
            expected: 0,
            found: 1,
        });
    }
}
