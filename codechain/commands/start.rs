use config;
use super::super::rpc;
use super::super::event_loop::{event_loop, forever};

pub fn start(cfg: config::Config) -> Result<(), String> {
    let mut el = event_loop();

    if cfg.rpc_config.enabled {
        info!("RPC Listening on {}", cfg.rpc_config.port);
        let _rpc_server = rpc::new_http(cfg.rpc_config);
    }
    el.run(forever()).unwrap();
    Ok(())
}
