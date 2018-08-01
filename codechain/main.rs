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

#[macro_use]
extern crate clap;
extern crate futures;

#[macro_use]
extern crate log;
extern crate tokio_core;

#[macro_use]
extern crate serde_derive;

extern crate app_dirs;
extern crate codechain_core as ccore;
extern crate codechain_discovery as cdiscovery;
extern crate codechain_key as ckey;
extern crate codechain_keystore as ckeystore;
#[macro_use]
extern crate codechain_logger as clogger;
extern crate codechain_network as cnetwork;
extern crate codechain_reactor as creactor;
extern crate codechain_rpc as crpc;
extern crate codechain_state as cstate;
extern crate codechain_sync as csync;
extern crate codechain_types as ctypes;
extern crate ctrlc;
extern crate env_logger;
extern crate fdlimit;
extern crate panic_hook;
extern crate parking_lot;
extern crate primitives;
extern crate rpassword;
extern crate toml;

mod account_command;
mod config;
mod constants;
mod rpc;
mod rpc_apis;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use app_dirs::AppInfo;
use ccore::{
    AccountProvider, Client, ClientService, EngineType, Miner, MinerOptions, MinerService, ShardValidator,
    ShardValidatorConfig, Spec, Stratum, StratumConfig, StratumError,
};
use cdiscovery::{KademliaConfig, KademliaExtension, UnstructuredConfig, UnstructuredExtension};
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use clogger::LoggerConfig;
use cnetwork::{NetworkConfig, NetworkControl, NetworkControlError, NetworkService, SocketAddr};
use creactor::EventLoop;
use csync::{BlockSyncExtension, ParcelSyncExtension, SnapshotService};
use ctrlc::CtrlC;
use fdlimit::raise_fd_limit;
use parking_lot::{Condvar, Mutex};
use primitives::H256;

use self::account_command::run_account_command;
use self::config::load_config;
use self::rpc::{rpc_http_start, rpc_ipc_start};

pub const APP_INFO: AppInfo = AppInfo {
    name: "codechain",
    author: "Kodebox",
};

pub fn network_start(cfg: &NetworkConfig) -> Result<Arc<NetworkService>, String> {
    info!("Handshake Listening on {}:{}", cfg.address, cfg.port);

    let addr = cfg.address.parse().map_err(|_| format!("Invalid NETWORK listen host given: {}", cfg.address))?;
    let sockaddress = SocketAddr::new(addr, cfg.port);
    let service = NetworkService::start(sockaddress, cfg.min_peers, cfg.max_peers)
        .map_err(|e| format!("Network service error: {:?}", e))?;

    Ok(service)
}

pub fn discovery_start(service: &NetworkService, cfg: &config::Network) -> Result<(), String> {
    match cfg.discovery_type.as_ref() {
        "unstructured" => {
            let config = UnstructuredConfig {
                bucket_size: cfg.discovery_bucket_size,
                t_refresh: cfg.discovery_refresh,
            };
            let unstructured = UnstructuredExtension::new(config);
            service.set_routing_table(&*unstructured);
            service.register_extension(unstructured)?;
            cinfo!(DISCOVERY, "Node runs with unstructured discovery");
        }
        "kademlia" => {
            let config = KademliaConfig {
                bucket_size: cfg.discovery_bucket_size,
                t_refresh: cfg.discovery_refresh,
            };
            let kademlia = KademliaExtension::new(config);
            service.set_routing_table(&*kademlia);
            service.register_extension(kademlia)?;
            cinfo!(DISCOVERY, "Node runs with kademlia discovery");
        }
        discovery_type => return Err(format!("Unknown discovery {}", discovery_type)),
    }
    Ok(())
}

pub fn client_start(cfg: &config::Config, spec: &Spec, miner: Arc<Miner>) -> Result<ClientService, String> {
    info!("Starting client");
    let client_path = Path::new(&cfg.operating.db_path);
    let client_config = Default::default();
    let service = ClientService::start(client_config, &spec, &client_path, miner)
        .map_err(|e| format!("Client service error: {}", e))?;

    Ok(service)
}

