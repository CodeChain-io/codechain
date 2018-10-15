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
use ctypes::transaction::{AssetMintOutput, AssetTransferInput, AssetTransferOutput, Transaction as TransactionType};
use ctypes::{ShardId, WorldId};
use primitives::H256;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum Transaction {
    #[serde(rename_all = "camelCase")]
    CreateWorld {
        network_id: NetworkId,
        shard_id: ShardId,
        seq: u64,
        owners: Vec<PlatformAddress>,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    SetWorldOwners {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        seq: u64,
        owners: Vec<PlatformAddress>,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    SetWorldUsers {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        seq: u64,
        users: Vec<PlatformAddress>,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetMint {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        metadata: String,
        registrar: Option<PlatformAddress>,
        nonce: u64,

        output: AssetMintOutput,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetTransfer {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        nonce: u64,
        hash: H256,
    },
    #[serde(rename_all = "camelCase")]
    AssetCompose {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        nonce: u64,
        metadata: String,
        registrar: Option<PlatformAddress>,
        inputs: Vec<AssetTransferInput>,
        output: AssetMintOutput,
    },
    #[serde(rename_all = "camelCase")]
    AssetDecompose {
        network_id: NetworkId,
        nonce: u64,
        input: AssetTransferInput,
        outputs: Vec<AssetTransferOutput>,
    },
}

impl From<TransactionType> for Transaction {
    fn from(from: TransactionType) -> Self {
        let hash = from.hash();
        match from {
            TransactionType::CreateWorld {
                network_id,
                shard_id,
                seq,
                owners,
            } => Transaction::CreateWorld {
                network_id,
                shard_id,
                seq,
                owners: owners.into_iter().map(|owner| PlatformAddress::new_v1(network_id, owner)).collect(),
                hash,
            },
            TransactionType::SetWorldOwners {
                network_id,
                shard_id,
                world_id,
                seq,
                owners,
            } => Transaction::SetWorldOwners {
                network_id,
                shard_id,
                world_id,
                seq,
                owners: owners.into_iter().map(|owner| PlatformAddress::new_v1(network_id, owner)).collect(),
                hash,
            },
            TransactionType::SetWorldUsers {
                network_id,
                shard_id,
                world_id,
                seq,
                users,
            } => Transaction::SetWorldUsers {
                network_id,
                shard_id,
                world_id,
                seq,
                users: users.into_iter().map(|user| PlatformAddress::new_v1(network_id, user)).collect(),
                hash,
            },
            TransactionType::AssetMint {
                network_id,
                shard_id,
                world_id,
                metadata,
                registrar,
                nonce,
                output,
            } => Transaction::AssetMint {
                network_id,
                shard_id,
                world_id,
                metadata,
                registrar: registrar.map(|registrar| PlatformAddress::new_v1(network_id, registrar)),
                nonce,
                output,
                hash,
            },
            TransactionType::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                nonce,
            } => Transaction::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                nonce,
                hash,
            },
            TransactionType::AssetCompose {
                network_id,
                shard_id,
                world_id,
                nonce,
                metadata,
                registrar,
                inputs,
                output,
            } => Transaction::AssetCompose {
                network_id,
                shard_id,
                world_id,
                nonce,
                metadata,
                registrar: registrar.map(|registrar| PlatformAddress::new_v1(network_id, registrar)),
                inputs,
                output,
            },
            TransactionType::AssetDecompose {
                network_id,
                nonce,
                input,
                outputs,
            } => Transaction::AssetDecompose {
                network_id,
                nonce,
                input,
                outputs,
            },
        }
    }
}

// FIXME: Use TryFrom.
impl From<Transaction> for Result<TransactionType, KeyError> {
    fn from(from: Transaction) -> Self {
        Ok(match from {
            Transaction::CreateWorld {
                network_id,
                shard_id,
                seq,
                owners,
                ..
            } => {
                let owners: Result<_, _> = owners.into_iter().map(PlatformAddress::try_into_address).collect();
                TransactionType::CreateWorld {
                    network_id,
                    shard_id,
                    seq,
                    owners: owners?,
                }
            }
            Transaction::SetWorldOwners {
                network_id,
                shard_id,
                world_id,
                seq,
                owners,
                ..
            } => {
                let owners: Result<_, _> = owners.into_iter().map(PlatformAddress::try_into_address).collect();
                TransactionType::SetWorldOwners {
                    network_id,
                    shard_id,
                    world_id,
                    seq,
                    owners: owners?,
                }
            }
            Transaction::SetWorldUsers {
                network_id,
                shard_id,
                world_id,
                seq,
                users,
                ..
            } => {
                let users: Result<_, _> = users.into_iter().map(PlatformAddress::try_into_address).collect();
                TransactionType::SetWorldUsers {
                    network_id,
                    shard_id,
                    world_id,
                    seq,
                    users: users?,
                }
            }
            Transaction::AssetMint {
                network_id,
                shard_id,
                world_id,
                metadata,
                registrar,
                nonce,
                output,
                ..
            } => {
                let registrar = match registrar {
                    Some(registrar) => Some(registrar.try_into_address()?),
                    None => None,
                };
                TransactionType::AssetMint {
                    network_id,
                    shard_id,
                    world_id,
                    metadata,
                    registrar,
                    nonce,
                    output,
                }
            }
            Transaction::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                nonce,
                ..
            } => TransactionType::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                nonce,
            },
            Transaction::AssetCompose {
                network_id,
                shard_id,
                world_id,
                nonce,
                metadata,
                registrar,
                inputs,
                output,
            } => {
                let registrar = match registrar {
                    Some(registrar) => Some(registrar.try_into_address()?),
                    None => None,
                };
                TransactionType::AssetCompose {
                    network_id,
                    shard_id,
                    world_id,
                    nonce,
                    metadata,
                    registrar,
                    inputs,
                    output,
                }
            }
            Transaction::AssetDecompose {
                network_id,
                nonce,
                input,
                outputs,
            } => TransactionType::AssetDecompose {
                network_id,
                nonce,
                input,
                outputs,
            },
        })
    }
}
