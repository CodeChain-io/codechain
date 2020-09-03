use ccore::ClientConfig;
use clogger::{self, LoggerConfig};
use clap::ArgMatches;
use kvdb::KeyValueDB;
use kvdb_rocksdb::{Database, DatabaseConfig};
use std::{path::Path, sync::Arc};

pub fn run_perf_write_command(matches: &ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(777), None).expect("Logger must be successfully initialized");
    let db = open_db()?;

    for i in 0..10 {
        let address = Address::random();
        toplevel_state.add_balance(&address, i).unwrap();
    }
    toplevel_state.commit().unwrap();


    unimplemented!();
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
