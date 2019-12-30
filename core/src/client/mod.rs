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

mod chain_notify;
#[cfg_attr(feature = "cargo-clippy", allow(clippy::module_inception))]
mod client;
mod config;
mod importer;
mod test_client;

pub use self::chain_notify::ChainNotify;

pub use self::client::Client;
pub use self::config::ClientConfig;
pub use self::test_client::TestBlockChainClient;

use std::ops::Range;
use std::sync::Arc;

use cdb::DatabaseError;
use ckey::{Address, NetworkId, PlatformAddress, Public};
use cmerkle::Result as TrieResult;
use cnetwork::NodeId;
use cstate::{AssetScheme, FindActionHandler, OwnedAsset, StateResult, Text, TopLevelState, TopStateView};
use ctypes::transaction::{AssetTransferInput, PartialHashing, ShardTransaction};
use ctypes::{BlockHash, BlockNumber, CommonParams, ShardId, Tracker, TxHash};
use cvm::ChainTimeInfo;
use kvdb::KeyValueDB;
use primitives::{Bytes, H160, H256, U256};

use crate::block::{ClosedBlock, OpenBlock, SealedBlock};
use crate::blockchain_info::BlockChainInfo;
use crate::consensus::EngineError;
use crate::encoded;
use crate::error::{BlockImportError, Error as GenericError};
use crate::transaction::{LocalizedTransaction, PendingSignedTransactions, SignedTransaction};
use crate::types::{BlockId, BlockStatus, TransactionId, VerificationQueueInfo as BlockQueueInfo};

/// Provides various blockchain information, like block header, chain state etc.
pub trait BlockChainTrait {
    /// Get blockchain information.
    fn chain_info(&self) -> BlockChainInfo;
    /// Get genesis accounts
    fn genesis_accounts(&self) -> Vec<PlatformAddress>;

    /// Get raw block header data by block id.
    fn block_header(&self, id: &BlockId) -> Option<encoded::Header>;

    /// Get the best block header.
    fn best_block_header(&self) -> encoded::Header;

    /// Get the best header. Note that this is different from best block's header.
    fn best_header(&self) -> encoded::Header;

    /// Get the best proposal block header.
    fn best_proposal_header(&self) -> encoded::Header;

    /// Get raw block data by block header hash.
    fn block(&self, id: &BlockId) -> Option<encoded::Block>;

    /// Get the hash of block that contains the transaction, if any.
    fn transaction_block(&self, id: &TransactionId) -> Option<BlockHash>;

    fn transaction_header(&self, tracker: &Tracker) -> Option<encoded::Header>;

    fn transaction_block_number(&self, tracker: &Tracker) -> Option<BlockNumber> {
        self.transaction_header(tracker).map(|header| header.number())
    }

    fn transaction_block_timestamp(&self, tracker: &Tracker) -> Option<u64> {
        self.transaction_header(tracker).map(|header| header.timestamp())
    }
}

pub trait EngineInfo: Send + Sync {
    fn network_id(&self) -> NetworkId;
    fn common_params(&self, block_id: BlockId) -> Option<CommonParams>;
    fn metadata_seq(&self, block_id: BlockId) -> Option<u64>;
    fn block_reward(&self, block_number: u64) -> u64;
    fn mining_reward(&self, block_number: u64) -> Option<u64>;
    fn recommended_confirmation(&self) -> u32;
    fn possible_authors(&self, block_number: Option<u64>) -> Result<Option<Vec<PlatformAddress>>, EngineError>;
}

/// Client facilities used by internally sealing Engines.
pub trait EngineClient: Sync + Send + BlockChainTrait + ImportBlock {
    /// Make a new block and seal it.
    fn update_sealing(&self, parent_block: BlockId, allow_empty_block: bool);

    /// Submit a seal for a block in the mining queue.
    fn submit_seal(&self, block_hash: BlockHash, seal: Vec<Bytes>);

    /// Convert PoW difficulty to target.
    fn score_to_target(&self, score: &U256) -> U256;

    /// Update the best block as the given block hash
    ///
    /// Used in Tendermint, when going to the commit step.
    fn update_best_as_committed(&self, block_hash: BlockHash);

    fn get_kvdb(&self) -> Arc<dyn KeyValueDB>;
}

pub trait ConsensusClient: BlockChainClient + EngineClient + EngineInfo + TermInfo + StateInfo {}

