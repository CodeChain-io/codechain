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

extern crate crypto as rcrypto;
#[macro_use]
extern crate log;
extern crate mio;
extern crate parking_lot;
extern crate rand;
extern crate rlp;
extern crate slab;

extern crate codechain_crypto as ccrypto;
extern crate codechain_io as cio;
extern crate codechain_types as ctypes;

mod client;
pub mod connection;
mod extension;
mod handshake;
mod limited_table;
mod service;
pub mod session;
pub mod address;
pub mod kademlia;

pub use self::address::Address;
pub use self::extension::{Api, Error, Extension, NodeId, Result};
pub use self::service::Service as NetworkService;
