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
use rpc::{HttpConfiguration as RpcHttpConfig, IpcConfiguration as RpcIpcConfig};
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
    #[serde(rename = "codechain")]
    pub operating: Operating,
    pub mining: Mining,
    pub network: Network,
    pub rpc: Rpc,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operating {
    pub quiet: bool,
    pub instance_id: Option<usize>,
    pub db_path: String,
    pub snapshot_path: String,
    pub chain: ChainType,
    pub secret_key: Secret,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mining {
    pub author: Option<Address>,
    pub engine_signer: Option<Address>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Network {
    pub disable: bool,
    pub port: u16,
    pub bootstrap_addresses: Vec<String>,
    pub min_peers: usize,
    pub max_peers: usize,
    pub sync: bool,
    pub parcel_relay: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rpc {
    pub disable: bool,
    pub port: u16,
}

impl<'a> Into<NetworkConfig> for &'a Network {
    fn into(self) -> NetworkConfig {
        let bootstrap_addresses =
            self.bootstrap_addresses.iter().map(|s| SocketAddr::from_str(s).unwrap()).collect::<Vec<_>>();
        NetworkConfig {
            port: self.port,
            bootstrap_addresses,
            min_peers: self.min_peers,
            max_peers: self.max_peers,
        }
    }
}

impl<'a> Into<RpcHttpConfig> for &'a Rpc {
    // FIXME: Add interface, cors and hosts options.
    fn into(self) -> RpcHttpConfig {
        RpcHttpConfig {
            interface: "127.0.0.1".to_string(),
            port: self.port,
            cors: None,
            hosts: None,
        }
    }
}

pub fn load(config_path: &str) -> Result<Config, String> {
    let toml_string = fs::read_to_string(config_path).map_err(|e| format!("Fail to read file: {:?}", e))?;
    toml::from_str(toml_string.as_ref()).map_err(|e| format!("Error while parse TOML: {:?}", e))
}

impl Operating {
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
        if let Some(snapshot_path) = matches.value_of("snapshot-path") {
            self.snapshot_path = snapshot_path.to_string();
        }
        if let Some(chain) = matches.value_of("chain") {
            self.chain = chain.parse()?;
        }
        if let Some(secret) = matches.value_of("secret-key") {
            self.secret_key = Secret::from_str(secret).map_err(|_| "Invalid secret key")?;
        }
        Ok(())
    }
}

impl Mining {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if let Some(author) = matches.value_of("author") {
            self.author = Some(Address::from_str(author).map_err(|_| "Invalid address")?);
        }
        if let Some(engine_signer) = matches.value_of("engine-signer") {
            self.engine_signer = Some(Address::from_str(engine_signer).map_err(|_| "Invalid address")?);
        }
        Ok(())
    }
}

impl Network {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-network") {
            self.disable = true;
        }

        if let Some(addresses) = matches.values_of("bootstrap-addresses") {
            self.bootstrap_addresses = addresses.into_iter().map(|a| a.into()).collect();
        }

        if let Some(port) = matches.value_of("port") {
            self.port = port.parse().map_err(|_| "Invalid port")?;
        }

        if let Some(min_peers) = matches.value_of("min-peers") {
            self.min_peers = min_peers.parse().map_err(|_| "Invalid min-peers")?;
        }
        if let Some(max_peers) = matches.value_of("min-peers") {
            self.max_peers = max_peers.parse().map_err(|_| "Invalid max-peers")?;
        }
        if self.min_peers > self.max_peers {
            return Err("Invalid min/max peers".to_owned())
        }

        if matches.is_present("no-sync") {
            self.sync = false;
        }
        if matches.is_present("no-parcel-relay") {
            self.parcel_relay = false;
        }
        Ok(())
    }
}

impl Rpc {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-jsonrpc") {
            self.disable = true;
        }

        if let Some(port) = matches.value_of("jsonrpc-port") {
            self.port = port.parse().map_err(|_| "Invalid port")?;
        }
        Ok(())
    }
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
    let bucket_size = match matches.value_of("discovery-bucket-size") {
        Some(k) => Some(k.parse().map_err(|_| "Invalid discovery-bucket-size")?),
        None => None,
    };

    match matches.value_of("discovery") {
        Some("unstructured") => Ok(Some(Discovery::Unstructured(UnstructuredConfig::new(bucket_size, refresh)))),
        Some("kademlia") => Ok(Some(Discovery::Kademlia(KademliaConfig::new(bucket_size, refresh)))),
        _ => unreachable!(),
    }
}

pub fn parse_rpc_ipc_config(matches: &clap::ArgMatches) -> Result<Option<RpcIpcConfig>, String> {
    if matches.is_present("no-jsonrpc") {
        return Ok(None)
    }

    let socket_addr = value_t_or_exit!(matches, "ipc-path", String);

    Ok(Some(RpcIpcConfig {
        socket_addr,
    }))
}
