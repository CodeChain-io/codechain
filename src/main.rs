extern crate env_logger;
#[macro_use]
extern crate log;

fn main() {
    env_logger::init();

    debug!("this is a debug {}", "message");
    error!("this is printed by default");
}
