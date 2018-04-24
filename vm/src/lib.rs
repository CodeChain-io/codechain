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

extern crate codechain_bytes as cbytes;
extern crate codechain_crypto as ccrypto;
extern crate codechain_keys as ckeys;
extern crate codechain_types as ctypes;

#[cfg(test)]
extern crate secp256k1;

mod decoder;
mod executor;
mod opcode;

#[cfg(test)]
mod tests;

pub use decoder::{decode, DecoderError};
pub use executor::{execute, RuntimeError, ScriptResult};
