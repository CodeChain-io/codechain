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

use std::fmt;

use cjson;
use cstate::ShardMetadata;
use rlp::{Encodable, RlpStream};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PodShardMetadata {
    pub number_of_worlds: u32,
    pub nonce: u64,
}

impl<'a> Into<ShardMetadata> for &'a PodShardMetadata {
    fn into(self) -> ShardMetadata {
        ShardMetadata::new_with_nonce(self.number_of_worlds, self.nonce)
    }
}

impl Encodable for PodShardMetadata {
    fn rlp_append(&self, s: &mut RlpStream) {
        let m: ShardMetadata = self.into();
        m.rlp_append(s);
    }
}

impl From<cjson::spec::Shard> for PodShardMetadata {
    fn from(s: cjson::spec::Shard) -> Self {
        Self {
            number_of_worlds: 0,
            nonce: s.nonce.unwrap_or(0),
        }
    }
}

impl fmt::Display for PodShardMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(#wordls={}; nonce={})", self.number_of_worlds, self.nonce)
    }
}
