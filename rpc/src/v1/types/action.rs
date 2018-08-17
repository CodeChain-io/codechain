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

use ckey::{Error as KeyError, NetworkId, PlatformAddress, Public, Signature};
use ctypes::parcel::{Action as ActionType, ChangeShard as ChangeShardType};
use ctypes::ShardId;
use primitives::{Bytes, H256, U256};

use super::Transaction;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeShard {
    pub shard_id: ShardId,
    pub pre_root: H256,
    pub post_root: H256,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum Action {
    ChangeShardState {
        /// Transaction, can be either asset mint or asset transfer
        transactions: Vec<Transaction>,
        changes: Vec<ChangeShard>,
        signatures: Vec<Signature>,
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
    Custom(Bytes),
}

impl From<ChangeShardType> for ChangeShard {
    fn from(from: ChangeShardType) -> Self {
        Self {
            shard_id: from.shard_id,
            pre_root: from.pre_root,
            post_root: from.post_root,
        }
    }
}

impl Action {
    pub fn from_core(from: ActionType, network_id: NetworkId) -> Self {
        const VERSION: u8 = 0;
        match from {
            ActionType::ChangeShardState {
                transactions,
                changes,
                signatures,
            } => Action::ChangeShardState {
                transactions: transactions.into_iter().map(From::from).collect(),
                changes: changes.into_iter().map(From::from).collect(),
                signatures,
            },
            ActionType::Payment {
                receiver,
                amount,
            } => Action::Payment {
                receiver: PlatformAddress::create(VERSION, network_id, receiver),
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
                owners: owners.into_iter().map(|owner| PlatformAddress::create(VERSION, network_id, owner)).collect(),
            },
            ActionType::SetShardUsers {
                shard_id,
                users,
            } => Action::SetShardUsers {
                shard_id,
                users: users.into_iter().map(|user| PlatformAddress::create(VERSION, network_id, user)).collect(),
            },
            ActionType::Custom(bytes) => Action::Custom(bytes),
        }
    }
}

impl From<ChangeShard> for ChangeShardType {
    fn from(from: ChangeShard) -> Self {
        Self {
            shard_id: from.shard_id,
            pre_root: from.pre_root,
            post_root: from.post_root,
        }
    }
}

// FIXME: Use TryFrom.
impl From<Action> for Result<ActionType, KeyError> {
    fn from(from: Action) -> Self {
        Ok(match from {
            Action::ChangeShardState {
                transactions,
                changes,
                signatures,
            } => {
                let transactions: Result<_, _> = transactions.into_iter().map(From::from).collect();
                ActionType::ChangeShardState {
                    transactions: transactions?,
                    changes: changes.into_iter().map(From::from).collect(),
                    signatures,
                }
            }
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
            Action::Custom(bytes) => ActionType::Custom(bytes),
        })
    }
}
