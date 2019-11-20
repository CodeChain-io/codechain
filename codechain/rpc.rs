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

use crate::config::Config;
use crate::rpc_apis;
use crpc::{
    jsonrpc_core, start_http, start_ipc, start_ws, HttpServer, IpcServer, MetaIoHandler, Middleware, WsError, WsServer,
};
use futures::future::Either;
use serde_json;

#[derive(Debug, PartialEq)]
pub struct RpcHttpConfig {
    pub interface: String,
    pub port: u16,
    pub cors: Option<Vec<String>>,
    pub hosts: Option<Vec<String>>,
}

pub fn rpc_http_start(
    server: MetaIoHandler<(), impl Middleware<()>>,
    config: RpcHttpConfig,
) -> Result<HttpServer, String> {
    let url = format!("{}:{}", config.interface, config.port);
    let addr = url.parse().map_err(|_| format!("Invalid JSONRPC listen host/port given: {}", url))?;
    let start_result = start_http(&addr, config.cors.clone(), config.hosts.clone(), server);
    match start_result {
        Err(ref err) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!("RPC address {} is already in use, make sure that another instance of a CodeChain node is not running or change the address using the --jsonrpc-port option.", url))
        },
        Err(e) => Err(format!("RPC error: {:?}", e)),
        Ok(server) => {
            cinfo!(RPC, "RPC Listening on {}", url);
            if let Some(hosts) = config.hosts {
                cinfo!(RPC, "Allowed hosts are {:?}", hosts);
            }
            if let Some(cors) = config.cors {
                cinfo!(RPC, "CORS domains are {:?}", cors);
            }
            Ok(server)
        },
    }
}

#[derive(Debug, PartialEq)]
pub struct RpcIpcConfig {
    pub socket_addr: String,
}

pub fn rpc_ipc_start(
    server: MetaIoHandler<(), impl Middleware<()>>,
    config: RpcIpcConfig,
) -> Result<IpcServer, String> {
    let start_result = start_ipc(&config.socket_addr, server);
    match start_result {
        Err(ref err) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!("IPC address {} is already in use, make sure that another instance of a Codechain node is not running or change the address using the --ipc-path options.", config.socket_addr))
            },
        Err(e) => Err(format!("IPC error: {:?}", e)),
        Ok(server) =>  {
            cinfo!(RPC, "IPC Listening on {}", config.socket_addr);
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

pub fn rpc_ws_start(server: MetaIoHandler<(), impl Middleware<()>>, config: RpcWsConfig) -> Result<WsServer, String> {
    let url = format!("{}:{}", config.interface, config.port);
    let addr = url.parse().map_err(|_| format!("Invalid WebSockets listen host/port given: {}", url))?;
    let start_result = start_ws(&addr, server, config.max_connections);
    match start_result {
        Err(WsError::Io(ref err)) if err.kind() == io::ErrorKind::AddrInUse => {
            Err(format!("WebSockets address {} is already in use, make sure that another instance of a Codechain node is not running or change the address using the --ws-port options.", addr))
        },
        Err(e) => Err(format!("WebSockets error: {:?}", e)),
        Ok(server) => {
            cinfo!(RPC, "WebSockets Listening on {}", addr);
            Ok(server)
        },
    }
}

pub fn setup_rpc_server(config: &Config, deps: &rpc_apis::ApiDependencies) -> MetaIoHandler<(), impl Middleware<()>> {
    let mut handler = MetaIoHandler::with_middleware(LogMiddleware::new());
    deps.extend_api(config, &mut handler);
    rpc_apis::setup_rpc(handler)
}

struct LogMiddleware {}

impl<M: jsonrpc_core::Metadata> jsonrpc_core::Middleware<M> for LogMiddleware {
    type Future = jsonrpc_core::FutureResponse;
    type CallFuture = jsonrpc_core::FutureOutput;

    fn on_request<F, X>(&self, request: jsonrpc_core::Request, meta: M, next: F) -> Either<Self::Future, X>
    where
        F: FnOnce(jsonrpc_core::Request, M) -> X + Send,
        X: futures::Future<Item = Option<jsonrpc_core::Response>, Error = ()> + Send + 'static, {
        match &request {
            jsonrpc_core::Request::Single(call) => Self::print_call(call),
            jsonrpc_core::Request::Batch(calls) => {
                for call in calls {
                    Self::print_call(call);
                }
            }
        }
        Either::B(next(request, meta))
    }
}

impl LogMiddleware {
    fn new() -> Self {
        LogMiddleware {}
    }

    fn print_call(call: &jsonrpc_core::Call) {
        match call {
            jsonrpc_core::Call::MethodCall(method_call) => {
                cinfo!(
                    RPC,
                    "RPC call({}({}))",
                    method_call.method,
                    serde_json::to_string(&method_call.params).unwrap()
                );
            }
            jsonrpc_core::Call::Notification(_) => {}
            jsonrpc_core::Call::Invalid {
                ..
            } => {}
        }
    }
}
