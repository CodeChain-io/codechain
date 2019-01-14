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

use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ccore::{
    AccountProvider, AccountProviderError, ChainNotify, Client, ClientService, EngineType, Miner, MinerService, Scheme,
    Stratum, StratumConfig, StratumError,
};
use cdiscovery::{Config, Discovery};
use ckey::Address;
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use cnetwork::{Filters, NetworkConfig, NetworkControl, NetworkService, SocketAddr};
use creactor::EventLoop;
use csync::{BlockSyncExtension, ParcelSyncExtension, SnapshotService};
use ctimer::TimerLoop;
use ctrlc::CtrlC;
use fdlimit::raise_fd_limit;
use finally::finally;
use parking_lot::{Condvar, Mutex};

use crate::config::{self, load_config};
use crate::constants::DEFAULT_KEYS_PATH;
use crate::dummy_network_service::DummyNetworkService;
use crate::json::PasswordFile;
use crate::rpc::{rpc_http_start, rpc_ipc_start, rpc_ws_start};
use crate::rpc_apis::ApiDependencies;

fn network_start(timer_loop: TimerLoop, cfg: &NetworkConfig) -> Result<Arc<NetworkService>, String> {
    cinfo!(NETWORK, "Handshake Listening on {}:{}", cfg.address, cfg.port);

    let addr = cfg.address.parse().map_err(|_| format!("Invalid NETWORK listen host given: {}", cfg.address))?;
    let sockaddress = SocketAddr::new(addr, cfg.port);
    let filters = Filters::new(cfg.whitelist.clone(), cfg.blacklist.clone());
    let service = NetworkService::start(timer_loop, sockaddress, cfg.min_peers, cfg.max_peers, filters)
        .map_err(|e| format!("Network service error: {:?}", e))?;

    Ok(service)
}

fn discovery_start(service: &NetworkService, cfg: &config::Network) -> Result<(), String> {
    let config = Config {
        bucket_size: cfg.discovery_bucket_size.unwrap(),
        t_refresh: cfg.discovery_refresh.unwrap(),
    };
    let discovery = match cfg.discovery_type.as_ref().map(|s| s.as_str()) {
        Some("unstructured") => {
            cinfo!(DISCOVERY, "Node runs with unstructured discovery");
            Some(Discovery::unstructured(config))
        }
        Some("kademlia") => {
            cinfo!(DISCOVERY, "Node runs with kademlia discovery");
            Some(Discovery::kademlia(config))
        }
        Some(discovery_type) => return Err(format!("Unknown discovery {}", discovery_type)),
        None => None,
    };
    if let Some(discovery) = discovery {
        service.set_routing_table(&*discovery);
        service.register_extension(discovery);
    }
    Ok(())
}

fn client_start(
    timer_loop: &TimerLoop,
    cfg: &config::Operating,
    scheme: &Scheme,
    miner: Arc<Miner>,
) -> Result<ClientService, String> {
    cinfo!(CLIENT, "Starting client");
    let db_path = cfg.db_path.as_ref().map(|s| s.as_str()).unwrap();
    let client_path = Path::new(db_path);
    let client_config = Default::default();
    let service = ClientService::start(&client_config, &scheme, &client_path, miner)
        .map_err(|e| format!("Client service error: {}", e))?;

    let reseal_timer = timer_loop.new_timer("Client reseal timer", service.client());
    service.client().register_reseal_timer(reseal_timer);

    Ok(service)
}

fn stratum_start(cfg: &StratumConfig, miner: &Arc<Miner>, client: Arc<Client>) -> Result<(), String> {
    match Stratum::start(cfg, Arc::clone(&miner), client) {
        // FIXME: Add specified condition like AddrInUse
        Err(StratumError::Service(_)) =>
            Err(format!("STRATUM address {} is already in use, make sure that another instance of a CodeChain node is not running or change the address using the --stratum-port option.", cfg.port)),
        Err(e) => Err(format!("STRATUM start error: {:?}", e)),
        Ok(stratum) => {
            miner.add_work_listener(Box::new(stratum));
            cinfo!(STRATUM, "Listening on {}", cfg.port);
            Ok(())
        }
    }
}