pub trait TermInfo {
    fn last_term_finished_block_num(&self, id: BlockId) -> Option<BlockNumber>;
    fn current_term_id(&self, id: BlockId) -> Option<u64>;
    fn term_common_params(&self, id: BlockId) -> Option<CommonParams>;
}

/// Provides methods to access account info
pub trait AccountData {
    /// Attempt to get address seq at given block.
    /// May not fail on BlockId::Latest.
    fn seq(&self, address: &Address, id: BlockId) -> Option<u64>;

    /// Get address seq at the latest block's state.
    fn latest_seq(&self, address: &Address) -> u64 {
        self.seq(address, BlockId::Latest).expect(
            "seq will return Some when given BlockId::Latest. seq was given BlockId::Latest. \
             Therefore seq has returned Some; qed",
        )
    }

    /// Get address balance at the given block's state.
    ///
    /// May not return None if given BlockId::Latest.
    /// Returns None if and only if the block's root hash has been pruned from the DB.
    fn balance(&self, address: &Address, state: StateOrBlock) -> Option<u64>;

    /// Get address balance at the latest block's state.
    fn latest_balance(&self, address: &Address) -> u64 {
        self.balance(address, BlockId::Latest.into()).expect(
            "balance will return Some if given BlockId::Latest. balance was given BlockId::Latest \
             Therefore balance has returned Some; qed",
        )
    }

    fn regular_key(&self, address: &Address, state: StateOrBlock) -> Option<Public>;
    fn latest_regular_key(&self, address: &Address) -> Option<Public> {
        self.regular_key(address, BlockId::Latest.into())
    }

    fn regular_key_owner(&self, address: &Address, state: StateOrBlock) -> Option<Address>;
    fn latest_regular_key_owner(&self, address: &Address) -> Option<Address> {
        self.regular_key_owner(address, BlockId::Latest.into())
    }
}


/// State information to be used during client query
pub enum StateOrBlock {
    /// State to be used, may be pending
    State(Box<dyn TopStateView>),

    /// Id of an existing block from a chain to get state from
    Block(BlockId),
}

impl From<Box<dyn TopStateView>> for StateOrBlock {
    fn from(info: Box<dyn TopStateView>) -> StateOrBlock {
        StateOrBlock::State(info)
    }
}

impl From<BlockId> for StateOrBlock {
    fn from(id: BlockId) -> StateOrBlock {
        StateOrBlock::Block(id)
    }
}

pub trait Shard {
    fn number_of_shards(&self, state: StateOrBlock) -> Option<ShardId>;

    fn shard_id_by_hash(&self, create_shard_tx_hash: &TxHash, state: StateOrBlock) -> Option<ShardId>;
    fn shard_root(&self, shard_id: ShardId, state: StateOrBlock) -> Option<H256>;

    fn shard_owners(&self, shard_id: ShardId, state: StateOrBlock) -> Option<Vec<Address>>;
    fn shard_users(&self, shard_id: ShardId, state: StateOrBlock) -> Option<Vec<Address>>;
}

/// Provides methods to import block into blockchain
pub trait ImportBlock {
    /// Import a block into the blockchain.
    fn import_block(&self, bytes: Bytes) -> Result<BlockHash, BlockImportError>;

    /// Import a header into the blockchain
    fn import_header(&self, bytes: Bytes) -> Result<BlockHash, BlockImportError>;

    /// Import sealed block. Skips all verifications.
    fn import_sealed_block(&self, block: &SealedBlock) -> ImportResult;

    /// Set reseal min timer as reseal_min_period, for creating blocks with transactions which are pending because of reseal_min_period
    fn set_min_timer(&self);
    /// Set reseal max timer as reseal_max_period, for creating empty blocks every reseal_max_period
    fn set_max_timer(&self);
}

/// Blockchain database client. Owns and manages a blockchain and a block queue.
pub trait BlockChainClient: Sync + Send + AccountData + BlockChainTrait + ImportBlock + ChainTimeInfo {
    /// Get block queue information.
    fn queue_info(&self) -> BlockQueueInfo;

    /// Queue own transaction for importing
    fn queue_own_transaction(&self, transaction: SignedTransaction) -> Result<(), GenericError>;

    /// Queue transactions for importing.
    fn queue_transactions(&self, transactions: Vec<Bytes>, peer_id: NodeId);

    /// Delete all pending transactions.
    fn delete_all_pending_transactions(&self);

    /// List all transactions that are allowed into the next block.
    fn ready_transactions(&self, range: Range<u64>) -> PendingSignedTransactions;

