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
use ckey::{Address, PlatformAddress};
use cstate::World;
use rlp::{Encodable, RlpStream};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PodWorld {
    pub seq: u64,
    pub owners: Vec<Address>,
    pub users: Vec<Address>,
}

impl<'a> From<&'a PodWorld> for World {
    fn from(pod: &'a PodWorld) -> Self {
        World::new_with_seq(pod.owners.clone(), pod.users.clone(), pod.seq)
    }
}

impl Encodable for PodWorld {
    fn rlp_append(&self, s: &mut RlpStream) {
        let w: World = self.into();
        w.rlp_append(s);
    }
}

impl From<cjson::scheme::World> for PodWorld {
    fn from(s: cjson::scheme::World) -> Self {
        Self {
            seq: s.seq.map(Into::into).unwrap_or(0),
            owners: s
                .owners
                .map(|a| a.into_iter().map(PlatformAddress::into_address).collect())
                .unwrap_or_else(Vec::new),
            users: s
                .users
                .map(|users| users.into_iter().map(PlatformAddress::into_address).collect())
                .unwrap_or_else(Vec::new),
        }
    }
}

impl fmt::Display for PodWorld {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(#seq={}; owners={:#?}\n users ={:#?})", self.seq, self.owners, self.users)
    }
}
