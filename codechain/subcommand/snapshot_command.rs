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


use std::path::Path;
use std::sync::Arc;

use ccore::{AccountProvider, BlockInfo, ClientService, DatabaseClient, EngineType, Miner, MinerService, Scheme};
use ckeystore::accounts_dir::RootDiskDirectory;
use ckeystore::KeyStore;
use clap::ArgMatches;
use csync::SnapshotService;

use super::super::config::{self, load_config};
use super::super::constants::DEFAULT_KEYS_PATH;

pub fn run_snapshot_command(matches: ArgMatches) -> Result<(), String> {
    if matches.subcommand.is_none() {
        println!("{}", matches.usage());
        return Ok(())
    }

    match matches.subcommand() {
        ("generate", _) => {
            let config = load_config(&matches)?;
            let scheme = config.operating.chain.scheme()?;
            let keys_path = match config.operating.keys_path {
                Some(ref keys_path) => keys_path,
                None => DEFAULT_KEYS_PATH,
            };
            let ap = prepare_account_provider(keys_path)?;
            let miner = new_miner(&config, &scheme, ap.clone())?;

            let client = client_start(&config, &scheme, miner.clone())?;

            let header = client.client().best_block_header();
            let db = client.client().database();
            let root_dir = config.snapshot.path;


            SnapshotService::write_snapshot(root_dir, header, db)
        }
        _ => Err("Invalid subcommand".to_string()),
    }
}

fn prepare_account_provider(keys_path: &str) -> Result<Arc<AccountProvider>, String> {
    let keystore_dir = RootDiskDirectory::create(keys_path).map_err(|_| "Cannot read key path directory")?;
    let keystore = KeyStore::open(Box::new(keystore_dir)).map_err(|_| "Cannot open key store")?;
    Ok(AccountProvider::new(keystore))
}

fn new_miner(config: &config::Config, scheme: &Scheme, ap: Arc<AccountProvider>) -> Result<Arc<Miner>, String> {
    let miner = Miner::new(config.miner_options()?, scheme, Some(ap.clone()));
    match miner.engine_type() {
        EngineType::PoW => match &config.mining.author {
            Some(ref author) => {
                miner.set_author((*author).into_address(), None).expect("set_author never fails when PoW is used")
            }
            None => return Err("mining.author is not specified".to_string()),
        },
        EngineType::InternalSealing => match &config.mining.engine_signer {
            Some(ref engine_signer) => {
                miner.set_author((*engine_signer).into_address(), None).map_err(|e| format!("{:?}", e))?
            }
            None => return Err("mining.engine_signer is not specified".to_string()),
        },
        EngineType::Solo => (),
    }

    Ok(miner)
}

fn client_start(cfg: &config::Config, scheme: &Scheme, miner: Arc<Miner>) -> Result<ClientService, String> {
    info!("Starting client");
    let client_path = Path::new(&cfg.operating.db_path);
    let client_config = Default::default();
    let service = ClientService::start(client_config, &scheme, &client_path, miner)
        .map_err(|e| format!("Client service error: {}", e))?;

    Ok(service)
}
