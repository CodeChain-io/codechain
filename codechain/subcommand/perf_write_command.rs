use ccore::ClientConfig;
use ckey::Address;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use cstate::{StateDB, StateWithCache, TopLevelState, TopState};
use kvdb::KeyValueDB;
use kvdb_rocksdb::{Database, DatabaseConfig};
use primitives::H256;
use std::{path::Path, sync::Arc, time::Instant};

pub const COL_STATE: Option<u32> = Some(0);
pub const COL_EXTRA: Option<u32> = Some(3);

pub fn run_perf_write_command(matches: &ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(777), None).expect("Logger must be successfully initialized");

    let num = matches.value_of("number").unwrap();
    let num: u32 = num.parse().unwrap();

    println!("start");
    let db = open_db()?;
    println!("open db");
    let journal_db = journaldb::new(Arc::clone(&db), journaldb::Algorithm::Archive, COL_STATE);
    println!("open journal db");
    let state_db = StateDB::new(journal_db);
    println!("open state db");

    println!("before getting root");

    let root = {
        let bytes = db.get(COL_EXTRA, b"perf_data_root").unwrap().unwrap();
        H256::from(bytes.as_ref())
    };

    let mut toplevel_state = TopLevelState::from_existing(state_db.clone(&root), root).unwrap();

    println!("before loop");
    for i in 0..10_u64.pow(num) {
        // println!("loop");
        let now = Instant::now();
        let address = Address::random();
        toplevel_state.add_balance(&address, i).unwrap();
        if now.elapsed().as_secs() >= 1 {
            println!("{}", now.elapsed().as_secs());
        }
    }
    toplevel_state.commit().unwrap();

    println!("Finished");

    Ok(())
}

pub const DEFAULT_DB_PATH: &str = "db_test";
pub const NUM_COLUMNS: Option<u32> = Some(6);


pub fn open_db() -> Result<Arc<dyn KeyValueDB>, String> {
    let base_path = ".".to_owned();
    let db_path = base_path + "/" + DEFAULT_DB_PATH;
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

    Ok(db)
}
