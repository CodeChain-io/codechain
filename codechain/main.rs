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
extern crate codechain_rpc as crpc;
extern crate fdlimit;
extern crate env_logger;
extern crate panic_hook;

mod config;
mod commands;
mod event_loop;
mod rpc;
mod rpc_apis;

use app_dirs::AppInfo;
use clogger::{setup_log, Config as LogConfig};
use fdlimit::raise_fd_limit;

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

    let config = config::parse(&matches)?;
    let spec = config.chain_type.spec()?;

    let log_config = LogConfig::default();
    let _logger = setup_log(&log_config).expect("Logger is initialized only once; qed");

    let _rpc_server = config::parse_rpc_config(&matches)?.map(commands::rpc_start);

    let _handshake_server = config::parse_network_config(&matches)?.map(commands::handshake_start);

    let _client = commands::client_start(&spec);

    commands::forever()
}

