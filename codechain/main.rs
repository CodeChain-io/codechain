#[macro_use]
extern crate clap;
extern crate futures;

#[macro_use]
extern crate log;
extern crate tokio_core;

extern crate app_dirs;
extern crate codechain_logger;
extern crate codechain_network;
extern crate codechain_rpc;
extern crate env_logger;
extern crate panic_hook;

mod config;
mod commands;
mod event_loop;
mod rpc;
mod rpc_apis;

use app_dirs::AppInfo;
use codechain_logger::setup_log;
use codechain_logger::Config as LogConfig;

pub const APP_INFO: AppInfo = AppInfo {
    name: "codechain",
    author: "Kodebox",
};

pub const LOG_INFO: &'static str = "sync=info";

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
    let cfg = try!(config::parse(&matches));

    let log_config = LogConfig::default();
    let _logger = setup_log(&log_config).expect("Logger is initialized only once; qed");

    info!("Listening on {}", cfg.port);
    match matches.subcommand() {
        _ => commands::start(cfg)
    }
}
