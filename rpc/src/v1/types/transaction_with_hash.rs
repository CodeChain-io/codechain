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

use ckey::{NetworkId, PlatformAddress};
use ctypes::transaction::{AssetMintOutput, AssetTransferInput, AssetTransferOutput, Transaction as TransactionType};
use ctypes::ShardId;
use primitives::H256;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum TransactionWithHash {
    #[serde(rename_all = "camelCase")]
    AssetMint {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,

        output: AssetMintOutput,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetTransfer {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetCompose {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        inputs: Vec<AssetTransferInput>,
        output: AssetMintOutput,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetDecompose {
        network_id: NetworkId,
        input: AssetTransferInput,
        outputs: Vec<AssetTransferOutput>,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetUnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
        hash: H256,
    },
}

impl From<TransactionType> for TransactionWithHash {
    fn from(from: TransactionType) -> Self {
        let hash = from.hash();
        match from {
            TransactionType::AssetMint {
                network_id,
                shard_id,
                metadata,
                approver,
                output,
            } => TransactionWithHash::AssetMint {
                network_id,
                shard_id,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                output,
                hash,
            },
            TransactionType::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
            } => TransactionWithHash::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                hash,
            },
            TransactionType::AssetCompose {
                network_id,
                shard_id,
                metadata,
                approver,
                inputs,
                output,
            } => TransactionWithHash::AssetCompose {
                network_id,
                shard_id,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                inputs,
                output,
                hash,
            },
            TransactionType::AssetDecompose {
                network_id,
                input,
                outputs,
            } => TransactionWithHash::AssetDecompose {
                network_id,
                input,
                outputs,
                hash,
            },
            TransactionType::AssetUnwrapCCC {
                network_id,
                burn,
            } => TransactionWithHash::AssetUnwrapCCC {
                network_id,
                burn,
                hash,
            },
        }
    }
}
