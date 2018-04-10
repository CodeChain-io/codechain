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

extern crate codechain_types as ctypes;
extern crate crypto as rcrypto;

pub mod aes;
mod hash;

pub use self::hash::{
    blake256, blake256_with_key, blake512, ripemd160, sha1, BLAKE_EMPTY, BLAKE_EMPTY_LIST_RLP, BLAKE_NULL_RLP,
};
