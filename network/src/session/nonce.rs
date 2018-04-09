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

use std::hash::{Hash, Hasher};

use ctypes::hash::H128;
use rand::{Rand, Rng};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Nonce(u32);

impl Nonce {
    pub fn new(n: u32) -> Self {
        Nonce(n)
    }

    pub fn zero() -> Self {
        Nonce::new(0)
    }
}

impl From<u32> for Nonce {
    fn from(nonce: u32) -> Self {
        Nonce(nonce)
    }
}

impl Into<u32> for Nonce {
    fn into(self) -> u32 {
        self.0
    }
}

impl Into<H128> for Nonce {
    fn into(self) -> H128 {
        // FIXME: This implementation is so naive.
        let mut hash: H128 = H128::zero();
        hash[3] = (self.0 & 0xFF) as u8;
        hash[5] = ((self.0 >> 8) & 0xFF) as u8;
        hash[7] = ((self.0 >> 16) & 0xFF) as u8;
        hash[11] = ((self.0 >> 24) & 0xFF) as u8;
        hash
    }
}

impl Decodable for Nonce {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Ok(From::from(rlp.as_val::<u32>()?))
    }
}

impl Encodable for Nonce {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.0.rlp_append(s);
    }
}

impl Rand for Nonce {
    fn rand<R: Rng>(rng: &mut R) -> Self {
        Nonce(Rand::rand(rng))
    }
}

impl Hash for Nonce {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}
