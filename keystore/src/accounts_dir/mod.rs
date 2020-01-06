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

//! Accounts Directory

use crate::{Error, SafeAccount};
use std::path::PathBuf;

mod disk;
mod memory;

/// `VaultKeyDirectory::set_key` error
#[derive(Debug)]
pub enum SetKeyError {
    /// Error is fatal and directory is probably in inconsistent state
    Fatal(Error),
    /// Error is non fatal, directory is reverted to pre-operation state
    NonFatalOld(Error),
    /// Error is non fatal, directory is consistent with new key
    NonFatalNew(Error),
}

/// Keys directory
pub trait KeyDirectory: Send + Sync {
    /// Read keys from directory
    fn load(&self) -> Result<Vec<SafeAccount>, Error>;
    /// Update key in the directory
    fn update(&self, account: SafeAccount) -> Result<SafeAccount, Error>;
    /// Insert new key to directory
    fn insert(&self, account: SafeAccount) -> Result<SafeAccount, Error>;
    /// Remove key from directory
    fn remove(&self, account: &SafeAccount) -> Result<(), Error>;
    /// Get directory filesystem path, if available
    fn path(&self) -> Option<&PathBuf> {
        None
    }
    /// Unique representation of directory account collection
    fn unique_repr(&self) -> Result<u64, Error>;
}

pub use self::disk::{DiskKeyFileManager, KeyFileManager, RootDiskDirectory};
pub use self::memory::MemoryDirectory;
