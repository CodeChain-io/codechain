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

mod chain_type;

use std::fs;
use std::str::FromStr;

use clap;
use cnetwork::{NetworkConfig, SocketAddr};
use ctypes::{Address, Secret};
use rpc::{HttpConfiguration as RpcHttpConfig, IpcConfiguration as RpcIpcConfig};
use toml;

use self::chain_type::ChainType;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub ipc: Ipc,
    #[serde(rename = "codechain")]
    pub operating: Operating,
    pub mining: Mining,
    pub network: Network,
    pub rpc: Rpc,
    pub snapshot: Snapshot,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ipc {
    pub disable: bool,
    pub path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operating {
    pub quiet: bool,
    pub instance_id: Option<usize>,
    pub db_path: String,
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
    pub discovery: bool,
    pub discovery_type: String,
    pub discovery_refresh: u32,
    pub discovery_bucket_size: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rpc {
    pub disable: bool,
    pub port: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub path: String,
}

impl<'a> Into<RpcIpcConfig> for &'a Ipc {
    fn into(self) -> RpcIpcConfig {
        RpcIpcConfig {
            socket_addr: self.path.clone(),
        }
    }
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

impl Ipc {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-jsonrpc") {
            self.disable = true;
        }
        if let Some(path) = matches.value_of("ipc-path") {
            self.path = path.to_string();
        }
        Ok(())
    }
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
            return Err("Invalid min/max peers".to_string())
        }

        if matches.is_present("no-sync") {
            self.sync = false;
        }
        if matches.is_present("no-parcel-relay") {
            self.parcel_relay = false;
        }

        if matches.is_present("no-discovery") {
            self.discovery = false;
        }
        if let Some(discovery_type) = matches.value_of("discovery") {
            self.discovery_type = discovery_type.to_string();
        }
        if let Some(refresh) = matches.value_of("discovery-refresh") {
            self.discovery_refresh = refresh.parse().map_err(|_| "Invalid discovery-refresh")?;
        }
        if let Some(bucket_size) = matches.value_of("discovery-bucket-size") {
            self.discovery_bucket_size = bucket_size.parse().map_err(|_| "Invalid discovery-bucket-size")?;
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

impl Snapshot {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if let Some(snapshot_path) = matches.value_of("snapshot-path") {
            self.path = snapshot_path.to_string();
        }
        Ok(())
    }
}
