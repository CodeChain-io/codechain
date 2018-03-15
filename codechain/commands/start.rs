use ccore::ClientService;
use cnetwork::Address;
use cnetwork::HandshakeService;
use codechain_rpc::Server as RpcServer;
use rpc::HttpConfiguration as RpcHttpConfig;

use super::super::config;
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

pub fn handshake_start(cfg: config::NetworkConfig) -> HandshakeService {
    info!("Handshake Listening on {}", cfg.port);
    let address = Address::v4(127, 0, 0, 1, cfg.port);
    HandshakeService::start(address, cfg.bootstrap_addresses).unwrap()
}

pub fn client_start() -> ClientService {
    info!("Starting client");
    ClientService::start().unwrap()
}

