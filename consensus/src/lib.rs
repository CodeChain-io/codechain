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

extern crate codechain_bytes as bytes;
extern crate codechain_crypto as crypto;
extern crate codechain_io as io;
extern crate codechain_keys as keys;
extern crate codechain_types;
extern crate heapsize;
extern crate rlp;
extern crate parking_lot;
extern crate time;
extern crate triehash;
extern crate util_error;

#[macro_use]
extern crate log;

mod block;
mod codechain_machine;
mod engine;
mod epoch;
mod error;
mod header;
mod machine;
mod signer;
mod solo;
mod tendermint;
mod transaction;
mod transition;
mod validator_set;
mod vote_collector;

type Bytes = Vec<u8>;

