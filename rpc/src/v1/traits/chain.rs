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

use cjson::bytes::Bytes;
use cjson::uint::Uint;
use ckey::{NetworkId, PlatformAddress, Public};
use ctypes::invoice::Invoice;
use ctypes::{BlockNumber, ShardId};
use primitives::{Bytes as BytesArray, H256};

use jsonrpc_core::Result;

use super::super::types::{AssetScheme, Block, BlockNumberAndHash, OwnedAsset, Text, Transaction, UnsignedTransaction};

build_rpc_trait! {
    pub trait Chain {
        /// Sends signed transaction, returning its hash.
        # [rpc(name = "chain_sendSignedTransaction")]
        fn send_signed_transaction(&self, Bytes) -> Result<H256>;

        /// Gets transaction with given hash.
        # [rpc(name = "chain_getTransaction")]
        fn get_transaction(&self, H256) -> Result<Option<Transaction>>;

        /// Gets transaction invoice with given transaction hash.
        # [rpc(name = "chain_getInvoice")]
        fn get_invoice(&self, H256) -> Result<Option<Invoice>>;

        /// Gets transaction with given transaction tracker.
        # [rpc(name = "chain_getTransactionByTracker")]
        fn get_transaction_by_tracker(&self, H256) -> Result<Option<Transaction>>;

        /// Gets transaction invoices with given transaction tracker.
        # [rpc(name = "chain_getInvoicesByTracker")]
        fn get_invoices_by_tracker(&self, H256) -> Result<Vec<Invoice>>;

        /// Gets asset scheme with given transaction tracker.
        # [rpc(name = "chain_getAssetSchemeByTracker")]
        fn get_asset_scheme_by_tracker(&self, H256, ShardId, Option<u64>) -> Result<Option<AssetScheme>>;

        /// Gets asset scheme with given asset type.
        # [rpc(name = "chain_getAssetSchemeByType")]
        fn get_asset_scheme_by_type(&self, H256, Option<u64>) -> Result<Option<AssetScheme>>;

        /// Gets text with given transaction hash.
        # [rpc(name = "chain_getText")]
        fn get_text(&self, H256, Option<u64>) -> Result<Option<Text>>;

        /// Gets asset with given asset type.
        # [rpc(name = "chain_getAsset")]
        fn get_asset(&self, H256, usize, Option<u64>) -> Result<Option<OwnedAsset>>;

        /// Checks whether an asset is spent or not.
        # [rpc(name = "chain_isAssetSpent")]
        fn is_asset_spent(&self, H256, usize, ShardId, Option<u64>) -> Result<Option<bool>>;

        /// Gets seq with given account.
        # [rpc(name = "chain_getSeq")]
        fn get_seq(&self, PlatformAddress, Option<u64>) -> Result<Option<u64>>;

        /// Gets balance with given account.
        # [rpc(name = "chain_getBalance")]
        fn get_balance(&self, PlatformAddress, Option<u64>) -> Result<Option<Uint>>;

        /// Gets regular key with given account
        # [rpc(name = "chain_getRegularKey")]
        fn get_regular_key(&self, PlatformAddress, Option<u64>) -> Result<Option<Public>>;

        /// Gets the owner of given regular key.
        # [rpc(name = "chain_getRegularKeyOwner")]
        fn get_regular_key_owner(&self, Public, Option<u64>) -> Result<Option<PlatformAddress>>;

        /// Gets the genesis accounts
        # [rpc(name = "chain_getGenesisAccounts")]
        fn get_genesis_accounts(&self) -> Result<Vec<PlatformAddress>>;

        /// Gets the number of shards
        # [rpc(name = "chain_getNumberOfShards")]
        fn get_number_of_shards(&self, Option<u64>) -> Result<Option<ShardId>>;

        /// Gets shard root
        # [rpc(name = "chain_getShardRoot")]
        fn get_shard_root(&self, ShardId, Option<u64>) -> Result<Option<H256>>;

        /// Gets number of best block.
        # [rpc(name = "chain_getBestBlockNumber")]
        fn get_best_block_number(&self) -> Result<BlockNumber>;

        /// Gets the number and the hash of the best block.
        # [rpc(name = "chain_getBestBlockId")]
        fn get_best_block_id(&self) -> Result<BlockNumberAndHash>;

        /// Gets the hash of the block with given number.
        # [rpc(name = "chain_getBlockHash")]
        fn get_block_hash(&self, u64) -> Result<Option<H256>>;

        /// Gets block with given number.
        # [rpc(name = "chain_getBlockByNumber")]
        fn get_block_by_number(&self, u64) -> Result<Option<Block>>;

        /// Gets block with given hash.
        # [rpc(name = "chain_getBlockByHash")]
        fn get_block_by_hash(&self, H256) -> Result<Option<Block>>;

        /// Gets transactions in the current mem pool.
        # [rpc(name = "chain_getPendingTransactions")]
        fn get_pending_transactions(&self) -> Result<Vec<Transaction>>;

        /// Gets the mining given block number
        # [rpc(name = "chain_getMiningReward")]
        fn get_mining_reward(&self, u64) -> Result<Option<u64>>;

        /// Return the network id that is used in this chain.
        # [rpc(name = "chain_getNetworkId")]
        fn get_network_id(&self) -> Result<NetworkId>;

        /// Execute Transactions
        # [rpc(name = "chain_executeTransaction")]
        fn execute_transaction(&self, UnsignedTransaction, PlatformAddress) -> Result<Invoice>;

        /// Execute AssetTransfer transaction inputs in VM
        # [rpc(name = "chain_executeVM")]
        fn execute_vm(&self, UnsignedTransaction, Vec<Vec<BytesArray>>, Vec<usize>) -> Result<Vec<String>>;
    }
}
