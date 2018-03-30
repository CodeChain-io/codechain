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
use ctypes::H256;

use super::block::SealedBlock;
use super::blockchain_info::BlockChainInfo;
use super::encoded;
use super::error::BlockImportError;
use super::miner::TransactionImportResult;
use super::transaction::PendingTransaction;
use super::types::{BlockId, TransactionId, VerificationQueueInfo as BlockQueueInfo};

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

/// Provides various blockchain information, like block header, chain state etc.
pub trait BlockChain: ChainInfo + BlockInfo + TransactionInfo {}

/// Blockchain database client. Owns and manages a blockchain and a block queue.
pub trait BlockChainClient : Sync + Send + BlockChain + ImportBlock {
    /// Get block queue information.
    fn queue_info(&self) -> BlockQueueInfo;

    /// Queue transactions for importing.
    fn queue_transactions(&self, transactions: Vec<Bytes>, peer_id: usize);

    /// Queue conensus engine message.
    fn queue_consensus_message(&self, message: Bytes);

    /// List all transactions that are allowed into the next block.
    fn ready_transactions(&self) -> Vec<PendingTransaction>;
}

/// Provides `import_sealed_block` method
pub trait ImportSealedBlock {
    /// Import sealed block. Skips all verifications.
    fn import_sealed_block(&self, block: SealedBlock) -> TransactionImportResult;
}

/// Provides `broadcast_proposal_block` method
pub trait BroadcastProposalBlock {
    /// Broadcast a block proposal.
    fn broadcast_proposal_block(&self, block: SealedBlock);
}

/// Provides methods to import sealed block and broadcast a block proposal
pub trait SealedBlockImporter: ImportSealedBlock + BroadcastProposalBlock {}

/// Extended client interface used for mining
pub trait MiningBlockChainClient: BlockChainClient + SealedBlockImporter {}

