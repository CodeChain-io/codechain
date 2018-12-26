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

use ckey::{Error as KeyError, NetworkId, PlatformAddress};
use ctypes::transaction::Transaction as TransactionType;
use ctypes::ShardId;
use primitives::H256;

use super::{AssetMintOutput, AssetTransferInput, AssetTransferOutput, OrderOnTransfer};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum Transaction {
    #[serde(rename_all = "camelCase")]
    AssetMint {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,

        output: AssetMintOutput,
    },
    #[serde(rename_all = "camelCase")]
    AssetTransfer {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,
    },
    #[serde(rename_all = "camelCase")]
    AssetSchemeChange {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    AssetCompose {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        inputs: Vec<AssetTransferInput>,
        output: AssetMintOutput,
    },
    #[serde(rename_all = "camelCase")]
    AssetDecompose {
        network_id: NetworkId,
        input: AssetTransferInput,
        outputs: Vec<AssetTransferOutput>,
    },
    #[serde(rename_all = "camelCase")]
    AssetUnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
    },
}

// FIXME: Use TryFrom.
impl From<Transaction> for Result<TransactionType, KeyError> {
    fn from(from: Transaction) -> Self {
        Ok(match from {
            Transaction::AssetMint {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                output,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                TransactionType::AssetMint {
                    network_id,
                    shard_id,
                    metadata,
                    approver,
                    administrator,
                    output: output.into(),
                }
            }
            Transaction::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
            } => TransactionType::AssetTransfer {
                network_id,
                burns: burns.into_iter().map(From::from).collect(),
                inputs: inputs.into_iter().map(From::from).collect(),
                outputs: outputs.into_iter().map(From::from).collect(),
                orders: orders.into_iter().map(From::from).collect(),
            },
            Transaction::AssetSchemeChange {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                TransactionType::AssetSchemeChange {
                    network_id,
                    asset_type,
                    metadata,
                    approver,
                    administrator,
                }
            }
            Transaction::AssetCompose {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                inputs,
                output,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                TransactionType::AssetCompose {
                    network_id,
                    shard_id,
                    metadata,
                    approver,
                    administrator,
                    inputs: inputs.into_iter().map(|input| input.into()).collect(),
                    output: output.into(),
                }
            }
            Transaction::AssetDecompose {
                network_id,
                input,
                outputs,
            } => TransactionType::AssetDecompose {
                network_id,
                input: input.into(),
                outputs: outputs.into_iter().map(|output| output.into()).collect(),
            },
            Transaction::AssetUnwrapCCC {
                network_id,
                burn,
                ..
            } => TransactionType::AssetUnwrapCCC {
                network_id,
                burn: burn.into(),
            },
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum TransactionWithHash {
    #[serde(rename_all = "camelCase")]
    AssetMint {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,

        output: AssetMintOutput,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetTransfer {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetSchemeChange {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    AssetCompose {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
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
                administrator,
                output,
            } => TransactionWithHash::AssetMint {
                network_id,
                shard_id,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                administrator: administrator.map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                output: output.into(),
                hash,
            },
            TransactionType::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
            } => TransactionWithHash::AssetTransfer {
                network_id,
                burns: burns.into_iter().map(From::from).collect(),
                inputs: inputs.into_iter().map(From::from).collect(),
                outputs: outputs.into_iter().map(From::from).collect(),
                orders: orders.into_iter().map(From::from).collect(),
                hash,
            },
            TransactionType::AssetSchemeChange {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
            } => TransactionWithHash::AssetSchemeChange {
                network_id,
                asset_type,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                administrator: administrator.map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
            },
            TransactionType::AssetCompose {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                inputs,
                output,
            } => TransactionWithHash::AssetCompose {
                network_id,
                shard_id,
                metadata,
                approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                administrator: administrator.map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                inputs: inputs.into_iter().map(From::from).collect(),
                output: output.into(),
                hash,
            },
            TransactionType::AssetDecompose {
                network_id,
                input,
                outputs,
            } => TransactionWithHash::AssetDecompose {
                network_id,
                input: input.into(),
                outputs: outputs.into_iter().map(From::from).collect(),
                hash,
            },
            TransactionType::AssetUnwrapCCC {
                network_id,
                burn,
            } => TransactionWithHash::AssetUnwrapCCC {
                network_id,
                burn: burn.into(),
                hash,
            },
        }
    }
}