fn new_miner(config: &config::Config, scheme: &Scheme, ap: Arc<AccountProvider>) -> Result<Arc<Miner>, String> {
    let miner = Miner::new(config.miner_options()?, scheme, Some(ap));

    if !config.mining.disable.unwrap() {
        match miner.engine_type() {
            EngineType::PoW => match &config.mining.author {
                Some(ref author) => {
                    miner.set_author((*author).into_address(), None).expect("set_author never fails when PoW is used")
                }
                None => return Err("The author is missing. Specify the author using --author option.".to_string()),
            },
            EngineType::PBFT | EngineType::PoA => match &config.mining.engine_signer {
                Some(ref engine_signer) => match miner.set_author((*engine_signer).into_address(), None) {
                    Err(AccountProviderError::NotUnlocked) => {
                        return Err(
                            "The account is not unlocked. Specify the password path using --password-path option."
                                .to_string(),
                        )
                    }
                    Err(e) => return Err(format!("{}", e)),
                    _ => (),
                },
                None => {
                    return Err("The engine signer is missing. Specify the engine signer using --engine-signer option."
                        .to_string())
                }
            },
            EngineType::Solo => miner
                .set_author(config.mining.author.map_or(Address::default(), |a| a.into_address()), None)
                .expect("set_author never fails when Solo is used"),
        }
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
    exit.1.wait(&mut l);
}

fn prepare_account_provider(keys_path: &str) -> Result<Arc<AccountProvider>, String> {
    let keystore_dir = RootDiskDirectory::create(keys_path).map_err(|_| "Cannot read key path directory")?;
    let keystore = KeyStore::open(Box::new(keystore_dir)).map_err(|_| "Cannot open key store")?;
    Ok(AccountProvider::new(keystore))
}

fn load_password_file(path: &Option<String>) -> Result<PasswordFile, String> {
    let pf = match path.as_ref() {
        Some(path) => {
            let file = fs::File::open(path).map_err(|e| format!("Could not read password file at {}: {}", path, e))?;
            PasswordFile::load(file).map_err(|e| format!("Invalid password file {}: {}", path, e))?
        }
        None => PasswordFile::default(),
    };
    Ok(pf)
}

fn unlock_accounts(ap: &AccountProvider, pf: &PasswordFile) -> Result<(), String> {
    for entry in pf.entries() {
        let entry_address = entry.address.into_address();
        ap.unlock_account_permanently(entry_address, entry.password.clone())
            .map_err(|e| format!("Failed to unlock account {}: {}", entry_address, e))?;
    }
    Ok(())
}

pub fn run_node(matches: &ArgMatches) -> Result<(), String> {
    // increase max number of open files
    raise_fd_limit();

    let _event_loop = EventLoop::spawn();
    let timer_loop = TimerLoop::new(2);

    let config = load_config(matches)?;

    // FIXME: It is the hotfix for #348.
    // Remove the below code if you find the proper way to solve #348.
    let _wait = finally(|| {
        const DEFAULT: u64 = 1;
        let wait_before_shutdown = env::var_os("WAIT_BEFORE_SHUTDOWN")
            .and_then(|sec| sec.into_string().ok())
            .and_then(|sec| sec.parse().ok())
            .unwrap_or(DEFAULT);
        ::std::thread::sleep(Duration::from_secs(wait_before_shutdown));
    });

    let scheme = match &config.operating.chain {
        Some(chain) => chain.scheme()?,
        None => return Err("chain is not specified".to_string()),
    };

    let instance_id = config.operating.instance_id.unwrap_or(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Current time should be later than unix epoch")
            .subsec_nanos() as usize,
    );
    clogger::init(&LoggerConfig::new(instance_id)).expect("Logger must be successfully initialized");

    let pf = load_password_file(&config.operating.password_path)?;
    let keys_path = match config.operating.keys_path {
        Some(ref keys_path) => keys_path,
        None => DEFAULT_KEYS_PATH,
    };
    let ap = prepare_account_provider(keys_path)?;
    unlock_accounts(&*ap, &pf)?;

    let miner = new_miner(&config, &scheme, ap.clone())?;
    let client = client_start(&timer_loop, &config.operating, &scheme, miner.clone())?;
    let sync = BlockSyncExtension::new(client.client());

    scheme.engine.register_chain_notify(client.client().as_ref());

    let network_service: Arc<NetworkControl> = {
        if !config.network.disable.unwrap() {
            let network_config = config.network_config()?;
            let service = network_start(timer_loop, &network_config)?;

            if config.network.discovery.unwrap() {
                discovery_start(&service, &config.network)?;
            } else {
                cwarn!(DISCOVERY, "Node runs without discovery extension");
            }

            if config.network.sync.unwrap() {
                service.register_extension(sync.clone());
                client.client().add_notify(Arc::downgrade(&sync) as Weak<ChainNotify>);
            }
            if config.network.transaction_relay.unwrap() {
                service.register_extension(ParcelSyncExtension::new(client.client()));
            }

            scheme.engine.register_network_extension_to_service(&service);

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
        block_sync: Arc::clone(&sync),
    });

    let _rpc_server = {
        if !config.rpc.disable.unwrap() {
            Some(rpc_http_start(config.rpc_http_config(), config.rpc.enable_devel_api, &*rpc_apis_deps)?)
        } else {
            None
        }
    };

    let _ipc_server = {
        if !config.ipc.disable.unwrap() {
            Some(rpc_ipc_start(&config.rpc_ipc_config(), config.rpc.enable_devel_api, &*rpc_apis_deps)?)
        } else {
            None
        }
    };

    let _ws_server = {
        if !config.ws.disable.unwrap() {
            Some(rpc_ws_start(&config.rpc_ws_config(), config.rpc.enable_devel_api, &*rpc_apis_deps)?)
        } else {
            None
        }
    };

    if (!config.stratum.disable.unwrap()) && (miner.engine_type() == EngineType::PoW) {
        stratum_start(&config.stratum_config(), &miner, client.client())?
    }

    let _snapshot_service = {
        if !config.snapshot.disable.unwrap() {
            let service =
                SnapshotService::new(client.client(), config.snapshot.path.unwrap(), scheme.params().snapshot_period);
            client.client().add_notify(Arc::downgrade(&service) as Weak<ChainNotify>);
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
