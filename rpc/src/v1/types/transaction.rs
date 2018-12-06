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

use cjson::uint::Uint;
use ckey::{Error as KeyError, NetworkId, PlatformAddress};
use ctypes::transaction::{
    AssetMintOutput as AssetMintOutputType, AssetOutPoint as AssetOutPointType,
    AssetTransferInput as AssetTransferInputType, AssetTransferOutput as AssetTransferOutputType, Order as OrderType,
    OrderOnTransfer as OrderOnTransferType, Timelock, Transaction as TransactionType,
};
use ctypes::ShardId;
use primitives::{Bytes, H160, H256};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetOutPoint {
    pub transaction_hash: H256,
    pub index: usize,
    pub asset_type: H256,
    pub amount: Uint,
}

impl From<AssetOutPointType> for AssetOutPoint {
    fn from(from: AssetOutPointType) -> Self {
        AssetOutPoint {
            transaction_hash: from.transaction_hash,
            index: from.index,
            asset_type: from.asset_type,
            amount: from.amount.into(),
        }
    }
}

impl From<AssetOutPoint> for AssetOutPointType {
    fn from(from: AssetOutPoint) -> Self {
        AssetOutPointType {
            transaction_hash: from.transaction_hash,
            index: from.index,
            asset_type: from.asset_type,
            amount: from.amount.into(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferInput {
    pub prev_out: AssetOutPoint,
    pub timelock: Option<Timelock>,
    pub lock_script: Bytes,
    pub unlock_script: Bytes,
}

impl From<AssetTransferInputType> for AssetTransferInput {
    fn from(from: AssetTransferInputType) -> Self {
        AssetTransferInput {
            prev_out: from.prev_out.into(),
            timelock: from.timelock,
            lock_script: from.lock_script,
            unlock_script: from.unlock_script,
        }
    }
}

impl From<AssetTransferInput> for AssetTransferInputType {
    fn from(from: AssetTransferInput) -> Self {
        AssetTransferInputType {
            prev_out: from.prev_out.into(),
            timelock: from.timelock,
            lock_script: from.lock_script,
            unlock_script: from.unlock_script,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub asset_type: H256,
    pub amount: Uint,
}

impl From<AssetTransferOutputType> for AssetTransferOutput {
    fn from(from: AssetTransferOutputType) -> Self {
        AssetTransferOutput {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
            asset_type: from.asset_type,
            amount: from.amount.into(),
        }
    }
}

impl From<AssetTransferOutput> for AssetTransferOutputType {
    fn from(from: AssetTransferOutput) -> Self {
        AssetTransferOutputType {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
            asset_type: from.asset_type,
            amount: from.amount.into(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMintOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub amount: Option<Uint>,
}

impl From<AssetMintOutputType> for AssetMintOutput {
    fn from(from: AssetMintOutputType) -> Self {
        AssetMintOutput {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
            amount: from.amount.map(|amount| amount.into()),
        }
    }
}

impl From<AssetMintOutput> for AssetMintOutputType {
    fn from(from: AssetMintOutput) -> Self {
        AssetMintOutputType {
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
            amount: from.amount.map(|amount| amount.into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub asset_type_from: H256,
    pub asset_type_to: H256,
    pub asset_type_fee: H256,
    pub asset_amount_from: Uint,
    pub asset_amount_to: Uint,
    pub asset_amount_fee: Uint,
    pub origin_outputs: Vec<AssetOutPoint>,
    pub expiration: u64,
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
}

impl From<OrderType> for Order {
    fn from(from: OrderType) -> Self {
        Order {
            asset_type_from: from.asset_type_from,
            asset_type_to: from.asset_type_to,
            asset_type_fee: from.asset_type_fee,
            asset_amount_from: from.asset_amount_from.into(),
            asset_amount_to: from.asset_amount_to.into(),
            asset_amount_fee: from.asset_amount_fee.into(),
            origin_outputs: from.origin_outputs.into_iter().map(From::from).collect(),
            expiration: from.expiration,
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
        }
    }
}

impl From<Order> for OrderType {
    fn from(from: Order) -> Self {
        OrderType {
            asset_type_from: from.asset_type_from,
            asset_type_to: from.asset_type_to,
            asset_type_fee: from.asset_type_fee,
            asset_amount_from: from.asset_amount_from.into(),
            asset_amount_to: from.asset_amount_to.into(),
            asset_amount_fee: from.asset_amount_fee.into(),
            origin_outputs: from.origin_outputs.into_iter().map(From::from).collect(),
            expiration: from.expiration,
            lock_script_hash: from.lock_script_hash,
            parameters: from.parameters,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderOnTransfer {
    pub order: Order,
    pub spent_amount: Uint,
    pub input_indices: Vec<usize>,
    pub output_indices: Vec<usize>,
}

impl From<OrderOnTransferType> for OrderOnTransfer {
    fn from(from: OrderOnTransferType) -> Self {
        OrderOnTransfer {
            order: from.order.into(),
            spent_amount: from.spent_amount.into(),
            input_indices: from.input_indices,
            output_indices: from.output_indices,
        }
    }
}

impl From<OrderOnTransfer> for OrderOnTransferType {
    fn from(from: OrderOnTransfer) -> Self {
        OrderOnTransferType {
            order: from.order.into(),
            spent_amount: from.spent_amount.into(),
            input_indices: from.input_indices,
            output_indices: from.output_indices,
        }
    }
}

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
