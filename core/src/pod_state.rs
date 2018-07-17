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

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;

use cjson;
use ckey::Address;

use super::pod_account::PodAccount;
use super::pod_shard_metadata::PodShardMetadata;

/// State of all accounts in the system expressed in Plain Old Data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PodAccounts(BTreeMap<Address, PodAccount>);

impl Deref for PodAccounts {
    type Target = BTreeMap<Address, PodAccount>;

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.0
    }
}

impl From<cjson::spec::Accounts> for PodAccounts {
    fn from(s: cjson::spec::Accounts) -> PodAccounts {
        let accounts =
            s.into_iter().filter(|(_, acc)| !acc.is_empty()).map(|(addr, acc)| (addr.into(), acc.into())).collect();
        PodAccounts(accounts)
    }
}

impl fmt::Display for PodAccounts {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (add, acc) in &self.0 {
            writeln!(f, "{} => {}", add, acc)?;
        }
        Ok(())
    }
}


/// State of all accounts in the system expressed in Plain Old Data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PodShards(BTreeMap<u32, PodShardMetadata>);

impl Deref for PodShards {
    type Target = BTreeMap<u32, PodShardMetadata>;

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.0
    }
}

impl From<cjson::spec::Shards> for PodShards {
    fn from(s: cjson::spec::Shards) -> PodShards {
        let shards = s.into_iter().map(|(shard_id, shard)| (shard_id, shard.into())).collect();
        PodShards(shards)
    }
}

impl fmt::Display for PodShards {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (shard_id, shard) in &self.0 {
            writeln!(f, "{}: {}", shard_id, shard)?;
        }
        Ok(())
    }
}