pub fn stratum_start(cfg: &StratumConfig, miner: Arc<Miner>, client: Arc<Client>) -> Result<(), String> {
    match Stratum::start(cfg, miner.clone(), client) {
        // FIXME: Add specified condition like AddrInUse
        Err(StratumError::Service(_)) =>
            Err(format!("STRATUM address {} is already in use, make sure that another instance of a CodeChain node is not running or change the address using the --stratum-port option.", cfg.port)),
        Err(e) => Err(format!("STRATUM start error: {:?}", e)),
        Ok(stratum) => {
            miner.add_work_listener(Box::new(stratum));
            info!("STRATUM Listening on {}", cfg.port);
            Ok(())
        }
    }
}

fn new_shard_validator(config: ShardValidatorConfig, ap: Arc<AccountProvider>) -> Result<Arc<ShardValidator>, String> {
    let account = {
        let password = match config.password_path {
            None => None,
            Some(password_path) => {
                let content = fs::read_to_string(password_path).map_err(|e| format!("{:?}", e))?;
                let password = content.lines().next().ok_or("Password file is empty")?;
                Some(password.to_string())
            }
        };
        Some((config.account, password))
    };
    let shard_validator = ShardValidator::new(account, Arc::clone(&ap));
    Ok(shard_validator)
}

#[cfg(all(unix, target_arch = "x86_64"))]
fn main() -> Result<(), String> {
    panic_hook::set();

    // Always print backtrace on panic.
    ::std::env::set_var("RUST_BACKTRACE", "1");

    run()
}

fn run() -> Result<(), String> {
    let yaml = load_yaml!("codechain.yml");
    let matches = clap::App::from_yaml(yaml).get_matches();

    match matches.subcommand {
        Some(_) => run_subcommand(matches),
        None => run_node(matches),
    }
}

fn run_subcommand(matches: ArgMatches) -> Result<(), String> {
    let subcommand = matches.subcommand.unwrap();
    if subcommand.name == "account" {
        run_account_command(subcommand.matches)
    } else {
        Err("Invalid subcommand".to_string())
    }
}

fn new_miner(config: &config::Config, spec: &Spec, ap: Arc<AccountProvider>) -> Result<Arc<Miner>, String> {
    let miner_options = MinerOptions {
        mem_pool_size: config.mining.mem_pool_size,
        mem_pool_memory_limit: match config.mining.mem_pool_mem_limit {
            0 => None,
            mem_size => Some(mem_size * 1024 * 1024),
        },
        new_work_notify: config.mining.notify_work.clone(),
        force_sealing: config.mining.force_sealing,
        reseal_min_period: Duration::from_millis(config.mining.reseal_min_period),
        reseal_max_period: Duration::from_millis(config.mining.reseal_max_period),
        work_queue_size: config.mining.work_queue_size,
        ..MinerOptions::default()
    };

    let miner = Miner::new(miner_options, spec, Some(ap.clone()));


    match miner.engine_type() {
        EngineType::PoW => {
            let author = config.mining.author;
            match author {
                Some(author) => miner.set_author(author),
                None => return Err("mining.author is not specified".to_string()),
            }
        }
        EngineType::InternalSealing => match config.mining.engine_signer {
            Some(engine_signer) => match ap.has_account(&engine_signer) {
                Ok(has_account) if !has_account => {
                    return Err("mining.engine_signer is not found in AccountProvider".to_string())
                }
                Ok(..) => match config.mining.password_path {
                    None => return Err("mining.password_path is not specified".to_string()),
                    Some(ref password_path) => match fs::read_to_string(password_path) {
                        Ok(content) => {
                            // Read the first line as password.
                            let password = content.lines().next().ok_or("Password file is empty")?;
                            miner
                                .set_engine_signer(engine_signer, password.to_string())
                                .map_err(|e| format!("{:?}", e))?
                        }
                        Err(_) => return Err(format!("Failed to read the password file")),
                    },
                },
                Err(e) => {
                    return Err(format!("Error while checking whether engine_signer is in AccountProvider: {:?}", e))
                }
            },
            None => return Err("mining.engine_signer is not specified".to_string()),
        },
        EngineType::Solo => (),
    }

    Ok(miner)
}

struct DummyNetworkService {}

impl DummyNetworkService {
    fn new() -> Self {
        DummyNetworkService {}
    }
}

