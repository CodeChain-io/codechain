use ccore::{
    AccountProvider, BlockChainTrait, BlockId, ClientConfig, ClientService, ImportBlock, Miner, Scheme, TimeGapParams,
};
use ckey::NetworkId;
use ckeystore::{accounts_dir::RootDiskDirectory, KeyStore};
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use cnetwork::{Filters, NetworkConfig, NetworkService, RoutingTable, SocketAddr};
use config::ChainType;
use ctimer::TimerLoop;
use ctypes::DebugRead;
use kvdb::KeyValueDB;
use kvdb_rocksdb::{Database, DatabaseConfig};
use parking_lot::Mutex;
use rocksdb::Options;
use std::{
    path::Path,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
    u32,
};

pub const NUM_COLUMNS: Option<u32> = Some(6);

pub fn run_execute_block_command(matches: &ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(777), None).expect("Logger must be successfully initialized");

    let db_dir = matches.value_of("db-dir").unwrap();
    let block_num: u64 = matches.value_of("block-num").unwrap().parse().unwrap();
    // let key = matches.value_of("key").unwrap().to_string().from_hex().unwrap();

    let timer_loop = TimerLoop::new(2);

    let time_gap_params = TimeGapParams {
        allowed_past_gap: Duration::from_millis(3000),
        allowed_future_gap: Duration::from_millis(3000),
    };
    let scheme = ChainType::Corgi.scheme()?;
    scheme.engine.register_time_gap_config_to_worker(time_gap_params);
    let client_config: ClientConfig = Default::default();
    let (db, opts) = open_db(db_dir)?;
    // let db = open_db(&config.operating, &client_config)?;

    let keys_path = "./keys".to_string();
    let ap = prepare_account_provider(&keys_path)?;
    let miner = new_miner(&scheme, ap.clone(), Arc::clone(&db))?;

    println!("start client");

    let client = client_start(&client_config, &timer_loop, db, &scheme, miner.clone())?;

    {
        let routing_table = RoutingTable::new();
        let network_config = NetworkConfig {
            address: "127.0.0.1".to_string(),
            port: 3485,
            bootstrap_addresses: Vec::new(),
            min_peers: 1,
            max_peers: 1,
            whitelist: Vec::new(),
            blacklist: Vec::new(),
        };
        println!("network start");
        let service =
            network_start(NetworkId::from_str("wc").unwrap(), timer_loop, &network_config, Arc::clone(&routing_table))?;
        println!("register network extension to service");
        scheme.engine.register_network_extension_to_service(&service);
    }

    println!("read block ");
    let block = client.client().block(&BlockId::Number(block_num)).expect("Read block from DB");

    println!("import block ");
    let now = Instant::now();
    let debug_read = DebugRead::empty();

    client.client().import_block_debug(block.into_inner());
    let elapsed = now.elapsed().as_micros();

    println!("{}", opts.lock().print_statistics());
    println!("elapsed {}us", elapsed);
    println!("{:?}", debug_read);

    Ok(())
}

pub fn open_db(db_dir: &str) -> Result<(Arc<dyn KeyValueDB>, Arc<Mutex<Options>>), String> {
    let base_path = ".".to_owned();
    let db_path = base_path + "/" + db_dir;
    let client_path = Path::new(&db_path);
    let mut db_config = DatabaseConfig::with_columns(NUM_COLUMNS);

    let client_config = ClientConfig::default();

    db_config.memory_budget = client_config.db_cache_size;
    db_config.compaction = client_config.db_compaction.compaction_profile(client_path);
    db_config.wal = client_config.db_wal;

    let db = Arc::new(
        Database::open(&db_config, &client_path.to_str().expect("DB path could not be converted to string."))
            .map_err(|_e| "Low level database error. Some issue with disk?".to_string())?,
    );

    let opts = db.opts.clone();
    Ok((db, opts))
}

fn new_miner(scheme: &Scheme, ap: Arc<AccountProvider>, db: Arc<dyn KeyValueDB>) -> Result<Arc<Miner>, String> {
    let miner = Miner::new(Default::default(), scheme, Some(ap), db);
    Ok(miner)
}

fn prepare_account_provider(keys_path: &str) -> Result<Arc<AccountProvider>, String> {
    let keystore_dir = RootDiskDirectory::create(keys_path).map_err(|_| "Cannot read key path directory")?;
    let keystore = KeyStore::open(Box::new(keystore_dir)).map_err(|_| "Cannot open key store")?;
    Ok(AccountProvider::new(keystore))
}

fn client_start(
    client_config: &ClientConfig,
    timer_loop: &TimerLoop,
    db: Arc<dyn KeyValueDB>,
    scheme: &Scheme,
    miner: Arc<Miner>,
) -> Result<ClientService, String> {
    cinfo!(CLIENT, "Starting client");
    let reseal_timer = timer_loop.new_timer_with_name("Client reseal timer");
    let service = ClientService::start(client_config, &scheme, db, miner, reseal_timer.clone())
        .map_err(|e| format!("Client service error: {}", e))?;
    reseal_timer.set_handler(Arc::downgrade(&service.client()));

    Ok(service)
}

fn network_start(
    network_id: NetworkId,
    timer_loop: TimerLoop,
    cfg: &NetworkConfig,
    routing_table: Arc<RoutingTable>,
) -> Result<Arc<NetworkService>, String> {
    let addr = cfg.address.parse().map_err(|_| format!("Invalid NETWORK listen host given: {}", cfg.address))?;
    let sockaddress = SocketAddr::new(addr, cfg.port);
    let filters = Filters::new(cfg.whitelist.clone(), cfg.blacklist.clone());
    let service = NetworkService::start(
        network_id,
        timer_loop,
        sockaddress,
        cfg.bootstrap_addresses.clone(),
        cfg.min_peers,
        cfg.max_peers,
        filters,
        routing_table,
    )
    .map_err(|e| format!("Network service error: {:?}", e))?;

    Ok(service)
}
