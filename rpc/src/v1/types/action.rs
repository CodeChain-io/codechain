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
use ckey::{Error as KeyError, NetworkId, PlatformAddress, Public, Signature};
use ctypes::parcel::Action as ActionType;
use ctypes::transaction::Transaction as TransactionType;
use ctypes::ShardId;
use primitives::{Bytes, H160, H256};

use super::{AssetMintOutput, AssetTransferInput, AssetTransferOutput, OrderOnTransfer};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum Action {
    #[serde(rename_all = "camelCase")]
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,

        output: AssetMintOutput,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    ChangeAssetScheme {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        inputs: Vec<AssetTransferInput>,
        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    DecomposeAsset {
        network_id: NetworkId,
        input: Box<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    UnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
    },
    Pay {
        receiver: PlatformAddress,
        amount: Uint,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
    SetShardOwners {
        shard_id: ShardId,
        owners: Vec<PlatformAddress>,
    },
    SetShardUsers {
        shard_id: ShardId,
        users: Vec<PlatformAddress>,
    },
    WrapCCC {
        shard_id: ShardId,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        amount: Uint,
    },
    Store {
        content: String,
        certifier: PlatformAddress,
        signature: Signature,
    },
    Remove {
        hash: H256,
        signature: Signature,
    },
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum ActionWithId {
    #[serde(rename_all = "camelCase")]
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,

        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,

        id: H256,
    },
    #[serde(rename_all = "camelCase")]
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,

        approvals: Vec<Signature>,

        id: H256,
    },
    #[serde(rename_all = "camelCase")]
    ChangeAssetScheme {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,

        approvals: Vec<Signature>,

        id: H256,
    },
    #[serde(rename_all = "camelCase")]
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        inputs: Vec<AssetTransferInput>,
        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,

        id: H256,
    },
    #[serde(rename_all = "camelCase")]
    DecomposeAsset {
        network_id: NetworkId,
        input: Box<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,

        approvals: Vec<Signature>,

        id: H256,
    },
    #[serde(rename_all = "camelCase")]
    UnwrapCCC {
        network_id: NetworkId,
        burn: Box<AssetTransferInput>,

        approvals: Vec<Signature>,

        id: H256,
    },
    Pay {
        receiver: PlatformAddress,
        amount: Uint,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
    SetShardOwners {
        shard_id: ShardId,
        owners: Vec<PlatformAddress>,
    },
    SetShardUsers {
        shard_id: ShardId,
        users: Vec<PlatformAddress>,
    },
    WrapCCC {
        shard_id: ShardId,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        amount: Uint,
    },
    Store {
        content: String,
        certifier: PlatformAddress,
        signature: Signature,
    },
    Remove {
        hash: H256,
        signature: Signature,
    },
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
}

impl ActionWithId {
    pub fn from_core(from: ActionType, network_id: NetworkId) -> Self {
        match from {
            ActionType::AssetTransaction {
                transaction,
                approvals,
            } => {
                let id = transaction.hash();
                match transaction {
                    TransactionType::AssetMint {
                        network_id,
                        shard_id,
                        metadata,
                        approver,
                        administrator,
                        output,
                    } => ActionWithId::MintAsset {
                        network_id,
                        shard_id,
                        metadata,
                        approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                        administrator: administrator
                            .map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                        output: Box::new(output.into()),
                        approvals,
                        id,
                    },
                    TransactionType::AssetTransfer {
                        network_id,
                        burns,
                        inputs,
                        outputs,
                        orders,
                    } => ActionWithId::TransferAsset {
                        network_id,
                        burns: burns.into_iter().map(From::from).collect(),
                        inputs: inputs.into_iter().map(From::from).collect(),
                        outputs: outputs.into_iter().map(From::from).collect(),
                        orders: orders.into_iter().map(From::from).collect(),
                        approvals,
                        id,
                    },
                    TransactionType::AssetSchemeChange {
                        network_id,
                        asset_type,
                        metadata,
                        approver,
                        administrator,
                    } => ActionWithId::ChangeAssetScheme {
                        network_id,
                        asset_type,
                        metadata,
                        approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                        administrator: administrator
                            .map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                        approvals,
                        id,
                    },
                    TransactionType::AssetCompose {
                        network_id,
                        shard_id,
                        metadata,
                        approver,
                        administrator,
                        inputs,
                        output,
                    } => ActionWithId::ComposeAsset {
                        network_id,
                        shard_id,
                        metadata,
                        approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                        administrator: administrator
                            .map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                        inputs: inputs.into_iter().map(From::from).collect(),
                        output: Box::new(output.into()),
                        approvals,
                        id,
                    },
                    TransactionType::AssetDecompose {
                        network_id,
                        input,
                        outputs,
                    } => ActionWithId::DecomposeAsset {
                        network_id,
                        input: Box::new(input.into()),
                        outputs: outputs.into_iter().map(From::from).collect(),
                        approvals,
                        id,
                    },
                    TransactionType::AssetUnwrapCCC {
                        network_id,
                        burn,
                    } => ActionWithId::UnwrapCCC {
                        network_id,
                        burn: Box::new(burn.into()),
                        approvals,
                        id,
                    },
                }
            }
            ActionType::Pay {
                receiver,
                amount,
            } => ActionWithId::Pay {
                receiver: PlatformAddress::new_v1(network_id, receiver),
                amount: amount.into(),
            },
            ActionType::SetRegularKey {
                key,
            } => ActionWithId::SetRegularKey {
                key,
            },
            ActionType::CreateShard => ActionWithId::CreateShard,
            ActionType::SetShardOwners {
                shard_id,
                owners,
            } => ActionWithId::SetShardOwners {
                shard_id,
                owners: owners.into_iter().map(|owner| PlatformAddress::new_v1(network_id, owner)).collect(),
            },
            ActionType::SetShardUsers {
                shard_id,
                users,
            } => ActionWithId::SetShardUsers {
                shard_id,
                users: users.into_iter().map(|user| PlatformAddress::new_v1(network_id, user)).collect(),
            },
            ActionType::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount,
            } => ActionWithId::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount: amount.into(),
            },
            ActionType::Store {
                content,
                certifier,
                signature,
            } => ActionWithId::Store {
                content,
                certifier: PlatformAddress::new_v1(network_id, certifier),
                signature,
            },
            ActionType::Remove {
                hash,
                signature,
            } => ActionWithId::Remove {
                hash,
                signature,
            },
            ActionType::Custom {
                handler_id,
                bytes,
            } => ActionWithId::Custom {
                handler_id,
                bytes,
            },
        }
    }
}

