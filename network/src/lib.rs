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

#![allow(deprecated)]

#[macro_use]
extern crate log;
extern crate mio;
extern crate parking_lot;
extern crate rand;
extern crate rlp;
extern crate slab;

extern crate codechain_crypto as ccrypto;
extern crate codechain_finally as cfinally;
extern crate codechain_io as cio;
extern crate codechain_keys as ckeys;
extern crate codechain_types as ctypes;
extern crate table as ctable;

mod addr;
mod client;
mod config;
mod discovery;
mod extension;
mod handshake;
mod limited_table;
mod service;
mod test;
mod timer_info;
mod token_generator;

pub mod connection;
pub mod session;

pub use self::addr::SocketAddr;
pub use self::config::Config as NetworkConfig;
pub use self::discovery::Api as DiscoveryApi;
pub use self::extension::{Api, Error, Extension as NetworkExtension, NodeToken, Result, TimerToken};
pub use self::service::Service as NetworkService;
pub use self::test::TestClient as TestNetworkClient;
