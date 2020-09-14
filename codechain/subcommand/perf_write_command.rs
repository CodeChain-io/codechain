use ccore::ClientConfig;
use ckey::Address;
use clap::ArgMatches;
use clogger::{self, LoggerConfig};
use cstate::{StateDB, StateWithCache, TopLevelState, TopState};
use ctypes::DebugInfo;
use kvdb::{DBTransaction, KeyValueDB};
use kvdb_rocksdb::{Database, DatabaseConfig};
use primitives::H256;
use std::{path::Path, sync::Arc, time::Instant, u128, u32, u64};

pub const COL_STATE: Option<u32> = Some(0);
pub const COL_EXTRA: Option<u32> = Some(3);

pub fn run_perf_write_command(matches: &ArgMatches) -> Result<(), String> {
    clogger::init(&LoggerConfig::new(777), None).expect("Logger must be successfully initialized");

    let num = matches.value_of("number").unwrap();
    let num: u32 = num.parse().unwrap();

    let db_dir = matches.value_of("db-dir").unwrap();

    println!("start");
    let db = open_db(db_dir)?;
    println!("open db");
    let journal_db = journaldb::new(Arc::clone(&db), journaldb::Algorithm::Archive, COL_STATE);
    println!("open journal db");
    let state_db = StateDB::new(journal_db);
    println!("open state db");

    println!("before getting root");

    let mut root = {
        let bytes = db.get(COL_EXTRA, b"perf_data_root").unwrap().unwrap();
        H256::from(bytes.as_ref())
    };

    let mut addresses = Vec::new();

    let mut toplevel_state = TopLevelState::from_existing(state_db.clone(&root), root).unwrap();

    println!("before loop");

    let mut total_elapsed_micros = 0_u128;
    /// (index, elapsed, read count)
    let mut max_elapsed = (0_u64, 0_u128, DebugInfo::empty());
    /// (index, elapsed, read count)
    let mut max_height = (0_u64, 0_u128, DebugInfo::empty());
    /// (index, elapsed, read count)
    let mut min_elapsed = (u64::MAX, u128::MAX, DebugInfo::empty());
    for i in 0..10_u64.pow(num) {
        // let address = Address::random();
        let address = Address::from(i as u64);
        addresses.push(address);
        let now = Instant::now();
        let debug_info = toplevel_state.add_balance_debug(&address, i + 1).unwrap();
        let elapsed = now.elapsed().as_micros();
        total_elapsed_micros += elapsed;
        let (_, max_elapsed_micros, _) = max_elapsed;
        if elapsed > max_elapsed_micros {
            max_elapsed = (i, elapsed, debug_info);
        }
        let (_, min_elapsed_micros, _) = min_elapsed;
        if elapsed < min_elapsed_micros {
            min_elapsed = (i, elapsed, debug_info);
        }
        let (_, _, max_height_) = max_height;
        if debug_info.read_count > max_height_.read_count {
            max_height = (i, elapsed, debug_info);
        }
        println!("debug info {:?}", debug_info);
    }
    println!("Average {}us from DB", total_elapsed_micros / 10_u128.pow(num));
    println!("Max {}us from DB address: {} debug_info {:?}", max_elapsed.1, max_elapsed.0, max_elapsed.2);
    println!("Min {}us from DB address: {} debug_info {:?}", min_elapsed.1, min_elapsed.0, min_elapsed.2);
    println!("Max height {}us from DB address: {} debug_info {:?}", max_height.1, max_height.0, max_height.2);

    root = toplevel_state.commit().unwrap();
    // {
    //     let mut batch = DBTransaction::new();
    //     let updated = toplevel_state.journal_under(&mut batch, 0).unwrap();
    //     println!("write to db {}", updated);
    //     db.write(batch).map_err(|err| err.to_string())?;
    //     println!("flush the db");
    //     db.flush().unwrap();
    // }

    // toplevel_state = TopLevelState::from_existing(state_db.clone(&root), root).unwrap();

    // let mut total_elapsed_micros = 0_u128;
    // let mut max_elapsed_micros = 0_u128;
    // for i in 0..10_u64.pow(num) {
    //     // println!("loop");
    //     let now = Instant::now();
    //     let address = addresses[i as usize];
    //     toplevel_state.add_balance(&address, i).unwrap();
    //     let elapsed = now.elapsed().as_micros();
    //     if elapsed >= 1 * 1000 * 1000 {
    //         println!("{}", elapsed);
    //     }
    //     total_elapsed_micros += now.elapsed().as_micros();
    //     if elapsed > max_elapsed_micros {
    //         max_elapsed_micros = elapsed;
    //     }
    // }
    // println!("Average {}us from cache", total_elapsed_micros / 10_u128.pow(num));
    // println!("Max {}us from cache", max_elapsed_micros);

    // toplevel_state.commit().unwrap();

    println!("Finished");

    Ok(())
}

// pub const DEFAULT_DB_PATH: &str = "db_test";
pub const NUM_COLUMNS: Option<u32> = Some(6);

pub fn open_db(db_dir: &str) -> Result<Arc<dyn KeyValueDB>, String> {
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

    Ok(db)
}
