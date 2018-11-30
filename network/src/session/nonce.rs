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

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

use primitives::{H128, U128};
use rand::distributions::{Distribution, Standard};
use rand::Rng;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Clone, Copy, Debug, Eq)]
pub struct Nonce(H128);

impl Nonce {
    pub fn zero() -> Self {
        From::from(H128::zero())
    }
}

impl From<H128> for Nonce {
    fn from(nonce: H128) -> Self {
        Nonce(nonce)
    }
}

impl From<u64> for Nonce {
    fn from(nonce: u64) -> Self {
        Nonce(H128::from(nonce))
    }
}

impl From<Nonce> for H128 {
    fn from(nonce: Nonce) -> Self {
        nonce.0
    }
}

impl Deref for Nonce {
    type Target = H128;

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.0
    }
}

impl Decodable for Nonce {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(From::from(rlp.as_val::<H128>()?))
    }
}

impl Encodable for Nonce {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.0.rlp_append(s);
    }
}

impl Distribution<Nonce> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Nonce {
        let mut result = [0u8; 16];
        rng.fill_bytes(&mut result);
        Nonce(H128::from(U128::from(result)))
    }
}

impl Hash for Nonce {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl PartialEq for Nonce {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl PartialOrd for Nonce {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for Nonce {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_nonce() {
        rlp_encode_and_decode_test!(Nonce::from(H128::random()));
    }
}
