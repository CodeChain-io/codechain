use ccore::ClientConfig;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use kvdb::KeyValueDB;
use kvdb_rocksdb::{Database, DatabaseConfig};
use std::{path::Path, sync::Arc, time::Instant};
use ckey::Address;
use cstate::{TopState, TopLevelState, StateDB, StateWithCache};
use primitives::H256;

pub const COL_STATE: Option<u32> = Some(0);
pub const COL_EXTRA: Option<u32> = Some(3);

pub fn run_perf_write_command(_matches: &ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(777), None).expect("Logger must be successfully initialized");
    let db = open_db()?;
    let journal_db = journaldb::new(Arc::clone(&db), journaldb::Algorithm::Archive, COL_STATE);
    let state_db = StateDB::new(journal_db);

    let root = {
        let bytes = db.get(COL_EXTRA, b"perf_data_root").unwrap().unwrap();
        H256::from(bytes.as_ref())
    };

    let mut toplevel_state = TopLevelState::from_existing(state_db.clone(&root), root).unwrap();

    for i in 0..10 {
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
