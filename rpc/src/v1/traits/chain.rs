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

use ccore::{Asset, AssetScheme, BlockNumber, Invoice, ParcelInvoice};
use ctypes::{H160, H256, Public, U256};

use jsonrpc_core::Result;

use super::super::types::{Block, Bytes, Parcel};

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

        /// Gets transaction invoice with given hash.
        # [rpc(name = "chain_getTransactionInvoice")]
        fn get_transaction_invoice(&self, H256) -> Result<Option<Invoice>>;

        /// Gets asset scheme with given asset type.
        # [rpc(name = "chain_getAssetScheme")]
        fn get_asset_scheme(&self, H256) -> Result<Option<AssetScheme>>;

        /// Gets asset with given asset type.
        # [rpc(name = "chain_getAsset")]
        fn get_asset(&self, H256, usize) -> Result<Option<Asset>>;

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
        fn get_number_of_shards(&self, Option<u64>) -> Result<Option<u32>>;

        /// Gets shard root
        # [rpc(name = "chain_getShardRoot")]
        fn get_shard_root(&self, u32, Option<u64>) -> Result<Option<H256>>;

        /// Gets number of best block.
        # [rpc(name = "chain_getBestBlockNumber")]
        fn get_best_block_number(&self) -> Result<BlockNumber>;

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
    }
}
