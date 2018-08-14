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

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ccore::{
    AccountProvider, Client, ClientService, EngineType, Miner, MinerService, Scheme, ShardValidator, Stratum,
    StratumConfig, StratumError,
};
use cdiscovery::{KademliaConfig, KademliaExtension, UnstructuredConfig, UnstructuredExtension};
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use cnetwork::{NetworkConfig, NetworkControl, NetworkService, SocketAddr};
use creactor::EventLoop;
use csync::{BlockSyncExtension, ParcelSyncExtension, SnapshotService};
use ctrlc::CtrlC;
use fdlimit::raise_fd_limit;
use parking_lot::{Condvar, Mutex};

use super::config::{self, load_config};
use super::constants::DEFAULT_KEYS_PATH;
use super::dummy_network_service::DummyNetworkService;
use super::json::PasswordFile;
use super::rpc::{rpc_http_start, rpc_ipc_start};
use super::rpc_apis::ApiDependencies;

fn network_start(cfg: &NetworkConfig) -> Result<Arc<NetworkService>, String> {
    info!("Handshake Listening on {}:{}", cfg.address, cfg.port);

    let addr = cfg.address.parse().map_err(|_| format!("Invalid NETWORK listen host given: {}", cfg.address))?;
    let sockaddress = SocketAddr::new(addr, cfg.port);
    let service = NetworkService::start(sockaddress, cfg.min_peers, cfg.max_peers)
        .map_err(|e| format!("Network service error: {:?}", e))?;

    Ok(service)
}

fn discovery_start(service: &NetworkService, cfg: &config::Network) -> Result<(), String> {
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

fn client_start(cfg: &config::Config, scheme: &Scheme, miner: Arc<Miner>) -> Result<ClientService, String> {
    info!("Starting client");
    let client_path = Path::new(&cfg.operating.db_path);
    let client_config = Default::default();
    let service = ClientService::start(client_config, &scheme, &client_path, miner)
        .map_err(|e| format!("Client service error: {}", e))?;

    Ok(service)
}

fn stratum_start(cfg: StratumConfig, miner: Arc<Miner>, client: Arc<Client>) -> Result<(), String> {
    match Stratum::start(&cfg, miner.clone(), client) {
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

fn new_miner(config: &config::Config, scheme: &Scheme, ap: Arc<AccountProvider>) -> Result<Arc<Miner>, String> {
    let miner = Miner::new(config.miner_options(), scheme, Some(ap.clone()));
    match miner.engine_type() {
        EngineType::PoW => match &config.mining.author {
            Some(ref author) => {
                miner.set_author(author.address.clone(), None).expect("set_author never fails when PoW is used")
            }
            None => return Err("mining.author is not specified".to_string()),
        },
        EngineType::InternalSealing => match &config.mining.engine_signer {
            Some(ref engine_signer) => {
                miner.set_author(engine_signer.address.clone(), None).map_err(|e| format!("{:?}", e))?
            }
            None => return Err("mining.engine_signer is not specified".to_string()),
        },
        EngineType::Solo => (),
    }

    Ok(miner)
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

fn prepare_account_provider(keys_path: &str) -> Result<Arc<AccountProvider>, String> {
    let keystore_dir = RootDiskDirectory::create(keys_path).map_err(|_| "Cannot read key path directory")?;
    let keystore = KeyStore::open(Box::new(keystore_dir)).map_err(|_| "Cannot open key store")?;
    Ok(AccountProvider::new(keystore))
}

fn load_password_file(path: Option<String>) -> Result<PasswordFile, String> {
    let pf = match path {
        Some(ref path) => {
            let file = fs::File::open(path).map_err(|e| format!("Could not read password file at {}: {}", path, e))?;
            PasswordFile::load(file).map_err(|e| format!("Invalid password file {}: {}", path, e))?
        }
        None => PasswordFile::default(),
    };
    Ok(pf)
}

fn unlock_accounts(ap: Arc<AccountProvider>, pf: &PasswordFile) -> Result<(), String> {
    for entry in pf.entries() {
        ap.unlock_account_permanently(entry.address.address.clone(), entry.password.clone())
            .map_err(|e| format!("Failed to unlock account {}: {}", entry.address.address, e))?;
    }
    Ok(())
}

pub fn run_node(matches: ArgMatches) -> Result<(), String> {
    // increase max number of open files
    raise_fd_limit();

    let _event_loop = EventLoop::spawn();
    let config = load_config(&matches)?;

    let scheme = config.operating.chain.scheme()?;

    let instance_id = config.operating.instance_id.unwrap_or(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Current time should be later than unix epoch")
            .subsec_nanos() as usize,
    );
    clogger::init(&LoggerConfig::new(instance_id)).expect("Logger must be successfully initialized");

    let pf = load_password_file(config.operating.password_path.clone())?;
    let keys_path = match config.operating.keys_path {
        Some(ref keys_path) => keys_path,
        None => DEFAULT_KEYS_PATH,
    };
    let ap = prepare_account_provider(keys_path)?;
    unlock_accounts(Arc::clone(&ap), &pf)?;

    let miner = new_miner(&config, &scheme, ap.clone())?;
    let client = client_start(&config, &scheme, miner.clone())?;

    let shard_validator = if scheme.params().use_shard_validator {
        None
    } else if config.shard_validator.disable {
        Some(ShardValidator::new(None, Arc::clone(&ap)))
    } else {
        Some(ShardValidator::new(Some(config.shard_validator_config().account), Arc::clone(&ap)))
    };

    let network_service: Arc<NetworkControl> = {
        if !config.network.disable {
            let network_config = config.network_config();
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
            if let Some(consensus_extension) = scheme.engine.network_extension() {
                service.register_extension(consensus_extension)?;
            }

            if let Some(shard_validator) = &shard_validator {
                service.register_extension(shard_validator.clone())?;
            }

            for address in network_config.bootstrap_addresses {
                service.connect_to(address)?;
            }
            service
        } else {
            Arc::new(DummyNetworkService::new())
        }
    };

    let rpc_apis_deps = Arc::new(ApiDependencies {
        client: client.client(),
        miner: Arc::clone(&miner),
        network_control: Arc::clone(&network_service),
        account_provider: ap,
        shard_validator,
    });

    let _rpc_server = {
        if !config.rpc.disable {
            Some(rpc_http_start(config.rpc_http_config(), config.rpc.enable_devel_api, Arc::clone(&rpc_apis_deps))?)
        } else {
            None
        }
    };

    let _ipc_server = {
        if !config.ipc.disable {
            Some(rpc_ipc_start(config.rpc_ipc_config(), config.rpc.enable_devel_api, Arc::clone(&rpc_apis_deps))?)
        } else {
            None
        }
    };

    if (!config.stratum.disable) && (miner.engine_type() == EngineType::PoW) {
        stratum_start(config.stratum_config(), Arc::clone(&miner), client.client())?
    }

    let _snapshot_service = {
        if !config.snapshot.disable {
            let service = SnapshotService::new(client.client(), config.snapshot.path, scheme.params().snapshot_period);
            client.client().add_notify(service.clone());
            Some(service)
        } else {
            None
        }
    };

    // drop the scheme to free up genesis state.
    drop(scheme);

    cinfo!(TEST_SCRIPT, "Initialization complete");

    wait_for_exit();

    Ok(())
}
