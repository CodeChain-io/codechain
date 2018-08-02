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

use ccore::{ShardValidatorConfig, StratumConfig};
use ckey::Address;
use clap;
use cnetwork::{NetworkConfig, SocketAddr};
use rpc::{RpcHttpConfig, RpcIpcConfig};
use toml;

use self::chain_type::ChainType;
use super::constants::DEFAULT_CONFIG_PATH;

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
    pub stratum: Stratum,
    pub shard_validator: ShardValidator,
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
    pub keys_path: Option<String>,
    pub chain: ChainType,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mining {
    pub author: Option<Address>,
    pub engine_signer: Option<Address>,
    pub password_path: Option<String>,
    pub mem_pool_size: usize,
    pub mem_pool_mem_limit: usize,
    pub notify_work: Vec<String>,
    pub force_sealing: bool,
    pub reseal_min_period: u64,
    pub reseal_max_period: u64,
    pub work_queue_size: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Network {
    pub address: String,
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
    pub interface: String,
    pub port: u16,
    #[serde(default = "default_enable_devel_api")]
    pub enable_devel_api: bool,
}

fn default_enable_devel_api() -> bool {
    cfg!(debug_assertions)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub disable: bool,
    pub path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stratum {
    pub disable: bool,
    pub port: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShardValidator {
    pub disable: bool,
    pub account: Option<Address>,
    pub password_path: Option<String>,
}

impl<'a> Into<RpcIpcConfig> for &'a Ipc {
    fn into(self) -> RpcIpcConfig {
        debug_assert!(!self.disable);

        RpcIpcConfig {
            socket_addr: self.path.clone(),
        }
    }
}

impl<'a> Into<NetworkConfig> for &'a Network {
    fn into(self) -> NetworkConfig {
        debug_assert!(!self.disable);

        let bootstrap_addresses =
            self.bootstrap_addresses.iter().map(|s| SocketAddr::from_str(s).unwrap()).collect::<Vec<_>>();
        NetworkConfig {
            port: self.port,
            bootstrap_addresses,
            min_peers: self.min_peers,
            max_peers: self.max_peers,
            address: self.address.to_string(),
        }
    }
}

impl<'a> Into<RpcHttpConfig> for &'a Rpc {
    // FIXME: Add interface, cors and hosts options.
    fn into(self) -> RpcHttpConfig {
        debug_assert!(!self.disable);

        RpcHttpConfig {
            interface: self.interface.clone(),
            port: self.port,
            cors: None,
            hosts: None,
        }
    }
}

impl<'a> Into<StratumConfig> for &'a Stratum {
    // FIXME: Add listen_addr and secret
    fn into(self) -> StratumConfig {
        debug_assert!(!self.disable);

        StratumConfig {
            listen_addr: "127.0.0.1".to_string(),
            port: self.port,
            secret: None,
        }
    }
}

impl<'a> Into<ShardValidatorConfig> for &'a ShardValidator {
    fn into(self) -> ShardValidatorConfig {
        debug_assert!(self.disable);

        ShardValidatorConfig {
            account: self.account.unwrap(),
            password_path: self.password_path.clone(),
        }
    }
}

impl Ipc {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-ipc") {
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
        if let Some(keys_path) = matches.value_of("keys-path") {
            self.keys_path = Some(keys_path.to_string());
        }
        if let Some(chain) = matches.value_of("chain") {
            self.chain = chain.parse()?;
        }
        Ok(())
    }
}

impl Mining {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if let Some(author) = matches.value_of("author") {
            self.author = Some(parse_address(author)?);
        }
        if let Some(engine_signer) = matches.value_of("engine-signer") {
            self.engine_signer = Some(parse_address(engine_signer)?);
        }
        if let Some(password_path) = matches.value_of("password-path") {
            self.password_path = Some(password_path.to_string());
        }
        if let Some(mem_pool_mem_limit) = matches.value_of("mem-pool-mem-limit") {
            self.mem_pool_mem_limit = mem_pool_mem_limit.parse().map_err(|_| "Invalid mem limit")?;
        }
        if let Some(mem_pool_size) = matches.value_of("mem-pool-size") {
            self.mem_pool_size = mem_pool_size.parse().map_err(|_| "Invalid size")?;
        }
        if let Some(notify_work) = matches.values_of("notify-work") {
            self.notify_work = notify_work.into_iter().map(|a| a.into()).collect();
        }
        if matches.is_present("force-sealing") {
            self.force_sealing = true;
        }
        if let Some(reseal_min_period) = matches.value_of("reseal-min-period") {
            self.reseal_min_period = reseal_min_period.parse().map_err(|_| "Invalid period")?;
        }
        if let Some(reseal_max_period) = matches.value_of("reseal-max-period") {
            self.reseal_max_period = reseal_max_period.parse().map_err(|_| "Invalid period")?;
        }
        if let Some(work_queue_size) = matches.value_of("work-queue-size") {
            self.work_queue_size = work_queue_size.parse().map_err(|_| "Invalid size")?;
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
        if let Some(interface) = matches.value_of("jsonrpc-interface") {
            self.interface = interface.to_string();
        }
        if matches.is_present("enable-devel-api") {
            self.enable_devel_api = true;
        }
        Ok(())
    }
}

impl Snapshot {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-snapshot") {
            self.disable = true;
        }

        if let Some(snapshot_path) = matches.value_of("snapshot-path") {
            self.path = snapshot_path.to_string();
        }
        Ok(())
    }
}

impl Stratum {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-stratum") {
            self.disable = true;
        }

        if let Some(port) = matches.value_of("stratum-port") {
            self.port = port.parse().map_err(|_| "Invalid port")?;
        }
        Ok(())
    }
}

impl ShardValidator {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-shard-validator") {
            self.disable = true;
        }

        if let Some(account) = matches.value_of("shard-validator") {
            self.account = Some(parse_address(account)?)
        }
        if let Some(password_path) = matches.value_of("shard-validator-password-path") {
            self.password_path = Some(password_path.to_string());
        }

        Ok(())
    }
}

pub fn load_config(matches: &clap::ArgMatches) -> Result<Config, String> {
    let config_path = matches.value_of("config").unwrap_or(DEFAULT_CONFIG_PATH);
    let toml_string = fs::read_to_string(config_path).map_err(|e| format!("Fail to read file: {:?}", e))?;

    let mut config: Config =
        toml::from_str(toml_string.as_ref()).map_err(|e| format!("Error while parsing TOML: {:?}", e))?;
    config.ipc.overwrite_with(&matches)?;
    config.operating.overwrite_with(&matches)?;
    config.mining.overwrite_with(&matches)?;
    config.network.overwrite_with(&matches)?;
    config.rpc.overwrite_with(&matches)?;
    config.snapshot.overwrite_with(&matches)?;
    config.stratum.overwrite_with(&matches)?;
    config.shard_validator.overwrite_with(&matches)?;

    Ok(config)
}

fn parse_address(value: &str) -> Result<Address, String> {
    if value.starts_with("0x") {
        Address::from_str(&value[2..])
    } else {
        Address::from_str(value)
    }.map_err(|_| "Invalid address".to_string())
}
