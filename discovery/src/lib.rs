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
extern crate parking_lot;
extern crate rand;
#[cfg_attr(test, macro_use)]
extern crate rlp;
extern crate time;

extern crate codechain_crypto as ccrypto;
extern crate codechain_key as ckey;
#[macro_use]
extern crate codechain_logger as clogger;
extern crate codechain_network as cnetwork;
extern crate codechain_types as ctypes;

mod kademlia;
mod unstructured;

pub use kademlia::{Config as KademliaConfig, Extension as KademliaExtension};
pub use unstructured::{Config as UnstructuredConfig, Extension as UnstructuredExtension};
