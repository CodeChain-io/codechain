// Copyright 2018-2019 Kodebox, Inc.
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
use ckey::{Address, PlatformAddress};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PodShardMetadata {
    pub owners: Vec<Address>,
    pub users: Vec<Address>,
    pub seq: u64,
}

impl From<cjson::scheme::Shard> for PodShardMetadata {
    fn from(s: cjson::scheme::Shard) -> Self {
        Self {
            seq: s.seq.map(Into::into).unwrap_or(0),
            owners: s.owners.into_iter().map(PlatformAddress::into_address).collect(),
            users: s.users.unwrap_or_else(Vec::new).into_iter().map(PlatformAddress::into_address).collect(),
        }
    }
}

impl fmt::Display for PodShardMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(#seq={}; owners={:#?}; users={:#?})", self.seq, self.owners, self.users)
    }
}
