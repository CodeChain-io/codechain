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

use std::io;
use std::net::SocketAddr;

use crpc::{Server, start_http, MetaIoHandler, Compatibility};
use rpc_apis::{self};

#[derive(Debug, PartialEq)]
pub struct HttpConfiguration {
    pub interface: String,
    pub port: u16,
    pub cors: Option<Vec<String>>,
    pub hosts: Option<Vec<String>>,
}

impl HttpConfiguration {
    pub fn with_port(port: u16) -> Self {
        HttpConfiguration {
            interface: "127.0.0.1".into(),
            port,
            cors: None,
            hosts: Some(Vec::new()),
        }
    }
}

pub fn new_http(cfg: HttpConfiguration) -> Result<Server, String> {
    let url = format!("{}:{}", cfg.interface, cfg.port);
    let addr = try!(url.parse().map_err(|_| format!("Invalid JSONRPC listen host/port given: {}", url)));
    let server = setup_http_rpc_server(&addr, cfg.cors, cfg.hosts)?;
    Ok(server)
}

pub fn setup_http_rpc_server(
    url: &SocketAddr,
    cors_domains: Option<Vec<String>>,
    allowed_hosts: Option<Vec<String>>,
    ) -> Result<Server, String> {
    let server = setup_rpc_server();
    let start_result = start_http(url, cors_domains, allowed_hosts, server);
    match start_result {
        Err(ref err) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!("RPC address {} is already in use, make sure that another instance of a Bitcoin node is not running or change the address using the --jsonrpc-port and --jsonrpc-interface options.", url))
        },
        Err(e) => Err(format!("RPC error: {:?}", e)),
        Ok(server) => Ok(server),
    }
}

fn setup_rpc_server() -> MetaIoHandler<()> {
    rpc_apis::setup_rpc(MetaIoHandler::with_compatibility(Compatibility::Both))
}

