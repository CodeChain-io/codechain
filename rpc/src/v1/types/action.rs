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

use ckey::{Error as KeyError, NetworkId, PlatformAddress, Public};
use ctypes::parcel::Action as ActionType;
use ctypes::ShardId;
use primitives::{Bytes, H160, U256};

use super::Transaction;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum Action {
    /// Transaction, can be either asset mint or asset transfer
    AssetTransaction {
        transaction: Transaction,
    },
    Payment {
        receiver: PlatformAddress,
        /// Transferred amount.
        amount: U256,
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
        amount: U256,
    },
    Custom(Bytes),
}

impl Action {
    pub fn from_core(from: ActionType, network_id: NetworkId) -> Self {
        match from {
            ActionType::AssetTransaction(transaction) => Action::AssetTransaction {
                transaction: transaction.into(),
            },
            ActionType::Payment {
                receiver,
                amount,
            } => Action::Payment {
                receiver: PlatformAddress::new_v1(network_id, receiver),
                amount,
            },
            ActionType::SetRegularKey {
                key,
            } => Action::SetRegularKey {
                key,
            },
            ActionType::CreateShard => Action::CreateShard,
            ActionType::SetShardOwners {
                shard_id,
                owners,
            } => Action::SetShardOwners {
                shard_id,
                owners: owners.into_iter().map(|owner| PlatformAddress::new_v1(network_id, owner)).collect(),
            },
            ActionType::SetShardUsers {
                shard_id,
                users,
            } => Action::SetShardUsers {
                shard_id,
                users: users.into_iter().map(|user| PlatformAddress::new_v1(network_id, user)).collect(),
            },
            ActionType::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount,
            } => Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount,
            },
            ActionType::Custom(bytes) => Action::Custom(bytes),
        }
    }
}

// FIXME: Use TryFrom.
impl From<Action> for Result<ActionType, KeyError> {
    fn from(from: Action) -> Self {
        Ok(match from {
            Action::AssetTransaction {
                transaction,
            } => ActionType::AssetTransaction(Result::from(transaction)?),
            Action::Payment {
                receiver,
                amount,
            } => ActionType::Payment {
                receiver: receiver.try_into_address()?,
                amount,
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
                amount,
            },
            Action::Custom(bytes) => ActionType::Custom(bytes),
        })
    }
}
