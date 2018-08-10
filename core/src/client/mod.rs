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
mod test_client;

pub use self::chain_notify::ChainNotify;

pub use self::client::Client;
pub use self::config::ClientConfig;
pub use self::error::Error;
pub use self::test_client::TestBlockChainClient;

use std::sync::Arc;

use ckey::{Address, Public};
use cmerkle::Result as TrieResult;
use cnetwork::NodeId;
use cstate::{ActionHandler, AssetScheme, AssetSchemeAddress, OwnedAsset, TopStateInfo};
use ctypes::invoice::{ParcelInvoice, TransactionInvoice};
use ctypes::parcel::ChangeShard;
use ctypes::transaction::Transaction;
use ctypes::{BlockNumber, ShardId};
use kvdb::KeyValueDB;
use primitives::{Bytes, H256, U256};

use super::block::{ClosedBlock, OpenBlock, SealedBlock};
use super::blockchain::ParcelAddress;
use super::blockchain_info::BlockChainInfo;
use super::encoded;
use super::error::{BlockImportError, Error as CoreError};
use super::parcel::{LocalizedParcel, SignedParcel};
use super::spec::CommonParams;
use super::types::{BlockId, BlockStatus, ParcelId, TransactionId, VerificationQueueInfo as BlockQueueInfo};

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

/// Provides various information on a parcel by it's ID
pub trait ParcelInfo {
    /// Get the hash of block that contains the parcel, if any.
    fn parcel_block(&self, id: ParcelId) -> Option<H256>;
}

pub trait TransactionInfo {
    fn transaction_parcel(&self, id: TransactionId) -> Option<ParcelAddress>;

    fn is_any_transaction_included(&self, transactions: &mut Iterator<Item = H256>) -> bool {
        for hash in transactions {
            if self.transaction_parcel(TransactionId::Hash(hash)).is_some() {
                return true
            }
        }
        false
    }
}

pub trait EngineInfo: Send + Sync {
    fn common_params(&self) -> &CommonParams;
}

/// Client facilities used by internally sealing Engines.
pub trait EngineClient: Sync + Send + ChainInfo + ImportBlock {
    /// Make a new block and seal it.
    fn update_sealing(&self);

    /// Submit a seal for a block in the mining queue.
    fn submit_seal(&self, block_hash: H256, seal: Vec<Bytes>);

    /// Convert PoW difficulty to target.
    fn score_to_target(&self, score: &U256) -> U256;
}

/// Provides `nonce` and `latest_nonce` methods
pub trait Nonce {
    /// Attempt to get address nonce at given block.
    /// May not fail on BlockId::Latest.
    fn nonce(&self, address: &Address, id: BlockId) -> Option<U256>;

    /// Get address nonce at the latest block's state.
    fn latest_nonce(&self, address: &Address) -> U256 {
        self.nonce(address, BlockId::Latest).expect(
            "nonce will return Some when given BlockId::Latest. nonce was given BlockId::Latest. \
             Therefore nonce has returned Some; qed",
        )
    }
}

/// State information to be used during client query
pub enum StateOrBlock {
    /// State to be used, may be pending
    State(Box<TopStateInfo>),

    /// Id of an existing block from a chain to get state from
    Block(BlockId),
}

impl From<Box<TopStateInfo>> for StateOrBlock {
    fn from(info: Box<TopStateInfo>) -> StateOrBlock {
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
    fn balance(&self, address: &Address, state: StateOrBlock) -> Option<U256>;

    /// Get address balance at the latest block's state.
    fn latest_balance(&self, address: &Address) -> U256 {
        self.balance(address, BlockId::Latest.into()).expect(
            "balance will return Some if given BlockId::Latest. balance was given BlockId::Latest \
             Therefore balance has returned Some; qed",
        )
    }
}

pub trait RegularKey {
    fn regular_key(&self, address: &Address, state: StateOrBlock) -> Option<Public>;
}

pub trait Shard {
    fn number_of_shards(&self, state: StateOrBlock) -> Option<ShardId>;

    fn shard_root(&self, shard_id: ShardId, state: StateOrBlock) -> Option<H256>;
}

/// Provides methods to access account info
pub trait AccountData: Nonce + Balance {}

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
pub trait BlockChainClient: Sync + Send + AccountData + BlockChain + ImportBlock {
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
    fn parcel_invoice(&self, id: ParcelId) -> Option<ParcelInvoice>;

    /// Get the transaction with given hash.
    fn transaction(&self, id: TransactionId) -> Option<Transaction>;

    fn transaction_invoice(&self, id: TransactionId) -> Option<TransactionInvoice>;

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
    fn get_asset_scheme(&self, asset_type: AssetSchemeAddress) -> TrieResult<Option<AssetScheme>>;

    fn get_asset(&self, transaction_hash: H256, index: usize, id: BlockId) -> TrieResult<Option<OwnedAsset>>;

    fn is_asset_spent(
        &self,
        transaction_hash: H256,
        index: usize,
        shard_id: ShardId,
        block_id: BlockId,
    ) -> TrieResult<Option<bool>>;
}

pub trait ExecuteClient {
    fn execute_transactions(
        &self,
        transactions: &[Transaction],
        sender: &Address,
    ) -> Result<Vec<ChangeShard>, CoreError>;
}
