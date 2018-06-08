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

extern crate codechain_core as ccore;
extern crate codechain_types as ctypes;
extern crate jsonrpc_core;
extern crate jsonrpc_http_server;
extern crate kvdb;
extern crate kvdb_rocksdb as rocksdb;
extern crate log;
extern crate rlp;
extern crate rustc_hex;
extern crate rustc_serialize;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;

#[macro_use]
extern crate jsonrpc_macros;

pub mod rpc_server;
pub mod v1;

pub use rustc_serialize::hex;

pub use jsonrpc_core::{Compatibility, Error, MetaIoHandler, Params, Value};
pub use jsonrpc_http_server::tokio_core::reactor::Remote;

pub use jsonrpc_http_server::Server;
pub use rpc_server::start_http;
