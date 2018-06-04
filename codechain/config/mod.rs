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
use cdiscovery::{KademliaConfig, UnstructuredConfig};
use clap;
use cnetwork::{NetworkConfig, SocketAddr};
use ctypes::{Address, Secret};
use rpc::HttpConfiguration as RpcHttpConfig;
use toml;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainType {
    Solo,
    SoloAuthority,
    Tendermint,
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
            ChainType::Custom(custom) => custom,
        })
    }
}

impl ChainType {
    pub fn spec<'a>(&self) -> Result<Spec, String> {
        match self {
            ChainType::Solo => Ok(Spec::new_solo()),
            ChainType::SoloAuthority => Ok(Spec::new_solo_authority()),
            ChainType::Tendermint => Ok(Spec::new_test_tendermint()),
            ChainType::Custom(filename) => {
                let file = fs::File::open(filename)
                    .map_err(|e| format!("Could not load specification file at {}: {}", filename, e))?;
                Spec::load(file)
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub quiet: bool,
    pub instance_id: Option<usize>,
    pub db_path: String,
    pub chain_type: ChainType,
    pub enable_block_sync: bool,
    pub enable_parcel_relay: bool,
    pub secret_key: Secret,
    pub author: Option<Address>,
    pub engine_signer: Option<Address>,
}

pub fn load(config_path: &str) -> Result<Config, String> {
    let toml_string = fs::read_to_string(config_path).map_err(|e| format!("Fail to read file: {:?}", e))?;
    toml::from_str(toml_string.as_ref()).map_err(|e| format!("Error while parse TOML: {:?}", e))
}

impl Config {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("quiet") {
            self.quiet = true;
        }
        if let Some(instance_id) = matches.value_of("instance-id") {
            self.instance_id = Some(instance_id.parse().map_err(|e| format!("{}", e))?);
        }
        if let Some(db_path) = matches.value_of("db-path") {
            self.db_path = db_path.to_string();
        }
        if let Some(chain) = matches.value_of("chain") {
            self.chain_type = chain.parse()?;
        }
        if matches.is_present("no-sync") {
            self.enable_block_sync = false;
        }
        if matches.is_present("no-parcel-relay") {
            self.enable_parcel_relay = false;
        }
        if let Some(secret) = matches.value_of("secret-key") {
            self.secret_key = Secret::from_str(secret).map_err(|_| "Invalid secret key")?;
        }
        if let Some(author) = matches.value_of("author") {
            self.author = Some(Address::from_str(author).map_err(|_| "Invalid address")?);
        }
        if let Some(engine_signer) = matches.value_of("engine-signer") {
            self.engine_signer = Some(Address::from_str(engine_signer).map_err(|_| "Invalid address")?);
        }
        Ok(())
    }
}

pub fn parse_network_config(matches: &clap::ArgMatches) -> Result<Option<NetworkConfig>, String> {
    if matches.is_present("no-network") {
        return Ok(None)
    }

    let bootstrap_addresses = {
        if let Some(addresses) = matches.values_of("bootstrap-addresses") {
            addresses.map(|s| SocketAddr::from_str(s).unwrap()).collect::<Vec<_>>()
        } else {
            vec![]
        }
    };

    let port = value_t_or_exit!(matches, "port", u16);


    let min_peers = value_t_or_exit!(matches, "min-peers", usize);
    let max_peers = value_t_or_exit!(matches, "max-peers", usize);

    if min_peers > max_peers {
        return Err("Invalid min/max peers".to_owned())
    }

    Ok(Some(NetworkConfig {
        port,
        bootstrap_addresses,
        min_peers,
        max_peers,
    }))
}

pub enum Discovery {
    Kademlia(KademliaConfig),
    Unstructured(UnstructuredConfig),
}

pub fn parse_discovery_config(matches: &clap::ArgMatches) -> Result<Option<Discovery>, String> {
    if matches.is_present("no-discovery") {
        return Ok(None)
    }

    let refresh = match matches.value_of("discovery-refresh") {
        Some(refresh) => Some(refresh.parse().map_err(|_| "Invalid discovery-refresh")?),
        None => None,
    };

    match matches.value_of("discovery") {
        Some("unstructured") => Ok(Some(Discovery::Unstructured(UnstructuredConfig::new(refresh)))),
        Some("kademlia") => {
            let alpha = match matches.value_of("kademlia-alpha") {
                Some(alpha) => Some(alpha.parse().map_err(|_| "Invalid kademlia-alpha")?),
                None => None,
            };
            let k = match matches.value_of("kademlia-k") {
                Some(k) => Some(k.parse().map_err(|_| "Invalid kademlia-k")?),
                None => None,
            };

            Ok(Some(Discovery::Kademlia(KademliaConfig::new(alpha, k, refresh))))
        }
        _ => unreachable!(),
    }
}

pub fn parse_rpc_config(matches: &clap::ArgMatches) -> Result<Option<RpcHttpConfig>, String> {
    if matches.is_present("no-jsonrpc") {
        return Ok(None)
    }

    let port = value_t_or_exit!(matches, "jsonrpc-port", u16);

    let mut config = RpcHttpConfig::with_port(port);

    if let Some(interface) = matches.value_of("jsonrpc-interface") {
        config.interface = interface.to_owned();
    }
    if let Some(cors) = matches.value_of("jsonrpc-cors") {
        config.cors = Some(vec![cors.parse().map_err(|_| "Invalid JSON RPC CORS".to_owned())?]);
    }
    if let Some(hosts) = matches.value_of("jsonrpc-hosts") {
        config.hosts = Some(vec![hosts.parse().map_err(|_| "Invalid JSON RPC hosts".to_owned())?]);
    }

    Ok(Some(config))
}
