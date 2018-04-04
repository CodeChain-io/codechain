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

use ccore::{ClientService, Spec};
use cnetwork::{Address, DiscoveryApi, NetworkConfig, NetworkService};
use crpc::Server as RpcServer;
use rpc::HttpConfiguration as RpcHttpConfig;

use super::super::config;
use super::super::rpc;

pub fn rpc_start(cfg: RpcHttpConfig) -> Result<RpcServer, String> {
    info!("RPC Listening on {}", cfg.port);
    rpc::new_http(cfg)
}

pub fn network_start(cfg: NetworkConfig, discovery: Arc<DiscoveryApi>) -> Result<NetworkService, String> {
    info!("Handshake Listening on {}", cfg.port);
    let secret_key = cfg.secret_key;
    let address = Address::v4(127, 0, 0, 1, cfg.port);
    let service = NetworkService::start(
        address,
        cfg.bootstrap_addresses,
        secret_key,
        discovery,
    ).map_err(|e| format!("Network service error: {:?}", e))?;

    Ok(service)
}

pub fn client_start(cfg: &config::Config, spec: &Spec) -> Result<ClientService, String> {
    info!("Starting client");
    let client_path = Path::new(&cfg.db_path);
    let client_config = Default::default();
    let service = ClientService::start(
        client_config,
        &spec,
        &client_path
    ).map_err(|e| format!("Client service error: {:?}", e))?;

    Ok(service)
}

