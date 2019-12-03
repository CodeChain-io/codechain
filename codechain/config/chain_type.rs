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

use ccore::Scheme;
use never_type::Never;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, PartialEq)]
pub enum ChainType {
    Mainnet,
    Solo,
    SimplePoA,
    Tendermint,
    Cuckoo,
    BlakePoW,
    Husky,
    Saluki,
    Corgi,
    Beagle,
    Custom(String),
}

impl Default for ChainType {
    fn default() -> Self {
        ChainType::Tendermint
    }
}

impl FromStr for ChainType {
    type Err = Never;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let scheme = match s {
            "mainnet" => ChainType::Mainnet,
            "solo" => ChainType::Solo,
            "simple_poa" => ChainType::SimplePoA,
            "tendermint" => ChainType::Tendermint,
            "cuckoo" => ChainType::Cuckoo,
            "blake_pow" => ChainType::BlakePoW,
            "husky" => ChainType::Husky,
            "saluki" => ChainType::Saluki,
            "corgi" => ChainType::Corgi,
            "beagle" => ChainType::Beagle,
            other => ChainType::Custom(other.into()),
        };
        Ok(scheme)
    }
}

impl<'a> Deserialize<'a> for ChainType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>, {
        struct ChainTypeVisitor;

        impl<'a> Visitor<'a> for ChainTypeVisitor {
            type Value = ChainType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a valid chain type string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error, {
                Ok(ChainType::from_str(value).expect("ChainType can always be deserialized"))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: Error, {
                self.visit_str(value.as_ref())
            }
        }

        deserializer.deserialize_any(ChainTypeVisitor)
    }
}

impl fmt::Display for ChainType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ChainType::Mainnet => "mainnet",
            ChainType::Solo => "solo",
            ChainType::SimplePoA => "simple_poa",
            ChainType::Tendermint => "tendermint",
            ChainType::Cuckoo => "cuckoo",
            ChainType::BlakePoW => "blake_pow",
            ChainType::Husky => "husky",
            ChainType::Saluki => "saluki",
            ChainType::Corgi => "corgi",
            ChainType::Beagle => "beagle",
            ChainType::Custom(custom) => custom,
        })
    }
}

impl ChainType {
    pub fn scheme(&self) -> Result<Scheme, String> {
        match self {
            ChainType::Mainnet => Ok(Scheme::new_mainnet()),
            ChainType::Solo => Ok(Scheme::new_test_solo()),
            ChainType::SimplePoA => Ok(Scheme::new_test_simple_poa()),
            ChainType::Tendermint => Ok(Scheme::new_test_tendermint()),
            ChainType::Cuckoo => Ok(Scheme::new_test_cuckoo()),
            ChainType::BlakePoW => Ok(Scheme::new_test_blake_pow()),
            ChainType::Husky => Ok(Scheme::new_husky()),
            ChainType::Saluki => Ok(Scheme::new_saluki()),
            ChainType::Corgi => Ok(Scheme::new_corgi()),
            ChainType::Beagle => Ok(Scheme::new_beagle()),
            ChainType::Custom(filename) => {
                let file = fs::File::open(filename)
                    .map_err(|e| format!("Could not load specification file at {}: {}", filename, e))?;
                Scheme::load(file)
            }
        }
    }
}
