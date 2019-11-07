// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Ethereum key-management.

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

extern crate libc;
extern crate parking_lot;
extern crate rand;
extern crate rustc_hex;
extern crate serde;
extern crate serde_json;
extern crate smallvec;
extern crate tempdir;
extern crate time;
extern crate vrf;

extern crate codechain_crypto as ccrypto;
extern crate codechain_json as cjson;
extern crate codechain_key as ckey;
extern crate codechain_types as ctypes;

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

#[cfg(test)]
#[macro_use]
extern crate matches;
#[cfg(test)]
extern crate primitives;

pub mod accounts_dir;
pub mod ckeys;

mod account;
mod json;

mod error;
mod import;
mod keystore;
mod random;
mod secret_store;

pub use crate::account::{Crypto, DecryptedAccount, SafeAccount};
pub use crate::error::Error;
pub use crate::import::{import_account, import_accounts};
pub use crate::json::OpaqueKeyFile as KeyFile;
pub use crate::keystore::{KeyMultiStore, KeyStore};
pub use crate::random::random_string;
pub use crate::secret_store::{SecretStore, SimpleSecretStore};
