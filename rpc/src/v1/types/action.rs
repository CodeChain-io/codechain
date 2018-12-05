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
use ctypes::ShardId;
use primitives::{Bytes, H160};

use super::{Transaction, TransactionWithHash};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum Action {
    AssetTransaction {
        transaction: Box<Transaction>,
        approvals: Vec<Signature>,
    },
    Payment {
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
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum ActionWithTxHash {
    AssetTransaction {
        transaction: Box<TransactionWithHash>,
        approvals: Vec<Signature>,
    },
    Payment {
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
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
}

impl ActionWithTxHash {
    pub fn from_core(from: ActionType, network_id: NetworkId) -> Self {
        match from {
            ActionType::AssetTransaction {
                transaction,
                approvals,
            } => ActionWithTxHash::AssetTransaction {
                transaction: Box::new(transaction.into()),
                approvals,
            },
            ActionType::Payment {
                receiver,
                amount,
            } => ActionWithTxHash::Payment {
                receiver: PlatformAddress::new_v1(network_id, receiver),
                amount: amount.into(),
            },
            ActionType::SetRegularKey {
                key,
            } => ActionWithTxHash::SetRegularKey {
                key,
            },
            ActionType::CreateShard => ActionWithTxHash::CreateShard,
            ActionType::SetShardOwners {
                shard_id,
                owners,
            } => ActionWithTxHash::SetShardOwners {
                shard_id,
                owners: owners.into_iter().map(|owner| PlatformAddress::new_v1(network_id, owner)).collect(),
            },
            ActionType::SetShardUsers {
                shard_id,
                users,
            } => ActionWithTxHash::SetShardUsers {
                shard_id,
                users: users.into_iter().map(|user| PlatformAddress::new_v1(network_id, user)).collect(),
            },
            ActionType::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount,
            } => ActionWithTxHash::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount: amount.into(),
            },
            ActionType::Custom {
                handler_id,
                bytes,
            } => ActionWithTxHash::Custom {
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
            Action::AssetTransaction {
                transaction,
                approvals,
            } => ActionType::AssetTransaction {
                transaction: Result::from(*transaction)?,
                approvals,
            },
            Action::Payment {
                receiver,
                amount,
            } => ActionType::Payment {
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
