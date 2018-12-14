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

use std::ops::Deref;

use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp, NULL_RLP};

use crate::CacheableItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionData(Bytes);

impl Default for ActionData {
    fn default() -> Self {
        ActionData(NULL_RLP.to_vec())
    }
}

impl Deref for ActionData {
    type Target = Bytes;

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.0
    }
}

impl CacheableItem for ActionData {
    type Address = H256;
    fn is_null(&self) -> bool {
        self.is_empty()
    }
}

impl Encodable for ActionData {
    fn rlp_append(&self, s: &mut RlpStream) {
        self.0.rlp_append(s);
    }
}

impl Decodable for ActionData {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        Bytes::decode(rlp).map(ActionData)
    }
}

impl From<Bytes> for ActionData {
    fn from(f: Vec<u8>) -> Self {
        ActionData(f)
    }
}

impl From<ActionData> for Bytes {
    fn from(f: ActionData) -> Self {
        f.0
    }
}
