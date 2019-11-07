// Copyright 2019 Kodebox, Inc.
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

use super::super::types;
use primitives::H256;
use rlp::{DecoderError, RlpStream, UntrustedRlp};

const MERKLE_KIND: &str = "merkle";

// FIXME: We can simplify it.
#[derive(RlpEncodable, RlpDecodable)]
pub struct Root {
    hash: H256,
}

impl Root {
    pub fn new(hash: H256) -> Self {
        Root {
            hash,
        }
    }
}

impl types::Root for Root {
    fn commitment_kind(&self) -> &'static str {
        MERKLE_KIND
    }

    fn encode(&self) -> Vec<u8> {
        rlp::encode(self).to_vec()
    }
}

pub struct Prefix {
    key_path: Vec<Vec<u8>>,
    key_prefix: Vec<u8>,
}

impl rlp::Encodable for Prefix {
    fn rlp_append(&self, s: &mut RlpStream) {
        unimplemented!()
    }
}

impl rlp::Decodable for Prefix {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        unimplemented!()
    }
}

impl Prefix {
    fn new(key_path: Vec<Vec<u8>>, key_prefix: Vec<u8>) -> Self {
        Prefix {
            key_path,
            key_prefix,
        }
    }

    fn key(&self, key: &[u8]) -> Vec<u8> {
        join(&self.key_prefix, key)
    }
}

impl types::Prefix for Prefix {
    fn commitment_kind(&self) -> &'static str {
        MERKLE_KIND
    }

    fn encode(&self) -> Vec<u8> {
        rlp::encode(self).to_vec()
    }
}

#[derive(Debug, PartialEq)]
pub struct Proof {
    key: Vec<u8>,
    value_hash: Vec<u8>,
    hashes: Vec<Vec<u8>>,
}

impl rlp::Encodable for Proof {
    fn rlp_append(&self, s: &mut RlpStream) {
        unimplemented!()
    }
}

impl rlp::Decodable for Proof {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        unimplemented!()
    }
}

impl types::Proof for Proof {
    fn commitment_kind(&self) -> &'static str {
        MERKLE_KIND
    }

    fn get_key(&self) -> &[u8] {
        &self.key
    }

    fn verify(_root: impl types::Root, _prefix: impl types::Prefix, _bytes: &[u8]) -> Result<(), String> {
        unimplemented!()
    }

    fn encode(&self) -> Vec<u8> {
        rlp::encode(self).to_vec()
    }
}

fn join(prefix: &[u8], key: &[u8]) -> Vec<u8> {
    [prefix, key].concat()
}
