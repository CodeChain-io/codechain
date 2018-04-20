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

use std::path::Path;
use std::sync::Arc;

use ccore::{ClientService, Miner, Spec};
use cnetwork::{NetworkConfig, NetworkService, SocketAddr};
use crpc::Server as RpcServer;
use rpc::HttpConfiguration as RpcHttpConfig;

use super::super::config;
use super::super::rpc;
use super::super::rpc_apis::ApiDependencies;

pub fn rpc_start(cfg: RpcHttpConfig, deps: Arc<ApiDependencies>) -> Result<RpcServer, String> {
    info!("RPC Listening on {}", cfg.port);
    rpc::new_http(cfg, deps)
}

pub fn network_start(cfg: &NetworkConfig) -> Result<NetworkService, String> {
    info!("Handshake Listening on {}", cfg.port);
    let secret_key = cfg.secret_key;
    let address = SocketAddr::v4(127, 0, 0, 1, cfg.port);
    let service = NetworkService::start(address, secret_key).map_err(|e| format!("Network service error: {:?}", e))?;

    Ok(service)
}

pub fn client_start(cfg: &config::Config, spec: &Spec, miner: Arc<Miner>) -> Result<ClientService, String> {
    info!("Starting client");
    let client_path = Path::new(&cfg.db_path);
    let client_config = Default::default();
    let service = ClientService::start(client_config, &spec, &client_path, miner)
        .map_err(|e| format!("Client service error: {:?}", e))?;

    Ok(service)
}
