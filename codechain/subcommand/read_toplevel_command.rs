use ccore::ClientConfig;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
// use cstate::StateDB;
use ckey::Address;
use cstate::{StateDB, TopLevelState, TopState};
use ctypes::DebugRead;
use kvdb::KeyValueDB;
use kvdb_rocksdb::{Database, DatabaseConfig};
use parking_lot::Mutex;
use primitives::H256;
use rocksdb::Options;
use rustc_hex::{FromHex, ToHex};
use std::{path::Path, sync::Arc, time::Instant, u32};

pub const COL_EXTRA: Option<u32> = Some(3);
pub const COL_STATE: Option<u32> = Some(0);
pub const NUM_COLUMNS: Option<u32> = Some(6);

pub fn run_read_toplevel_command(matches: &ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(777), None).expect("Logger must be successfully initialized");

    let db_dir = matches.value_of("db-dir").unwrap();
    let key: u64 = matches.value_of("key").unwrap().to_string().parse().unwrap();
    let prepare = matches.is_present("prepare");

    let (db, opts) = open_db(db_dir)?;
    let journal_db = journaldb::new(Arc::clone(&db), journaldb::Algorithm::Archive, COL_STATE);
    let state_db = StateDB::new(journal_db);
    let root = {
        let bytes = db.get(COL_EXTRA, b"perf_data_root").unwrap().unwrap();
        H256::from(bytes.as_ref())
    };

    let mut toplevel_state = TopLevelState::from_existing(state_db.clone(&root), root).unwrap();

    if prepare {
        for key_ in (0..key) {
            let address = Address::random();
            let now = Instant::now();
            toplevel_state.add_balance_debug(&address, 7).unwrap();
            let elapsed = now.elapsed().as_micros();
            if (elapsed > 1000) {
                println!("elapsed {}us key: {}", elapsed, key_);
            }
        }
    }

    let address = Address::from(key);
    let now = Instant::now();
    let debug_info = toplevel_state.add_balance_debug(&address, 7).unwrap();
    let elapsed = now.elapsed().as_micros();

    println!("{}", opts.lock().print_statistics());
    println!("elapsed {}us debug info: {:?}", elapsed, debug_info);

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
