extern crate codechain_bytes as cbytes;
extern crate codechain_core as ccore;
extern crate codechain_types as ctypes;
extern crate jsonrpc_core;
extern crate jsonrpc_http_server;
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
