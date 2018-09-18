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
use std::net::IpAddr;
use std::str::{self, FromStr};
use std::time::Duration;

use ccore::{MinerOptions, ShardValidatorConfig, StratumConfig};
use ckey::PlatformAddress;
use clap;
use cnetwork::{NetworkConfig, SocketAddr};
use rpc::{RpcHttpConfig, RpcIpcConfig};
use toml;

pub use self::chain_type::ChainType;

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

impl Config {
    pub fn miner_options(&self) -> Result<MinerOptions, String> {
        let (reseal_on_own_parcel, reseal_on_external_parcel) =
            match self.mining.reseal_on_txs.as_ref().map(|s| s.as_str()) {
                Some("all") => (true, true),
                Some("own") => (true, false),
                Some("ext") => (false, true),
                Some("none") => (false, false),
                Some(x) => {
                    return Err(format!(
                        "{} isn't a valid value for reseal-on-txs. Possible values are all, own, ext, none",
                        x
                    ))
                }
                None => unreachable!(),
            };

        Ok(MinerOptions {
            mem_pool_size: self.mining.mem_pool_size.unwrap(),
            mem_pool_memory_limit: match self.mining.mem_pool_mem_limit.unwrap() {
                0 => None,
                mem_size => Some(mem_size * 1024 * 1024),
            },
            new_work_notify: self.mining.notify_work.clone().unwrap(),
            force_sealing: self.mining.force_sealing.unwrap(),
            reseal_on_own_parcel,
            reseal_on_external_parcel,
            reseal_min_period: Duration::from_millis(self.mining.reseal_min_period.unwrap()),
            reseal_max_period: Duration::from_millis(self.mining.reseal_max_period.unwrap()),
            work_queue_size: self.mining.work_queue_size.unwrap(),
            ..MinerOptions::default()
        })
    }

    pub fn rpc_http_config(&self) -> RpcHttpConfig {
        debug_assert!(!self.rpc.disable.unwrap());

        // FIXME: Add interface, cors and hosts options.
        RpcHttpConfig {
            interface: self.rpc.interface.clone().unwrap(),
            port: self.rpc.port.unwrap(),
            cors: None,
            hosts: None,
        }
    }

    pub fn rpc_ipc_config(&self) -> RpcIpcConfig {
        debug_assert!(!self.ipc.disable.unwrap());

        RpcIpcConfig {
            socket_addr: self.ipc.path.clone().unwrap(),
        }
    }

    pub fn network_config(&self) -> Result<NetworkConfig, String> {
        debug_assert!(!self.network.disable.unwrap());

        fn make_ipaddr_list(list_path: Option<&String>, list_name: &str) -> Result<Vec<IpAddr>, String> {
            list_path
                .map(|path| {
                    fs::read_to_string(path)
                        .map_err(|e| format!("Cannot open the {}list file {:?}: {:?}", list_name, path, e))
                        .map(|rstr| {
                            rstr.split_whitespace()
                                .filter(|s| s.len() != 0)
                                .map(|s| s.parse().map_err(|e| (s, e)))
                                .collect::<Result<Vec<_>, _>>()
                                .map_err(|(s, e)| format!("Cannot parse IP address {:?}: {:?}", s, e))
                        })
                        .unwrap_or_else(|e| Err(e))
                })
                .unwrap_or(Ok(Vec::new()))
        }

        let bootstrap_addresses = self
            .network
            .bootstrap_addresses
            .clone()
            .unwrap()
            .iter()
            .map(|s| SocketAddr::from_str(s).unwrap())
            .collect::<Vec<_>>();

        let whitelist = make_ipaddr_list(self.network.whitelist_path.as_ref(), "white")?;
        let blacklist = make_ipaddr_list(self.network.blacklist_path.as_ref(), "black")?;

        Ok(NetworkConfig {
            address: self.network.interface.clone().unwrap(),
            port: self.network.port.unwrap(),
            bootstrap_addresses,
            min_peers: self.network.min_peers.unwrap(),
            max_peers: self.network.max_peers.unwrap(),
            whitelist,
            blacklist,
        })
    }

    pub fn stratum_config(&self) -> StratumConfig {
        debug_assert!(!self.stratum.disable.unwrap());

        // FIXME: Add listen_addr and secret
        StratumConfig {
            listen_addr: "127.0.0.1".to_string(),
            port: self.stratum.port.unwrap(),
            secret: None,
        }
    }

