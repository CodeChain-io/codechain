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
mod test_client;

pub use self::chain_notify::ChainNotify;

pub use self::client::Client;
pub use self::config::ClientConfig;
pub use self::error::Error;
pub use self::test_client::TestBlockChainClient;

use std::sync::Arc;

use ckey::{Address, PlatformAddress, Public};
use cmerkle::Result as TrieResult;
use cnetwork::NodeId;
use cstate::{ActionHandler, AssetScheme, AssetSchemeAddress, OwnedAsset, TopStateView};
use ctypes::invoice::Invoice;
use ctypes::transaction::Transaction;
use ctypes::{BlockNumber, ShardId};
use cvm::ChainTimeInfo;
use kvdb::KeyValueDB;
use primitives::{Bytes, H256, U256};

use crate::block::{ClosedBlock, OpenBlock, SealedBlock};
use crate::blockchain_info::BlockChainInfo;
use crate::encoded;
use crate::error::{BlockImportError, Error as CoreError};
use crate::parcel::{LocalizedParcel, SignedParcel};
use crate::scheme::CommonParams;
use crate::types::{BlockId, BlockStatus, ParcelId, VerificationQueueInfo as BlockQueueInfo};

/// Provides `chain_info` method
pub trait ChainInfo {
    /// Get blockchain information.
    fn chain_info(&self) -> BlockChainInfo;
    /// Get genesis accounts
    fn genesis_accounts(&self) -> Vec<PlatformAddress>;
}

/// Provides various information on a block by it's ID
pub trait BlockInfo {
    /// Get raw block header data by block id.
    fn block_header(&self, id: BlockId) -> Option<encoded::Header>;

    /// Get the best block header.
    fn best_block_header(&self) -> encoded::Header;

    /// Get the best header. Note that this is different from best block's header.
    fn best_header(&self) -> encoded::Header;

    /// Get raw block data by block header hash.
    fn block(&self, id: BlockId) -> Option<encoded::Block>;
}

/// Provides various information on a parcel by it's ID
pub trait ParcelInfo {
    /// Get the hash of block that contains the parcel, if any.
    fn parcel_block(&self, id: ParcelId) -> Option<H256>;
}

pub trait TransactionInfo {
    fn transaction_header(&self, hash: &H256) -> Option<::encoded::Header>;

    fn transaction_block_number(&self, hash: &H256) -> Option<BlockNumber> {
        self.transaction_header(hash).map(|header| header.number())
    }

    fn transaction_block_timestamp(&self, hash: &H256) -> Option<u64> {
        self.transaction_header(hash).map(|header| header.timestamp())
    }
}

pub trait EngineInfo: Send + Sync {
    fn common_params(&self) -> &CommonParams;
    fn block_reward(&self, block_number: u64) -> u64;
    fn mining_reward(&self, block_number: u64) -> Option<u64>;
    fn recommended_confirmation(&self) -> u32;
}

/// Client facilities used by internally sealing Engines.
pub trait EngineClient: Sync + Send + ChainInfo + ImportBlock + BlockInfo {
    /// Make a new block and seal it.
    fn update_sealing(&self);

    /// Submit a seal for a block in the mining queue.
    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>);

    /// Convert PoW difficulty to target.
    fn score_to_target(&self, score: &U256) -> U256;
}

/// Provides `seq` and `latest_seq` methods
pub trait Seq {
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
}

/// State information to be used during client query
pub enum StateOrBlock {
    /// State to be used, may be pending
    State(Box<TopStateView>),

    /// Id of an existing block from a chain to get state from
    Block(BlockId),
}

impl From<Box<TopStateView>> for StateOrBlock {
    fn from(info: Box<TopStateView>) -> StateOrBlock {
        StateOrBlock::State(info)
    }
}

impl From<BlockId> for StateOrBlock {
    fn from(id: BlockId) -> StateOrBlock {
        StateOrBlock::Block(id)
    }
}

/// Provides `balance` and `latest_balance` methods
pub trait Balance {
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
}

pub trait RegularKey {
    fn regular_key(&self, address: &Address, state: StateOrBlock) -> Option<Public>;
    fn latest_regular_key(&self, address: &Address) -> Option<Public> {
        self.regular_key(address, BlockId::Latest.into())
    }
}

pub trait RegularKeyOwner {
    fn regular_key_owner(&self, address: &Address, state: StateOrBlock) -> Option<Address>;
    fn latest_regular_key_owner(&self, address: &Address) -> Option<Address> {
        self.regular_key_owner(address, BlockId::Latest.into())
    }
}

