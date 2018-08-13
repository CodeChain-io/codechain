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

use ckey::{Address, Public, Signature};
use ctypes::parcel::{Action as ActionType, ChangeShard as ChangeShardType};
use ctypes::transaction::Transaction;
use ctypes::ShardId;
use primitives::{Bytes, H256, U256};

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
        receiver: Address,
        /// Transferred amount.
        amount: U256,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
    ChangeShardOwners {
        shard_id: ShardId,
        owners: Vec<Address>,
    },
    ChangeShardUsers {
        shard_id: ShardId,
        users: Vec<Address>,
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

impl From<ActionType> for Action {
    fn from(from: ActionType) -> Self {
        match from {
            ActionType::ChangeShardState {
                transactions,
                changes,
                signatures,
            } => Action::ChangeShardState {
                transactions,
                changes: changes.into_iter().map(From::from).collect(),
                signatures,
            },
            ActionType::Payment {
                receiver,
                amount,
            } => Action::Payment {
                receiver,
                amount,
            },
            ActionType::SetRegularKey {
                key,
            } => Action::SetRegularKey {
                key,
            },
            ActionType::CreateShard => Action::CreateShard,
            ActionType::ChangeShardOwners {
                shard_id,
                owners,
            } => Action::ChangeShardOwners {
                shard_id,
                owners,
            },
            ActionType::ChangeShardUsers {
                shard_id,
                users,
            } => Action::ChangeShardUsers {
                shard_id,
                users,
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

impl From<Action> for ActionType {
    fn from(from: Action) -> Self {
        match from {
            Action::ChangeShardState {
                transactions,
                changes,
                signatures,
            } => ActionType::ChangeShardState {
                transactions,
                changes: changes.into_iter().map(From::from).collect(),
                signatures,
            },
            Action::Payment {
                receiver,
                amount,
            } => ActionType::Payment {
                receiver,
                amount,
            },
            Action::SetRegularKey {
                key,
            } => ActionType::SetRegularKey {
                key,
            },
            Action::CreateShard => ActionType::CreateShard,
            Action::ChangeShardOwners {
                shard_id,
                owners,
            } => ActionType::ChangeShardOwners {
                shard_id,
                owners,
            },
            Action::ChangeShardUsers {
                shard_id,
                users,
            } => ActionType::ChangeShardUsers {
                shard_id,
                users,
            },
            Action::Custom(bytes) => ActionType::Custom(bytes),
        }
    }
}
