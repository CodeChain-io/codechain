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

#[macro_export]
macro_rules! log_target {
    (BLOCKCHAIN) => {
        "blockchain"
    };
    (CLIENT) => {
        "client"
    };
    (DISCOVERY) => {
        "discovery"
    };
    (ENGINE) => {
        "engine"
    };
    (EXTERNAL_PARCEL) => {
        "external_parcel"
    };
    (IO) => {
        "io"
    };
    (MEM_POOL) => {
        "mem_pool"
    };
    (MINER) => {
        "miner"
    };
    (NETWORK) => {
        "network"
    };
    (NETAPI) => {
        "netapi"
    };
    (NETFILTER) => {
        "netfilter"
    };
    (OWN_PARCEL) => {
        "own_parcel"
    };
    (ROUTING_TABLE) => {
        "routing_table"
    };
    (RPC) => {
        "rpc"
    };
    (SHUTDOWN) => {
        "shutdown"
    };
    (SNAPSHOT) => {
        "snapshot"
    };
    (SPEC) => {
        "spec"
    };
    (STATE) => {
        "state"
    };
    (STATE_DB) => {
        "state_db"
    };
    (STRATUM) => {
        "stratum"
    };
    (SYNC) => {
        "sync"
    };
    (SYNC_PARCEL) => {
        "sync_parcel"
    };
    (TEST_SCRIPT) => {
        "test_script"
    };
    (TRIE) => {
        "trie"
    };
    (TX) => {
        "tx"
    };
}

#[macro_export]
macro_rules! clog {
    ($target:ident, $lvl:expr, $($arg:tt)+) => ({
        log!(target: log_target!($target), $lvl, $($arg)*);
    });
}

#[macro_export]
macro_rules! cerror {
    ($target:ident, $($arg:tt)*) => (
        clog!($target, $crate::Level::Error, $($arg)*)
    );
}

#[macro_export]
macro_rules! cwarn {
    ($target:ident, $($arg:tt)*) => (
        clog!($target, $crate::Level::Warn, $($arg)*)
    );
}

#[macro_export]
macro_rules! cinfo {
    ($target:ident, $($arg:tt)*) => (
        clog!($target, $crate::Level::Info, $($arg)*)
    );
}

#[macro_export]
macro_rules! cdebug {
    ($target:ident, $($arg:tt)*) => (
        clog!($target, $crate::Level::Debug, $($arg)*)
    );
}

#[macro_export]
macro_rules! ctrace {
    ($target:ident, $($arg:tt)*) => (
        clog!($target, $crate::Level::Trace, $($arg)*)
    );
}
