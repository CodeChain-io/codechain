extern crate app_dirs;
extern crate env_logger;

#[macro_use]
extern crate log;

use app_dirs::AppInfo;

pub const APP_INFO: AppInfo = AppInfo {
	name: "codechain",
	author: "Kodebox",
};

fn main() {
	env_logger::init();

	debug!("this is a debug {}", "message");
	error!("this is printed by default");
}
