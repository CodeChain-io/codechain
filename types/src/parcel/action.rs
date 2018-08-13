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

use ccrypto::Blake;
use ckey::{Address, Public, Signature};
use primitives::{Bytes, H256, U256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::transaction::Transaction;
use super::super::ShardId;

const CHANGE_SHARD_STATE: u8 = 1;
const PAYMENT: u8 = 2;
const SET_REGULAR_KEY: u8 = 3;
const CREATE_SHARD: u8 = 4;
const CHANGE_SHARD_OWNERS: u8 = 5;
const CHANGE_SHARD_USERS: u8 = 6;
const CUSTOM: u8 = 0xFF;

#[derive(Debug, Clone, PartialEq, Eq, RlpDecodable, RlpEncodable)]
pub struct ChangeShard {
    pub shard_id: ShardId,
    pub pre_root: H256,
    pub post_root: H256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Action {
    pub fn hash(&self) -> H256 {
        let rlp = self.rlp_bytes();
        Blake::blake(rlp)
    }
}

impl Encodable for Action {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Action::ChangeShardState {
                transactions,
                changes,
                signatures,
            } => {
                s.begin_list(4);
                s.append(&CHANGE_SHARD_STATE);
                s.append_list(transactions);
                s.append_list(changes);
                s.append_list(signatures);
            }
            Action::Payment {
                receiver,
                amount,
            } => {
                s.begin_list(3);
                s.append(&PAYMENT);
                s.append(receiver);
                s.append(amount);
            }
            Action::SetRegularKey {
                key,
            } => {
                s.begin_list(2);
                s.append(&SET_REGULAR_KEY);
                s.append(key);
            }
            Action::CreateShard => {
                s.begin_list(1);
                s.append(&CREATE_SHARD);
            }
            Action::ChangeShardOwners {
                shard_id,
                owners,
            } => {
                s.begin_list(3);
                s.append(&CHANGE_SHARD_OWNERS);
                s.append(shard_id);
                s.append_list(owners);
            }
            Action::ChangeShardUsers {
                shard_id,
                users,
            } => {
                s.begin_list(3);
                s.append(&CHANGE_SHARD_USERS);
                s.append(shard_id);
                s.append_list(users);
            }
            Action::Custom(bytes) => {
                s.begin_list(2);
                s.append(&CUSTOM);
                s.append(bytes);
            }
        }
    }
}

impl Decodable for Action {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.val_at(0)? {
            CHANGE_SHARD_STATE => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::ChangeShardState {
                    transactions: rlp.list_at(1)?,
                    changes: rlp.list_at(2)?,
                    signatures: rlp.list_at(3)?,
                })
            }
            PAYMENT => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::Payment {
                    receiver: rlp.val_at(1)?,
                    amount: rlp.val_at(2)?,
                })
            }
            SET_REGULAR_KEY => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::SetRegularKey {
                    key: rlp.val_at(1)?,
                })
            }
            CREATE_SHARD => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::CreateShard)
            }
            CHANGE_SHARD_OWNERS => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::ChangeShardOwners {
                    shard_id: rlp.val_at(1)?,
                    owners: rlp.list_at(2)?,
                })
            }
            CHANGE_SHARD_USERS => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::ChangeShardUsers {
                    shard_id: rlp.val_at(1)?,
                    users: rlp.list_at(2)?,
                })
            }
            CUSTOM => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::Custom(rlp.val_at(1)?))
            }
            _ => Err(DecoderError::Custom("Unexpected action prefix")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_decode_change_shard_owners() {
        rlp_encode_and_decode_test!(Action::ChangeShardOwners {
            shard_id: 1,
            owners: vec![Address::random(), Address::random()],
        });
    }

    #[test]
    fn encode_and_decode_change_shard_users() {
        rlp_encode_and_decode_test!(Action::ChangeShardUsers {
            shard_id: 1,
            users: vec![Address::random(), Address::random()],
        });
    }
}
