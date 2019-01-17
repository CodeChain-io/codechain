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

use std::iter::FromIterator;

use cjson::uint::Uint;
use ckey::{NetworkId, PlatformAddress, Public, Signature};
use ctypes::transaction::{Action as ActionType, AssetMintOutput as AssetMintOutputType};
use ctypes::ShardId;
use primitives::{Bytes, H160, H256};
use rustc_serialize::hex::FromHexError;

use super::super::errors::ConversionError;
use super::{AssetMintOutput, AssetTransferInput, AssetTransferOutput, OrderOnTransfer};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Action {
    #[serde(rename_all = "camelCase")]
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        output: Box<AssetMintOutput>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,

        metadata: String,
        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    ChangeAssetScheme {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

        approvals: Vec<Signature>,
    },
    #[serde(rename_all = "camelCase")]
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,
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
        quantity: Uint,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
    #[serde(rename_all = "camelCase")]
    SetShardOwners {
        shard_id: ShardId,
        owners: Vec<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    SetShardUsers {
        shard_id: ShardId,
        users: Vec<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    WrapCCC {
        shard_id: ShardId,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        quantity: Uint,
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
    #[serde(rename_all = "camelCase")]
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ActionWithId {
    #[serde(rename_all = "camelCase")]
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<PlatformAddress>,
        administrator: Option<PlatformAddress>,
        allowed_script_hashes: Vec<H160>,

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

        metadata: String,
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
        allowed_script_hashes: Vec<H160>,

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
        allowed_script_hashes: Vec<H160>,
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
        quantity: Uint,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
    #[serde(rename_all = "camelCase")]
    SetShardOwners {
        shard_id: ShardId,
        owners: Vec<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    SetShardUsers {
        shard_id: ShardId,
        users: Vec<PlatformAddress>,
    },
    #[serde(rename_all = "camelCase")]
    WrapCCC {
        shard_id: ShardId,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        quantity: Uint,
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
    #[serde(rename_all = "camelCase")]
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
}

impl ActionWithId {
    pub fn from_core(from: ActionType, network_id: NetworkId) -> Self {
        let tracker = from.tracker();
        match from {
            ActionType::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,

                output,
                approvals,
            } => {
                let id = tracker.unwrap();
                ActionWithId::MintAsset {
                    network_id,
                    shard_id,
                    metadata,
                    approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                    administrator: administrator
                        .map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                    allowed_script_hashes,
                    output: Box::new((*output).into()),
                    approvals,
                    id,
                }
            }
            ActionType::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
                metadata,
                approvals,
            } => {
                let id = tracker.unwrap();
                ActionWithId::TransferAsset {
                    network_id,
                    burns: burns.into_iter().map(From::from).collect(),
                    inputs: inputs.into_iter().map(From::from).collect(),
                    outputs: outputs.into_iter().map(From::from).collect(),
                    orders: orders.into_iter().map(From::from).collect(),
                    metadata,
                    approvals,
                    id,
                }
            }
            ActionType::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                approvals,
            } => {
                let id = tracker.unwrap();
                ActionWithId::ChangeAssetScheme {
                    network_id,
                    asset_type,
                    metadata,
                    approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                    administrator: administrator
                        .map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                    allowed_script_hashes,
                    approvals,
                    id,
                }
            }
            ActionType::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output,
                approvals,
            } => {
                let id = tracker.unwrap();
                ActionWithId::ComposeAsset {
                    network_id,
                    shard_id,
                    metadata,
                    approver: approver.map(|approver| PlatformAddress::new_v1(network_id, approver)),
                    administrator: administrator
                        .map(|administrator| PlatformAddress::new_v1(network_id, administrator)),
                    allowed_script_hashes,
                    inputs: inputs.into_iter().map(From::from).collect(),
                    output: Box::new((*output).into()),
                    approvals,
                    id,
                }
            }
            ActionType::DecomposeAsset {
                network_id,
                input,
                outputs,
                approvals,
            } => {
                let id = tracker.unwrap();
                ActionWithId::DecomposeAsset {
                    network_id,
                    input: Box::new(input.into()),
                    outputs: outputs.into_iter().map(From::from).collect(),
                    approvals,
                    id,
                }
            }
            ActionType::UnwrapCCC {
                network_id,
                burn,
                approvals,
            } => {
                let id = tracker.unwrap();
                ActionWithId::UnwrapCCC {
                    network_id,
                    burn: Box::new(burn.into()),
                    approvals,
                    id,
                }
            }
            ActionType::Pay {
                receiver,
                quantity,
            } => ActionWithId::Pay {
                receiver: PlatformAddress::new_v1(network_id, receiver),
                quantity: quantity.into(),
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
                quantity,
            } => ActionWithId::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                quantity: quantity.into(),
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
impl From<Action> for Result<ActionType, ConversionError> {
    fn from(from: Action) -> Self {
        Ok(match from {
            Action::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
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
                let output_content = Result::<AssetMintOutputType, FromHexError>::from(*output)?;
                ActionType::MintAsset {
                    network_id,
                    shard_id,
                    metadata,
                    approver,
                    administrator,
                    allowed_script_hashes,
                    output: Box::new(output_content),
                    approvals,
                }
            }
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,

                metadata,
                approvals,
            } => {
                let iter_outputs = outputs.into_iter().map(From::from);
                ActionType::TransferAsset {
                    network_id,
                    burns: burns.into_iter().map(From::from).collect(),
                    inputs: inputs.into_iter().map(From::from).collect(),
                    outputs: Result::from_iter(iter_outputs)?,
                    orders: orders.into_iter().map(From::from).collect(),
                    metadata,
                    approvals,
                }
            }
            Action::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,

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
                ActionType::ChangeAssetScheme {
                    network_id,
                    asset_type,
                    metadata,
                    approver,
                    administrator,
                    allowed_script_hashes,
                    approvals,
                }
            }
            Action::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
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
                let output_content = Result::<AssetMintOutputType, FromHexError>::from(*output)?;
                ActionType::ComposeAsset {
                    network_id,
                    shard_id,
                    metadata,
                    approver,
                    administrator,
                    allowed_script_hashes,
                    inputs: inputs.into_iter().map(|input| input.into()).collect(),
                    output: Box::new(output_content),
                    approvals,
                }
            }
            Action::DecomposeAsset {
                network_id,
                input,
                outputs,

                approvals,
            } => {
                let iter_outputs = outputs.into_iter().map(From::from);
                ActionType::DecomposeAsset {
                    network_id,
                    input: (*input).into(),
                    outputs: Result::from_iter(iter_outputs)?,
                    approvals,
                }
            }
            Action::UnwrapCCC {
                network_id,
                burn,
            } => ActionType::UnwrapCCC {
                network_id,
                burn: burn.into(),
                approvals: vec![],
            },
            Action::Pay {
                receiver,
                quantity,
            } => ActionType::Pay {
                receiver: receiver.try_into_address()?,
                quantity: quantity.into(),
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
                quantity,
            } => ActionType::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                quantity: quantity.into(),
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