    pub fn shard_validator_config(&self) -> ShardValidatorConfig {
        debug_assert!(self.shard_validator.disable.unwrap());

        ShardValidatorConfig {
            account: self.shard_validator.account.unwrap().into_address(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ipc {
    pub disable: Option<bool>,
    pub path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operating {
    pub quiet: Option<bool>,
    pub instance_id: Option<usize>,
    pub db_path: Option<String>,
    pub keys_path: Option<String>,
    pub password_path: Option<String>,
    pub chain: Option<ChainType>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mining {
    pub disable: Option<bool>,
    pub author: Option<PlatformAddress>,
    pub engine_signer: Option<PlatformAddress>,
    pub mem_pool_size: Option<usize>,
    pub mem_pool_mem_limit: Option<usize>,
    pub notify_work: Option<Vec<String>>,
    pub force_sealing: Option<bool>,
    pub reseal_on_txs: Option<String>,
    pub reseal_min_period: Option<u64>,
    pub reseal_max_period: Option<u64>,
    pub work_queue_size: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Network {
    pub interface: Option<String>,
    pub disable: Option<bool>,
    pub port: Option<u16>,
    pub bootstrap_addresses: Option<Vec<String>>,
    pub min_peers: Option<usize>,
    pub max_peers: Option<usize>,
    pub sync: Option<bool>,
    pub parcel_relay: Option<bool>,
    pub discovery: Option<bool>,
    pub discovery_type: Option<String>,
    pub discovery_refresh: Option<u32>,
    pub discovery_bucket_size: Option<u8>,
    pub blacklist_path: Option<String>,
    pub whitelist_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rpc {
    pub disable: Option<bool>,
    pub interface: Option<String>,
    pub port: Option<u16>,
    #[serde(default = "default_enable_devel_api")]
    pub enable_devel_api: bool,
}

fn default_enable_devel_api() -> bool {
    cfg!(debug_assertions)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub disable: Option<bool>,
    pub path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stratum {
    pub disable: Option<bool>,
    pub port: Option<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShardValidator {
    pub disable: Option<bool>,
    pub account: Option<PlatformAddress>,
}

impl Ipc {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-ipc") {
            self.disable = Some(true);
        }
        if let Some(path) = matches.value_of("ipc-path") {
            self.path = Some(path.to_string());
        }
        Ok(())
    }
}

impl Operating {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("quiet") {
            self.quiet = Some(true);
        }
        if let Some(instance_id) = matches.value_of("instance-id") {
            self.instance_id = Some(instance_id.parse().map_err(|e| format!("{}", e))?);
        }
        if let Some(db_path) = matches.value_of("db-path") {
            self.db_path = Some(db_path.to_string());
        }
        if let Some(keys_path) = matches.value_of("keys-path") {
            self.keys_path = Some(keys_path.to_string());
        }
        if let Some(password_path) = matches.value_of("password-path") {
            self.password_path = Some(password_path.to_string());
        }
        if let Some(chain) = matches.value_of("chain") {
            self.chain = Some(chain.parse()?);
        }
        Ok(())
    }
}

impl Mining {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-miner") {
            self.disable = Some(true);
        }

        if let Some(author) = matches.value_of("author") {
            self.author = Some(author.parse().map_err(|_| "Invalid address format")?);
        }
        if let Some(engine_signer) = matches.value_of("engine-signer") {
            self.engine_signer = Some(engine_signer.parse().map_err(|_| "Invalid address format")?);
        }
        if let Some(mem_pool_mem_limit) = matches.value_of("mem-pool-mem-limit") {
            self.mem_pool_mem_limit = Some(mem_pool_mem_limit.parse().map_err(|_| "Invalid mem limit")?);
        }
        if let Some(mem_pool_size) = matches.value_of("mem-pool-size") {
            self.mem_pool_size = Some(mem_pool_size.parse().map_err(|_| "Invalid size")?);
        }
        if let Some(notify_work) = matches.values_of("notify-work") {
            self.notify_work = Some(notify_work.into_iter().map(|a| a.into()).collect());
        }
        if matches.is_present("force-sealing") {
            self.force_sealing = Some(true);
        }
        if let Some(reseal_on_txs) = matches.value_of("reseal-on-txs") {
            self.reseal_on_txs = Some(reseal_on_txs.to_string());
        }
        if let Some(reseal_min_period) = matches.value_of("reseal-min-period") {
            self.reseal_min_period = Some(reseal_min_period.parse().map_err(|_| "Invalid period")?);
        }
        if let Some(reseal_max_period) = matches.value_of("reseal-max-period") {
            self.reseal_max_period = Some(reseal_max_period.parse().map_err(|_| "Invalid period")?);
        }
        if let Some(work_queue_size) = matches.value_of("work-queue-size") {
            self.work_queue_size = Some(work_queue_size.parse().map_err(|_| "Invalid size")?);
        }
        Ok(())
    }
}

impl Network {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-network") {
            self.disable = Some(true);
        }

        if let Some(addresses) = matches.values_of("bootstrap-addresses") {
            self.bootstrap_addresses = Some(addresses.into_iter().map(|a| a.into()).collect());
        }

        if let Some(interface) = matches.value_of("interface") {
            self.interface = Some(interface.to_string());
        }
        if let Some(port) = matches.value_of("port") {
            self.port = Some(port.parse().map_err(|_| "Invalid port")?);
        }

        if let Some(min_peers) = matches.value_of("min-peers") {
            self.min_peers = Some(min_peers.parse().map_err(|_| "Invalid min-peers")?);
        }
        if let Some(max_peers) = matches.value_of("min-peers") {
            self.max_peers = Some(max_peers.parse().map_err(|_| "Invalid max-peers")?);
        }
        if self.min_peers > self.max_peers {
            return Err("Invalid min/max peers".to_string())
        }

        if matches.is_present("no-sync") {
            self.sync = Some(false);
        }
        if matches.is_present("no-parcel-relay") {
            self.parcel_relay = Some(false);
        }

        if matches.is_present("no-discovery") {
            self.discovery = Some(false);
        }
        if let Some(discovery_type) = matches.value_of("discovery") {
            self.discovery_type = Some(discovery_type.to_string());
        }
        if let Some(refresh) = matches.value_of("discovery-refresh") {
            self.discovery_refresh = Some(refresh.parse().map_err(|_| "Invalid discovery-refresh")?);
        }
        if let Some(bucket_size) = matches.value_of("discovery-bucket-size") {
            self.discovery_bucket_size = Some(bucket_size.parse().map_err(|_| "Invalid discovery-bucket-size")?);
        }

        if let Some(file_path) = matches.value_of("whitelist-path") {
            self.whitelist_path = Some(file_path.to_string());
        }
        if let Some(file_path) = matches.value_of("blacklist-path") {
            self.blacklist_path = Some(file_path.to_string());
        }

        Ok(())
    }
}

impl Rpc {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-jsonrpc") {
            self.disable = Some(true);
        }

        if let Some(port) = matches.value_of("jsonrpc-port") {
            self.port = Some(port.parse().map_err(|_| "Invalid port")?);
        }
        if let Some(interface) = matches.value_of("jsonrpc-interface") {
            self.interface = Some(interface.to_string());
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
            self.disable = Some(true);
        }

        if let Some(snapshot_path) = matches.value_of("snapshot-path") {
            self.path = Some(snapshot_path.to_string());
        }
        Ok(())
    }
}

impl Stratum {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-stratum") {
            self.disable = Some(true);
        }

