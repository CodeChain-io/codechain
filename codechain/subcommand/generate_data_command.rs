use ccore::ClientConfig;
use ccrypto::BLAKE_NULL_RLP;
use ckey::Address;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use cstate::{StateDB, StateWithCache, TopLevelState, TopState};
use journaldb;
use kvdb::{DBTransaction, KeyValueDB};
use kvdb_rocksdb::{Database, DatabaseConfig};
use std::{path::Path, sync::Arc};

pub const COL_STATE: Option<u32> = Some(0);

/// Generate large trie to test update speed
pub fn run_generate_data_command(matches: &ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(777), None).expect("Logger must be successfully initialized");
    let db = open_db()?;

    let journal_db = journaldb::new(Arc::clone(&db), journaldb::Algorithm::Archive, COL_STATE);
    let state_db = StateDB::new(journal_db);
    let root = BLAKE_NULL_RLP;
    let mut toplevel_state = TopLevelState::from_existing(state_db.clone(&root), root).unwrap();

    let num = matches.value_of("number").unwrap();
    let num: u32 = num.parse().unwrap();

    //for i in 0..1_000_000_000 {
    for i in 0..10_u64.pow(num) {
        let address = Address::random();
        toplevel_state.add_balance(&address, i).unwrap();
    }
    toplevel_state.commit().unwrap();

    let mut batch = DBTransaction::new();
    let updated = toplevel_state.journal_under(&mut batch, 0).unwrap();
    //let updated = state_db.journal_under(&mut batch, 0, root).map_err(|err| err.to_string())?;
    db.write(batch).map_err(|err| err.to_string())?;

    println!("updated {}", updated);

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

