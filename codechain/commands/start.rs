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

use ccore::{ClientService, Spec};
use cnetwork::{Address, HandshakeService};
use crpc::Server as RpcServer;
use rpc::HttpConfiguration as RpcHttpConfig;

use super::super::config;
use super::super::rpc;

pub fn rpc_start(cfg: RpcHttpConfig) -> RpcServer {
    info!("RPC Listening on {}", cfg.port);
    rpc::new_http(cfg).unwrap().unwrap()
}

pub fn handshake_start(cfg: config::NetworkConfig) -> HandshakeService {
    info!("Handshake Listening on {}", cfg.port);
    let address = Address::v4(127, 0, 0, 1, cfg.port);
    HandshakeService::start(address, cfg.bootstrap_addresses).unwrap()
}

pub fn client_start(spec: &Spec) -> ClientService {
    info!("Starting client");
    // FIXME: Don't hardcode client_path.
    let client_path = Path::new("./db");
    let client_config = Default::default();
    ClientService::start(client_config, &spec, &client_path).unwrap()
}

