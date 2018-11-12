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

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

extern crate app_dirs;
extern crate codechain_core as ccore;
extern crate codechain_discovery as cdiscovery;
extern crate codechain_finally as cfinally;
extern crate codechain_key as ckey;
extern crate codechain_keystore as ckeystore;
#[macro_use]
extern crate codechain_logger as clogger;
extern crate codechain_network as cnetwork;
extern crate codechain_reactor as creactor;
extern crate codechain_rpc as crpc;
extern crate codechain_state as cstate;
extern crate codechain_sync as csync;
extern crate codechain_timer as ctimer;
extern crate codechain_types as ctypes;
extern crate ctrlc;
extern crate env_logger;
extern crate fdlimit;
extern crate never;
extern crate panic_hook;
extern crate parking_lot;
extern crate primitives;
extern crate rpassword;
extern crate toml;

mod config;
mod constants;
mod dummy_network_service;
mod json;
mod rpc;
mod rpc_apis;
mod run_node;
mod subcommand;

use app_dirs::AppInfo;

use self::run_node::run_node;
use self::subcommand::run_subcommand;

pub const APP_INFO: AppInfo = AppInfo {
    name: "codechain",
    author: "Kodebox",
};

#[cfg(all(unix, target_arch = "x86_64"))]
fn main() -> Result<(), String> {
    panic_hook::set();

    // Always print backtrace on panic.
    ::std::env::set_var("RUST_BACKTRACE", "1");

    run()
}

fn run() -> Result<(), String> {
    let yaml = load_yaml!("codechain.yml");
    let matches = clap::App::from_yaml(yaml).get_matches();

    match matches.subcommand {
        Some(_) => run_subcommand(matches),
        None => run_node(matches),
    }
}