    /// List all transactions in future block.
    fn future_ready_transactions(&self, range: Range<u64>) -> PendingSignedTransactions;

    /// Get the count of all pending transactions currently in the mem_pool.
    fn count_pending_transactions(&self, range: Range<u64>) -> usize;

    /// Get the count of all pending transactions included future transaction in the mem_pool.
    fn future_included_count_pending_transactions(&self, range: Range<u64>) -> usize;

    /// Check there are transactions which are allowed into the next block.
    fn is_pending_queue_empty(&self) -> bool;

    /// Look up the block number for the given block ID.
    fn block_number(&self, id: &BlockId) -> Option<BlockNumber>;

    /// Get raw block body data by block id.
    /// Block body is an RLP list of one item: transactions.
    fn block_body(&self, id: &BlockId) -> Option<encoded::Body>;

    /// Get block status by block header hash.
    fn block_status(&self, id: &BlockId) -> BlockStatus;


    /// Get block total score.
    fn block_total_score(&self, id: &BlockId) -> Option<U256>;

    /// Get block hash.
    fn block_hash(&self, id: &BlockId) -> Option<BlockHash>;

    /// Get transaction with given hash.
    fn transaction(&self, id: &TransactionId) -> Option<LocalizedTransaction>;

    /// Get invoice with given hash.
    fn error_hint(&self, hash: &TxHash) -> Option<String>;

    /// Get the transaction with given tracker.
    fn transaction_by_tracker(&self, tracker: &Tracker) -> Option<LocalizedTransaction>;

    fn error_hints_by_tracker(&self, tracker: &Tracker) -> Vec<(TxHash, Option<String>)>;
}

/// Result of import block operation.
pub type ImportResult = Result<BlockHash, DatabaseError>;

/// Provides methods used for sealing new state
pub trait BlockProducer {
    /// Reopens an OpenBlock and updates uncles.
    fn reopen_block(&self, block: ClosedBlock) -> OpenBlock;

    /// Returns OpenBlock prepared for closing.
    fn prepare_open_block(&self, parent_block: BlockId, author: Address, extra_data: Bytes) -> OpenBlock;
}

/// Extended client interface used for mining
pub trait MiningBlockChainClient: BlockChainClient + BlockProducer + FindActionHandler {
    /// Returns malicious users who sent failing transactions.
    fn get_malicious_users(&self) -> Vec<Address>;

    /// Release designated users from the malicious user list.
    fn release_malicious_users(&self, prisoner_vec: Vec<Address>);

    /// Append designated users to the malicious user list.
    fn imprison_malicious_users(&self, prisoner_vec: Vec<Address>);

    /// Returns users immune from getting banned.
    fn get_immune_users(&self) -> Vec<Address>;

    /// Append designated users to the immune user list.
    fn register_immune_users(&self, immune_user_vec: Vec<Address>);
}

/// Provides methods to access database.
pub trait DatabaseClient {
    fn database(&self) -> Arc<dyn KeyValueDB>;
}

/// Provides methods to access asset
pub trait AssetClient {
    fn get_asset_scheme(&self, asset_type: H160, shard_id: ShardId, id: BlockId) -> TrieResult<Option<AssetScheme>>;

    fn get_asset(
        &self,
        tracker: Tracker,
        index: usize,
        shard_id: ShardId,
        id: BlockId,
    ) -> TrieResult<Option<OwnedAsset>>;

    fn is_asset_spent(
        &self,
        tracker: Tracker,
        index: usize,
        shard_id: ShardId,
        block_id: BlockId,
    ) -> TrieResult<Option<bool>>;
}

/// Provides methods to texts
pub trait TextClient {
    fn get_text(&self, tx_hash: TxHash, id: BlockId) -> TrieResult<Option<Text>>;
}

pub trait ExecuteClient: ChainTimeInfo {
    fn execute_transaction(&self, transaction: &ShardTransaction, sender: &Address) -> StateResult<()>;

    fn execute_vm(
        &self,
        tx: &dyn PartialHashing,
        inputs: &[AssetTransferInput],
        params: &[Vec<Bytes>],
        indices: &[usize],
    ) -> Result<Vec<String>, DatabaseError>;
}

pub trait StateInfo {
    /// Attempt to get a copy of a specific block's final state.
    ///
    /// This will not fail if given BlockId::Latest.
    /// Otherwise, this can fail (but may not) if the DB prunes state or the block
    /// is unknown.
    fn state_at(&self, id: BlockId) -> Option<TopLevelState>;
}
