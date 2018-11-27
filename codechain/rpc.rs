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
use std::sync::Arc;

use crate::rpc_apis;
use crpc::{start_http, start_ipc, start_ws, HttpServer, IpcServer, WsError, WsErrorKind, WsServer};
use crpc::{Compatibility, MetaIoHandler};

#[derive(Debug, PartialEq)]
pub struct RpcHttpConfig {
    pub interface: String,
    pub port: u16,
    pub cors: Option<Vec<String>>,
    pub hosts: Option<Vec<String>>,
}

pub fn rpc_http_start(
    cfg: RpcHttpConfig,
    enable_devel_api: bool,
    deps: Arc<rpc_apis::ApiDependencies>,
) -> Result<HttpServer, String> {
    let url = format!("{}:{}", cfg.interface, cfg.port);
    let addr = url.parse().map_err(|_| format!("Invalid JSONRPC listen host/port given: {}", url))?;
    let server = setup_http_rpc_server(&addr, cfg.cors, cfg.hosts, enable_devel_api, deps)?;
    cinfo!(RPC, "RPC Listening on {}", url);
    Ok(server)
}

fn setup_http_rpc_server(
    url: &SocketAddr,
    cors_domains: Option<Vec<String>>,
    allowed_hosts: Option<Vec<String>>,
    enable_devel_api: bool,
    deps: Arc<rpc_apis::ApiDependencies>,
) -> Result<HttpServer, String> {
    let server = setup_rpc_server(enable_devel_api, deps);
    let start_result = start_http(url, cors_domains, allowed_hosts, server);
    match start_result {
        Err(ref err) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!("RPC address {} is already in use, make sure that another instance of a CodeChain node is not running or change the address using the --jsonrpc-port option.", url))
        },
        Err(e) => Err(format!("RPC error: {:?}", e)),
        Ok(server) => Ok(server),
    }
}

#[derive(Debug, PartialEq)]
pub struct RpcIpcConfig {
    pub socket_addr: String,
}

pub fn rpc_ipc_start(
    cfg: RpcIpcConfig,
    enable_devel_api: bool,
    deps: Arc<rpc_apis::ApiDependencies>,
) -> Result<IpcServer, String> {
    let server = setup_rpc_server(enable_devel_api, deps);
    let start_result = start_ipc(&cfg.socket_addr, server);
    match start_result {
        Err(ref err) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!("IPC address {} is already in use, make sure that another instance of a Codechain node is not running or change the address using the --ipc-path options.", cfg.socket_addr))
            },
        Err(e) => Err(format!("IPC error: {:?}", e)),
        Ok(server) =>  {
            cinfo!(RPC, "IPC Listening on {}", cfg.socket_addr);
            Ok(server)
        },
    }
}

#[derive(Debug, PartialEq)]
pub struct RpcWsConfig {
    pub interface: String,
    pub port: u16,
    pub max_connections: usize,
}

pub fn rpc_ws_start(
    cfg: RpcWsConfig,
    enable_devel_api: bool,
    deps: Arc<rpc_apis::ApiDependencies>,
) -> Result<WsServer, String> {
    let server = setup_rpc_server(enable_devel_api, deps);
    let url = format!("{}:{}", cfg.interface, cfg.port);
    let addr = url.parse().map_err(|_| format!("Invalid WebSockets listen host/port given: {}", url))?;
    let start_result = start_ws(&addr, server, cfg.max_connections);
    match start_result {
        Err(WsError(WsErrorKind::Io(ref err), _)) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!("WebSockets address {} is already in use, make sure that another instance of a Codechain node is not running or change the address using the --ws-port options.", addr))
        },
        Err(e) => Err(format!("WebSockets error: {:?}", e)),
        Ok(server) => {
            cinfo!(RPC, "WebSockets Listening on {}", addr);
            Ok(server)
        },
    }
}

fn setup_rpc_server(enable_devel_api: bool, deps: Arc<rpc_apis::ApiDependencies>) -> MetaIoHandler<()> {
    let mut handler = MetaIoHandler::with_compatibility(Compatibility::Both);
    deps.extend_api(enable_devel_api, &mut handler);
    rpc_apis::setup_rpc(handler)
}
