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

pub use std::string::ToString;

pub enum LogTarget {
    Sync,
}

impl ToString for LogTarget {
    fn to_string(&self) -> String {
        match self {
            LogTarget::Sync => String::from("sync"),
        }
    }
}

#[macro_export]
macro_rules! clog {
    ($target:expr, $lvl:expr, $($arg:tt)+) => ({
        log!(target: $crate::LogTarget::$target.to_string(), $lvl, $($arg)*);
    });
}

#[macro_export]
macro_rules! cerror {
    ($target:ident, $($arg:tt)*) => (
        error!(target: &$crate::LogTarget::$target.to_string(), $($arg)*);
    );
}

#[macro_export]
macro_rules! cwarn {
    ($target:ident, $($arg:tt)*) => (
        warn!(target: &$crate::LogTarget::$target.to_string(), $($arg)*);
    );
}

#[macro_export]
macro_rules! cinfo {
    ($target:ident, $($arg:tt)*) => (
        info!(target: &$crate::LogTarget::$target.to_string(), $($arg)*);
    );
}

#[macro_export]
macro_rules! cdebug {
    ($target:ident, $($arg:tt)*) => (
        debug!(target: &$crate::LogTarget::$target.to_string(), $($arg)*);
    );
}

#[macro_export]
macro_rules! ctrace {
    ($target:ident, $($arg:tt)*) => (
        trace!(target: &$crate::LogTarget::$target.to_string(), $($arg)*);
    );
}
