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
extern crate codechain_logger as clogger;
extern crate codechain_network as cnetwork;
extern crate codechain_reactor as creactor;
extern crate codechain_rpc as crpc;
extern crate codechain_sync as csync;
extern crate codechain_types as ctypes;
extern crate ctrlc;
extern crate env_logger;
extern crate fdlimit;
extern crate panic_hook;
extern crate parking_lot;
extern crate toml;

mod config;
mod rpc;
mod rpc_apis;

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use app_dirs::AppInfo;
use ccore::{AccountProvider, ClientService, Miner, MinerOptions, MinerService, Spec};
use cdiscovery::{KademliaExtension, SimpleDiscovery};
use clogger::LoggerConfig;
use cnetwork::{NetworkConfig, NetworkService, SocketAddr};
use creactor::EventLoop;
use crpc::Server as RpcServer;
use csync::{BlockSyncExtension, ParcelSyncExtension};
use ctrlc::CtrlC;
use fdlimit::raise_fd_limit;
use parking_lot::{Condvar, Mutex};
use rpc::HttpConfiguration as RpcHttpConfig;

const DEFAULT_CONFIG_PATH: &'static str = "codechain/config/presets/config.dev.toml";

pub const APP_INFO: AppInfo = AppInfo {
    name: "codechain",
    author: "Kodebox",
};

pub fn rpc_start(cfg: RpcHttpConfig, deps: Arc<rpc_apis::ApiDependencies>) -> Result<RpcServer, String> {
    info!("RPC Listening on {}", cfg.port);
    rpc::new_http(cfg, deps)
}

pub fn network_start(cfg: &NetworkConfig) -> Result<NetworkService, String> {
    info!("Handshake Listening on {}", cfg.port);
    let address = SocketAddr::v4(127, 0, 0, 1, cfg.port);
    let service = NetworkService::start(address, cfg.min_peers, cfg.max_peers)
        .map_err(|e| format!("Network service error: {:?}", e))?;

    Ok(service)
}

pub fn client_start(cfg: &config::Config, spec: &Spec, miner: Arc<Miner>) -> Result<ClientService, String> {
    info!("Starting client");
    let client_path = Path::new(&cfg.db_path);
    let client_config = Default::default();
    let service = ClientService::start(client_config, &spec, &client_path, miner)
        .map_err(|e| format!("Client service error: {:?}", e))?;

    Ok(service)
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

    // increase max number of open files
    raise_fd_limit();

    let _event_loop = EventLoop::spawn();

    let config_path = matches.value_of("config-path").unwrap_or(DEFAULT_CONFIG_PATH);
    let mut config = config::load(&config_path)?;
    config.overwrite_with(&matches)?;
    let spec = config.chain_type.spec()?;

    let instance_id = config.instance_id.unwrap_or(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Current time should be later than unix epoch")
        .subsec_nanos() as usize);
    clogger::init(&LoggerConfig::new(instance_id)).expect("Logger must be successfully initialized");

    let ap = AccountProvider::new();
    let address = ap.insert_account(config.secret_key.into()).map_err(|e| format!("Invalid secret key: {:?}", e))?;

    let miner = Miner::new(MinerOptions::default(), &spec, Some(ap.clone()));
    let author = config.author.unwrap_or(address);
    miner.set_author(author);
    let enginer_signer = config.engine_signer.unwrap_or(address);
    miner.set_engine_signer(enginer_signer).map_err(|err| format!("{:?}", err))?;

    let client = client_start(&config, &spec, miner.clone())?;

    let rpc_apis_deps = Arc::new(rpc_apis::ApiDependencies {
        client: client.client(),
        miner: miner.clone(),
    });

    let _rpc_server = {
        if let Some(rpc_config) = config::parse_rpc_config(&matches)? {
            Some(rpc_start(rpc_config, rpc_apis_deps.clone())?)
        } else {
            None
        }
    };

    let _network_service = {
        if let Some(network_config) = config::parse_network_config(&matches)? {
            let service = network_start(&network_config)?;

            if let Some(kademlia_config) = config::parse_kademlia_config(&matches)? {
                let kademlia = Arc::new(KademliaExtension::new(kademlia_config));
                service.register_extension(kademlia.clone())?;
                service.set_discovery_api(kademlia);
            } else {
                service.set_discovery_api(Arc::new(SimpleDiscovery::new()));
            }

            if config.enable_block_sync {
                let sync = BlockSyncExtension::new(client.client());
                service.register_extension(sync.clone())?;
                client.client().add_notify(sync.clone());
            }
            if config.enable_parcel_relay {
                service.register_extension(ParcelSyncExtension::new(client.client()))?;
            }
            if let Some(consensus_extension) = spec.engine.network_extension() {
                service.register_extension(consensus_extension)?;
            }

            for address in network_config.bootstrap_addresses {
                service.connect_to(address)?;
            }
            Some(service)
        } else {
            None
        }
    };

    // drop the spec to free up genesis state.
    drop(spec);

    info!(target: "test_script", "Initialization complete");

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
