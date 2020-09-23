use ccore::ClientConfig;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
// use cstate::StateDB;
use kvdb::KeyValueDB;
use kvdb_rocksdb::{Database, DatabaseConfig};
use rustc_hex::{FromHex, ToHex};
use std::{path::Path, sync::Arc, time::Instant, u32};
use rocksdb::Options;
use parking_lot::Mutex;
use ctypes::DebugRead;

// pub const COL_EXTRA: Option<u32> = Some(3);
pub const COL_STATE: Option<u32> = Some(0);
pub const NUM_COLUMNS: Option<u32> = Some(6);

pub fn run_read_db_command(matches: &ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(777), None).expect("Logger must be successfully initialized");

    let db_dir = matches.value_of("db-dir").unwrap();
    let key = matches.value_of("key").unwrap().to_string().from_hex().unwrap();

    let (db, opts) = open_db(db_dir)?;
    // let journal_db = journaldb::new(Arc::clone(&db), journaldb::Algorithm::Archive, COL_STATE);
    // let state_db = StateDB::new(journal_db);

    let now = Instant::now();
    let mut debug_read = DebugRead::empty();
    let value = db.get_debug(COL_STATE, &key, &mut debug_read).unwrap().unwrap();
    let elapsed = now.elapsed().as_micros();

    println!("{}", opts.lock().print_statistics());
    println!("elapsed {}us value: {}", elapsed, value.to_hex());
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