impl NetworkControl for DummyNetworkService {
    fn register_secret(&self, _secret: H256, _addr: SocketAddr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn connect(&self, _addr: SocketAddr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn disconnect(&self, _addr: SocketAddr) -> Result<(), NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }

    fn is_connected(&self, _addr: &SocketAddr) -> Result<bool, NetworkControlError> {
        Err(NetworkControlError::Disabled)
    }
}

fn run_node(matches: ArgMatches) -> Result<(), String> {
    // increase max number of open files
    raise_fd_limit();

    let _event_loop = EventLoop::spawn();
    let config = load_config(&matches)?;

    let spec = config.operating.chain.spec()?;

    let instance_id = config.operating.instance_id.unwrap_or(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Current time should be later than unix epoch")
            .subsec_nanos() as usize,
    );
    clogger::init(&LoggerConfig::new(instance_id)).expect("Logger must be successfully initialized");

    let keys_path = match config.operating.keys_path {
        Some(ref keys_path) => keys_path.clone(),
        None => constants::DEFAULT_KEYS_PATH.to_string(),
    };
    let keystore_dir = RootDiskDirectory::create(keys_path).map_err(|_| "Cannot read key path directory")?;
    let keystore = KeyStore::open(Box::new(keystore_dir)).map_err(|_| "Cannot open key store")?;
    let ap = AccountProvider::new(keystore);
    let miner = new_miner(&config, &spec, ap.clone())?;
    let client = client_start(&config, &spec, miner.clone())?;

    let shard_validator = if config.shard_validator.disable {
        ShardValidator::new(None, Arc::clone(&ap))
    } else {
        let shard_validator_config = (&config.shard_validator).into();
        new_shard_validator(shard_validator_config, Arc::clone(&ap))?
    };

    let network_service: Arc<NetworkControl> = {
        if !config.network.disable {
            let network_config = (&config.network).into();
            let service = network_start(&network_config)?;

            if config.network.discovery {
                discovery_start(&service, &config.network)?;
            } else {
                cwarn!(DISCOVERY, "Node runs without discovery extension");
            }

            if config.network.sync {
                let sync = BlockSyncExtension::new(client.client());
                service.register_extension(sync.clone())?;
                client.client().add_notify(sync.clone());
            }
            if config.network.parcel_relay {
                service.register_extension(ParcelSyncExtension::new(client.client()))?;
            }
            if let Some(consensus_extension) = spec.engine.network_extension() {
                service.register_extension(consensus_extension)?;
            }

            service.register_extension(shard_validator.clone())?;

            for address in network_config.bootstrap_addresses {
                service.connect_to(address)?;
            }
            service
        } else {
            Arc::new(DummyNetworkService::new())
        }
    };

    let rpc_apis_deps = Arc::new(rpc_apis::ApiDependencies {
        client: client.client(),
        miner: Arc::clone(&miner),
        network_control: Arc::clone(&network_service),
        account_provider: ap,
        shard_validator,
    });

    let _rpc_server = {
        if !config.rpc.disable {
            let rpc_config = (&config.rpc).into();
            Some(rpc_http_start(rpc_config, config.rpc.enable_devel_api, Arc::clone(&rpc_apis_deps))?)
        } else {
            None
        }
    };

    let _ipc_server = {
        if !config.ipc.disable {
            let ipc_config = (&config.ipc).into();
            Some(rpc_ipc_start(ipc_config, config.rpc.enable_devel_api, Arc::clone(&rpc_apis_deps))?)
        } else {
            None
        }
    };

    if !config.stratum.disable {
        let stratum_config = (&config.stratum).into();
        stratum_start(&stratum_config, Arc::clone(&miner), client.client())?
    }

    let _snapshot_service = {
        if !config.snapshot.disable {
            let service = SnapshotService::new(client.client(), config.snapshot.path, spec.params().snapshot_period);
            client.client().add_notify(service.clone());
            Some(service)
        } else {
            None
        }
    };

    // drop the spec to free up genesis state.
    drop(spec);

    cinfo!(TEST_SCRIPT, "Initialization complete");

    wait_for_exit();

    Ok(())
}

fn wait_for_exit() {
    let exit = Arc::new((Mutex::new(()), Condvar::new()));

    // Handle possible exits
    let e = exit.clone();
    CtrlC::set_handler(move || {
        e.1.notify_all();
    });

    // Wait for signal
    let mut l = exit.0.lock();
    let _ = exit.1.wait(&mut l);
}