// FIXME: Use TryFrom.
impl From<Action> for Result<ActionType, KeyError> {
    fn from(from: Action) -> Self {
        Ok(match from {
            Action::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                output,
                approvals,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                ActionType::AssetTransaction {
                    transaction: TransactionType::AssetMint {
                        network_id,
                        shard_id,
                        metadata,
                        approver,
                        administrator,
                        output: output.into(),
                    },
                    approvals,
                }
            }
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,

                approvals,
            } => ActionType::AssetTransaction {
                transaction: TransactionType::AssetTransfer {
                    network_id,
                    burns: burns.into_iter().map(From::from).collect(),
                    inputs: inputs.into_iter().map(From::from).collect(),
                    outputs: outputs.into_iter().map(From::from).collect(),
                    orders: orders.into_iter().map(From::from).collect(),
                },
                approvals,
            },
            Action::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,

                approvals,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                ActionType::AssetTransaction {
                    transaction: TransactionType::AssetSchemeChange {
                        network_id,
                        asset_type,
                        metadata,
                        approver,
                        administrator,
                    },
                    approvals,
                }
            }
            Action::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                inputs,
                output,

                approvals,
            } => {
                let approver = match approver {
                    Some(approver) => Some(approver.try_into_address()?),
                    None => None,
                };
                let administrator = match administrator {
                    Some(administrator) => Some(administrator.try_into_address()?),
                    None => None,
                };
                ActionType::AssetTransaction {
                    transaction: TransactionType::AssetCompose {
                        network_id,
                        shard_id,
                        metadata,
                        approver,
                        administrator,
                        inputs: inputs.into_iter().map(|input| input.into()).collect(),
                        output: (*output).into(),
                    },
                    approvals,
                }
            }
            Action::DecomposeAsset {
                network_id,
                input,
                outputs,

                approvals,
            } => ActionType::AssetTransaction {
                transaction: TransactionType::AssetDecompose {
                    network_id,
                    input: (*input).into(),
                    outputs: outputs.into_iter().map(|output| output.into()).collect(),
                },
                approvals,
            },
            Action::UnwrapCCC {
                network_id,
                burn,
            } => ActionType::AssetTransaction {
                transaction: TransactionType::AssetUnwrapCCC {
                    network_id,
                    burn: burn.into(),
                },
                approvals: vec![],
            },
            Action::Pay {
                receiver,
                amount,
            } => ActionType::Pay {
                receiver: receiver.try_into_address()?,
                amount: amount.into(),
            },
            Action::SetRegularKey {
                key,
            } => ActionType::SetRegularKey {
                key,
            },
            Action::CreateShard => ActionType::CreateShard,
            Action::SetShardOwners {
                shard_id,
                owners,
            } => {
                let owners: Result<_, _> = owners.into_iter().map(PlatformAddress::try_into_address).collect();
                ActionType::SetShardOwners {
                    shard_id,
                    owners: owners?,
                }
            }
            Action::SetShardUsers {
                shard_id,
                users,
            } => {
                let users: Result<_, _> = users.into_iter().map(PlatformAddress::try_into_address).collect();
                ActionType::SetShardUsers {
                    shard_id,
                    users: users?,
                }
            }
            Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount,
            } => ActionType::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount: amount.into(),
            },
            Action::Store {
                content,
                certifier,
                signature,
            } => ActionType::Store {
                content,
                certifier: certifier.try_into_address()?,
                signature,
            },
            Action::Remove {
                hash,
                signature,
            } => ActionType::Remove {
                hash,
                signature,
            },
            Action::Custom {
                handler_id,
                bytes,
            } => ActionType::Custom {
                handler_id,
                bytes,
            },
        })
    }
}