pub trait Shard {
    fn number_of_shards(&self, state: StateOrBlock) -> Option<ShardId>;

    fn shard_root(&self, shard_id: ShardId, state: StateOrBlock) -> Option<H256>;
}

/// Provides methods to access account info
pub trait AccountData: Seq + Balance {}

/// Provides methods to import block into blockchain
pub trait ImportBlock {
    /// Import a block into the blockchain.
    fn import_block(&self, bytes: Bytes) -> Result<H256, BlockImportError>;

    /// Import a header into the blockchain
    fn import_header(&self, bytes: Bytes) -> Result<H256, BlockImportError>;
}

/// Provides various blockchain information, like block header, chain state etc.
pub trait BlockChain: ChainInfo + BlockInfo + ParcelInfo + TransactionInfo {}

/// Blockchain database client. Owns and manages a blockchain and a block queue.
pub trait BlockChainClient:
    Sync + Send + AccountData + BlockChain + ImportBlock + RegularKeyOwner + ChainTimeInfo {
    /// Get block queue information.
    fn queue_info(&self) -> BlockQueueInfo;

    /// Queue parcels for importing.
    fn queue_parcels(&self, parcels: Vec<Bytes>, peer_id: NodeId);

    /// List all parcels that are allowed into the next block.
    fn ready_parcels(&self) -> Vec<SignedParcel>;

    /// Look up the block number for the given block ID.
    fn block_number(&self, id: BlockId) -> Option<BlockNumber>;

    /// Get raw block body data by block id.
    /// Block body is an RLP list of one item: parcels.
    fn block_body(&self, id: BlockId) -> Option<encoded::Body>;

    /// Get block status by block header hash.
    fn block_status(&self, id: BlockId) -> BlockStatus;

    /// Get block total score.
    fn block_total_score(&self, id: BlockId) -> Option<U256>;

    /// Get block hash.
    fn block_hash(&self, id: BlockId) -> Option<H256>;

    /// Get parcel with given hash.
    fn parcel(&self, id: ParcelId) -> Option<LocalizedParcel>;

    /// Get parcel invoice with given hash.
    fn parcel_invoice(&self, id: ParcelId) -> Option<Invoice>;

    /// Get the transaction with given hash.
    fn transaction(&self, hash: &H256) -> Option<Transaction>;

    fn transaction_invoices(&self, hash: &H256) -> Vec<Invoice>;

    fn custom_handlers(&self) -> Vec<Arc<ActionHandler>>;
}

/// Result of import block operation.
pub type ImportResult = Result<H256, Error>;

/// Provides `import_sealed_block` method
pub trait ImportSealedBlock {
    /// Import sealed block. Skips all verifications.
    fn import_sealed_block(&self, block: SealedBlock) -> ImportResult;
}

/// Provides `reopen_block` method
pub trait ReopenBlock {
    /// Reopens an OpenBlock and updates uncles.
    fn reopen_block(&self, block: ClosedBlock) -> OpenBlock;
}

/// Provides `prepare_open_block` method
pub trait PrepareOpenBlock {
    /// Returns OpenBlock prepared for closing.
    fn prepare_open_block(&self, author: Address, extra_data: Bytes) -> OpenBlock;
}

/// Provides methods used for sealing new state
pub trait BlockProducer: PrepareOpenBlock + ReopenBlock {}

/// Extended client interface used for mining
pub trait MiningBlockChainClient: BlockChainClient + BlockProducer + ImportSealedBlock {}

/// Provides methods to access database.
pub trait DatabaseClient {
    fn database(&self) -> Arc<KeyValueDB>;
}

/// Provides methods to access asset
pub trait AssetClient {
    fn get_asset_scheme(&self, asset_type: AssetSchemeAddress, id: BlockId) -> TrieResult<Option<AssetScheme>>;

    fn get_asset(&self, transaction_hash: H256, index: usize, id: BlockId) -> TrieResult<Option<OwnedAsset>>;

    fn is_asset_spent(
        &self,
        transaction_hash: H256,
        index: usize,
        shard_id: ShardId,
        block_id: BlockId,
    ) -> TrieResult<Option<bool>>;
}

pub trait ExecuteClient: ChainTimeInfo {
    fn execute_transaction(&self, transaction: &Transaction, sender: &Address) -> Result<Invoice, CoreError>;
}
