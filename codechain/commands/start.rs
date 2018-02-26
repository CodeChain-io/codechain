use config;
use super::super::rpc;

pub fn start(cfg: config::Config) -> Result<(), String> {
	let _rpc_server = rpc::new_http(cfg.rpc_config);
	Ok(())
}
