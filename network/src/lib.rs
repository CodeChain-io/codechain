// Copyright 2018-2019 Kodebox, Inc.
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
extern crate primitives;
extern crate rand;
extern crate rlp;
#[macro_use]
extern crate rlp_derive;

extern crate cidr;
extern crate codechain_crypto as ccrypto;
extern crate codechain_io as cio;
extern crate codechain_key as ckey;
extern crate codechain_timer as ctimer;
extern crate codechain_types as ctypes;
extern crate finally;
extern crate limited_table;
extern crate table as ctable;
extern crate time;
extern crate token_generator;

#[macro_use]
extern crate codechain_logger as clogger;
extern crate core;

mod addr;
mod client;
mod config;
mod discovery;
mod extension;
mod filters;
mod node_id;
mod routing_table;
mod service;
mod stream;
mod test;

pub mod control;
mod p2p;
pub mod session;

pub use crate::addr::SocketAddr;
pub use crate::config::Config as NetworkConfig;
pub use crate::control::{Control as NetworkControl, Error as NetworkControlError};
pub use crate::discovery::Api as DiscoveryApi;
pub use crate::extension::{
    Api, Error as NetworkExtensionError, Extension as NetworkExtension, Result as NetworkExtensionResult,
};
pub use crate::node_id::{IntoSocketAddr, NodeId};
pub use crate::service::{Error as NetworkServiceError, Service as NetworkService};
pub use crate::test::{Call as TestNetworkCall, TestClient as TestNetworkClient};

pub use crate::filters::{FilterEntry, Filters, FiltersControl};
pub use crate::routing_table::RoutingTable;