        if let Some(port) = matches.value_of("stratum-port") {
            self.port = Some(port.parse().map_err(|_| "Invalid port")?);
        }
        Ok(())
    }
}

impl ShardValidator {
    pub fn overwrite_with(&mut self, matches: &clap::ArgMatches) -> Result<(), String> {
        if matches.is_present("no-shard-validator") {
            self.disable = Some(true);
        }

        if let Some(account) = matches.value_of("shard-validator") {
            self.account = Some(account.parse().map_err(|_| "Invalid address format")?)
        }

        Ok(())
    }
}

#[cfg(not(debug_assertions))]
pub fn read_preset_config() -> &'static str {
    let bytes = include_bytes!("presets/config.prod.toml");
    str::from_utf8(bytes).expect("The preset config file must be valid")
}

#[cfg(debug_assertions)]
pub fn read_preset_config() -> &'static str {
    let bytes = include_bytes!("presets/config.dev.toml");
    str::from_utf8(bytes).expect("The preset config file must be valid")
}

pub fn load_config(matches: &clap::ArgMatches) -> Result<Config, String> {
    let toml_string = match matches.value_of("config") {
        Some(config_path) => fs::read_to_string(config_path).map_err(|e| format!("Fail to read file: {:?}", e))?,
        None => read_preset_config().to_string(),
    };

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
