#[macro_use]
extern crate clap;

#[macro_use]
extern crate log;

extern crate app_dirs;
extern crate env_logger;

mod config;

use app_dirs::AppInfo;

pub const APP_INFO: AppInfo = AppInfo {
	name: "codechain",
	author: "Kodebox",
};

fn main() {
	// Always print backtrace on panic.
	::std::env::set_var("RUST_BACKTRACE", "1");

	env_logger::init();

	if let Err(err) = run() {
		println!("{}", err);
	}
}

fn run() -> Result<(), String> {
	let yaml = load_yaml!("codechain.yml");
	let matches = clap::App::from_yaml(yaml).get_matches();
	let cfg = try!(config::parse(&matches));
	info!("Listening on {}", cfg.port);
	Ok(())
}
