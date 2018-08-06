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

use ckey::{Address, Public};
use cstate::{Asset, AssetScheme};
use ctypes::invoice::{ParcelInvoice, TransactionInvoice};
use ctypes::parcel::ChangeShard;
use ctypes::transaction::Transaction;
use ctypes::{BlockNumber, ShardId, WorldId};
use primitives::{H160, H256, U256};

use jsonrpc_core::Result;

use super::super::types::{Block, BlockNumberAndHash, Bytes, Parcel};

build_rpc_trait! {
    pub trait Chain {
        /// Sends signed parcel, returning its hash.
        # [rpc(name = "chain_sendSignedParcel")]
        fn send_signed_parcel(&self, Bytes) -> Result<H256>;

        /// Gets parcel with given hash.
        # [rpc(name = "chain_getParcel")]
        fn get_parcel(&self, H256) -> Result<Option<Parcel>>;

        /// Gets parcel invoices with given hash.
        # [rpc(name = "chain_getParcelInvoice")]
        fn get_parcel_invoice(&self, H256) -> Result<Option<ParcelInvoice>>;

        /// Gets transaction with given hash.
        # [rpc(name = "chain_getTransaction")]
        fn get_transaction(&self, H256) -> Result<Option<Transaction>>;

        /// Gets transaction invoice with given hash.
        # [rpc(name = "chain_getTransactionInvoice")]
        fn get_transaction_invoice(&self, H256) -> Result<Option<TransactionInvoice>>;

        /// Gets asset scheme with given transaction hash.
        # [rpc(name = "chain_getAssetSchemeByHash")]
        fn get_asset_scheme_by_hash(&self, H256, ShardId, WorldId) -> Result<Option<AssetScheme>>;

        /// Gets asset scheme with given asset type.
        # [rpc(name = "chain_getAssetSchemeByType")]
        fn get_asset_scheme_by_type(&self, H256) -> Result<Option<AssetScheme>>;

        /// Gets asset with given asset type.
        # [rpc(name = "chain_getAsset")]
        fn get_asset(&self, H256, usize, Option<u64>) -> Result<Option<Asset>>;

        /// Checks whether an asset is spent or not.
        # [rpc(name = "chain_isAssetSpent")]
        fn is_asset_spent(&self, H256, usize, ShardId, Option<u64>) -> Result<Option<bool>>;

        /// Gets nonce with given account.
        # [rpc(name = "chain_getNonce")]
        fn get_nonce(&self, H160, Option<u64>) -> Result<Option<U256>>;

        /// Gets balance with given account.
        # [rpc(name = "chain_getBalance")]
        fn get_balance(&self, H160, Option<u64>) -> Result<Option<U256>>;

        /// Gets regular key with given account
        # [rpc(name = "chain_getRegularKey")]
        fn get_regular_key(&self, H160, Option<u64>) -> Result<Option<Public>>;

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

        /// Gets parcels in the current mem pool.
        # [rpc(name = "chain_getPendingParcels")]
        fn get_pending_parcels(&self) -> Result<Vec<Parcel>>;

        /// Gets coinbase's account id
        # [rpc(name = "chain_getCoinbase")]
        fn get_coinbase(&self) -> Result<Option<Address>>;

        /// Return the network id that is used in this chain.
        # [rpc(name = "chain_getNetworkId")]
        fn get_network_id(&self) -> Result<u32>;

        /// Execute Transactions
        # [rpc(name = "chain_executeTransactions")]
        fn execute_change_shard_state(&self, Bytes, Address) -> Result<Vec<ChangeShard>>;
    }
}
