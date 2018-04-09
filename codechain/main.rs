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

#[macro_use]
extern crate clap;
extern crate futures;

#[macro_use]
extern crate log;
extern crate tokio_core;

extern crate app_dirs;
extern crate codechain_core as ccore;
extern crate codechain_logger as clogger;
extern crate codechain_network as cnetwork;
extern crate codechain_reactor as creactor;
extern crate codechain_rpc as crpc;
extern crate codechain_sync as csync;
extern crate codechain_types as ctypes;
extern crate ctrlc;
extern crate env_logger;
extern crate fdlimit;
extern crate panic_hook;
extern crate parking_lot;

mod commands;
mod config;
mod rpc;
mod rpc_apis;

use std::sync::Arc;

use app_dirs::AppInfo;
use clogger::{setup_log, Config as LogConfig};
use cnetwork::kademlia::Extension as KademliaExtension;
use creactor::EventLoop;
use csync::BlockSyncExtension;
use ctrlc::CtrlC;
use fdlimit::raise_fd_limit;
use parking_lot::{Condvar, Mutex};

pub const APP_INFO: AppInfo = AppInfo {
    name: "codechain",
    author: "Kodebox",
};

fn main() {
    panic_hook::set();

    // Always print backtrace on panic.
    ::std::env::set_var("RUST_BACKTRACE", "1");

    if let Err(err) = run() {
        println!("{}", err);
    }
}

fn run() -> Result<(), String> {
    let yaml = load_yaml!("codechain.yml");
    let matches = clap::App::from_yaml(yaml).get_matches();

    // increase max number of open files
    raise_fd_limit();

    let _event_loop = EventLoop::spawn();

    let config = config::parse(&matches)?;
    let spec = config.chain_type.spec()?;

    let log_config = LogConfig::default();
    let _logger = setup_log(&log_config).expect("Logger is initialized only once; qed");

    let _rpc_server = {
        if let Some(rpc_config) = config::parse_rpc_config(&matches)? {
            Some(commands::rpc_start(rpc_config)?)
        } else {
            None
        }
    };

    let client = commands::client_start(&config, &spec)?;

    let _network_service = {
        if let Some(network_config) = config::parse_network_config(&matches)? {
            let kademlia = {
                let kademlia_config = config::parse_kademlia_config(&matches)?;
                Arc::new(KademliaExtension::new(kademlia_config))
            };

            let service = commands::network_start(network_config, kademlia.clone())?;
            service.register_extension(kademlia);
            if config.enable_block_sync {
                service.register_extension(BlockSyncExtension::new(client.client()));
            }
            Some(service)
        } else {
            None
        }
    };

    // drop the spec to free up genesis state.
    drop(spec);

    wait_for_exit();

    Ok(())
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
    let _ = exit.1.wait(&mut l);
}
