#[macro_use]
extern crate clap;
extern crate futures;

#[macro_use]
extern crate log;
extern crate tokio_core;

extern crate app_dirs;
extern crate env_logger;
extern crate logs;
extern crate panic_hook;
extern crate rpc as codechain_rpc;

mod config;
mod commands;
mod event_loop;
mod rpc;
mod rpc_apis;

use app_dirs::AppInfo;

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

	if !cfg.quiet {
		if cfg!(windows) {
			logs::init(LOG_INFO, logs::DateLogFormatter);
		} else {
			logs::init(LOG_INFO, logs::DateAndColorLogFormatter);
		}
	} else {
		env_logger::init();
	}

	info!("Listening on {}", cfg.port);
	match matches.subcommand() {
		_ => commands::start(cfg)
	}
}
