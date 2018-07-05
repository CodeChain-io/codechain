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

use std::str::FromStr;
use std::{fmt, fs};

use ccore::Spec;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainType {
    Solo,
    SoloAuthority,
    Tendermint,
    Cuckoo,
    Custom(String),
}

impl Default for ChainType {
    fn default() -> Self {
        ChainType::Tendermint
    }
}

impl FromStr for ChainType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let spec = match s {
            "solo" => ChainType::Solo,
            "solo_authority" => ChainType::SoloAuthority,
            "tendermint" => ChainType::Tendermint,
            "cuckoo" => ChainType::Cuckoo,
            other => ChainType::Custom(other.into()),
        };
        Ok(spec)
    }
}

impl fmt::Display for ChainType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ChainType::Solo => "solo",
            ChainType::SoloAuthority => "solo_authority",
            ChainType::Tendermint => "tendermint",
            ChainType::Cuckoo => "cuckoo",
            ChainType::Custom(custom) => custom,
        })
    }
}

impl ChainType {
    pub fn spec<'a>(&self) -> Result<Spec, String> {
        match self {
            ChainType::Solo => Ok(Spec::new_test_solo()),
            ChainType::SoloAuthority => Ok(Spec::new_test_solo_authority()),
            ChainType::Tendermint => Ok(Spec::new_test_tendermint()),
            ChainType::Cuckoo => Ok(Spec::new_test_cuckoo()),
            ChainType::Custom(filename) => {
                let file = fs::File::open(filename)
                    .map_err(|e| format!("Could not load specification file at {}: {}", filename, e))?;
                Spec::load(file)
            }
        }
    }
}
