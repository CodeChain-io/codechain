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

mod chain_notify;
mod client;
mod config;
mod error;
mod importer;

pub use self::chain_notify::ChainNotify;

pub use self::client::Client;
pub use self::config::ClientConfig;
pub use self::error::Error;

use cbytes::Bytes;
use ctypes::{Address, H256};

use super::blockchain_info::BlockChainInfo;
use super::encoded;
use super::error::BlockImportError;
use super::types::{BlockId, TransactionId};

/// Provides `chain_info` method
pub trait ChainInfo {
    /// Get blockchain information.
    fn chain_info(&self) -> BlockChainInfo;
}

/// Provides various information on a block by it's ID
pub trait BlockInfo {
    /// Get raw block header data by block id.
    fn block_header(&self, id: BlockId) -> Option<encoded::Header>;

    /// Get the best block header.
    fn best_block_header(&self) -> encoded::Header;

    /// Get raw block data by block header hash.
    fn block(&self, id: BlockId) -> Option<encoded::Block>;
}

/// Provides various information on a transaction by it's ID
pub trait TransactionInfo {
    /// Get the hash of block that contains the transaction, if any.
    fn transaction_block(&self, id: TransactionId) -> Option<H256>;
}

/// Client facilities used by internally sealing Engines.
pub trait EngineClient: Sync + Send  + ChainInfo {
    /// Broadcast a consensus message to the network.
    fn broadcast_consensus_message(&self, message: Bytes);

    /// Make a new block and seal it.
    fn update_sealing(&self);

    /// Submit a seal for a block in the mining queue.
    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>);
}

/// Provides methods to import block into blockchain
pub trait ImportBlock {
    /// Import a block into the blockchain.
    fn import_block(&self, bytes: Bytes) -> Result<H256, BlockImportError>;
}
