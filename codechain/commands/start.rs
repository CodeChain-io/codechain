use codechain_rpc::Server as RpcServer;
use rpc::HttpConfiguration as RpcHttpConfig;

use super::super::rpc;
use super::super::event_loop;
use super::super::event_loop::event_loop;

pub fn forever() -> Result<(), String> {
    let mut el = event_loop();

    info!("Run forever");
    el.run(event_loop::forever()).unwrap();
    Ok(())
}

pub fn rpc_start(cfg: RpcHttpConfig) -> RpcServer {
    info!("RPC Listening on {}", cfg.port);
    rpc::new_http(cfg).unwrap().unwrap()
}
