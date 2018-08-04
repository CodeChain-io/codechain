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
use ckey::Address;
use cstate::ShardMetadata;
use rlp::{Encodable, RlpStream};

use super::pod_world::PodWorld;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PodShardMetadata {
    pub owner: Address,
    pub nonce: u64,
    pub worlds: Vec<PodWorld>,
}

impl<'a> Into<ShardMetadata> for &'a PodShardMetadata {
    fn into(self) -> ShardMetadata {
        assert!(self.worlds.len() <= ::std::u16::MAX as usize);
        ShardMetadata::new_with_nonce(self.worlds.len() as u16, self.nonce)
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
            nonce: s.nonce.map(Into::into).unwrap_or(0),
            owner: s.owner,
            worlds: s.worlds.unwrap_or_else(Vec::new).into_iter().map(Into::into).collect(),
        }
    }
}

impl fmt::Display for PodShardMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(#nonce={}; owner={}; worlds={:#?})", self.nonce, self.owner, self.worlds)
    }
}
