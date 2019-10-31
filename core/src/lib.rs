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

#![cfg_attr(feature = "nightly", feature(test))]

extern crate codechain_crypto as ccrypto;
extern crate codechain_io as cio;
extern crate codechain_json as cjson;
extern crate codechain_key as ckey;
extern crate codechain_keystore as ckeystore;
#[macro_use]
extern crate codechain_logger as clogger;
extern crate codechain_merkle as cmerkle;
extern crate codechain_network as cnetwork;
extern crate codechain_state as cstate;
extern crate codechain_stratum as cstratum;
extern crate codechain_timer as ctimer;
extern crate codechain_types as ctypes;
extern crate codechain_vm as cvm;
extern crate crossbeam_channel;
extern crate cuckoo;
extern crate hashdb;
extern crate journaldb;
extern crate kvdb;
extern crate kvdb_memorydb;
extern crate kvdb_rocksdb;
extern crate linked_hash_map;
extern crate lru_cache;
extern crate memorydb;
extern crate num_rational;
extern crate primitives;
extern crate rand;
#[cfg(test)]
extern crate rand_xorshift;
extern crate rlp;
extern crate rlp_compress;
#[macro_use]
extern crate rlp_derive;
extern crate parking_lot;
extern crate rug;
extern crate snap;
extern crate statrs;
extern crate table;
extern crate util_error;
#[macro_use]
extern crate log;
extern crate core;
extern crate hyper;

mod account_provider;
pub mod block;
mod blockchain;
mod blockchain_info;
mod client;
mod codechain_machine;
mod consensus;
mod db;
mod db_version;
pub mod encoded;
mod error;
mod invoice;
mod miner;
mod scheme;
mod service;
mod transaction;
mod types;
mod verification;
mod views;

#[cfg(test)]
mod tests;

pub use crate::account_provider::{AccountProvider, Error as AccountProviderError};
pub use crate::block::Block;
pub use crate::client::{
    AccountData, AssetClient, BlockChainClient, BlockChainTrait, ChainNotify, Client, ClientConfig, DatabaseClient,
    EngineClient, EngineInfo, ExecuteClient, ImportBlock, MiningBlockChainClient, Shard, StateInfo, TermInfo,
    TestBlockChainClient, TextClient,
};
pub use crate::consensus::{EngineType, TimeGapParams};
pub use crate::db::{COL_STATE, NUM_COLUMNS};
pub use crate::error::{BlockImportError, Error, ImportError};
pub use crate::miner::{MemPoolFees, Miner, MinerOptions, MinerService, Stratum, StratumConfig, StratumError};
pub use crate::scheme::Scheme;
pub use crate::service::ClientService;
pub use crate::transaction::{
    LocalizedTransaction, PendingSignedTransactions, SignedTransaction, UnverifiedTransaction,
};
pub use crate::types::{BlockId, TransactionId};
