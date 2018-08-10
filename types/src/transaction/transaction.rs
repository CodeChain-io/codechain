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

use std::collections::HashMap;
use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};
use ccrypto::blake256;
use ckey::{Address, NetworkId};
use primitives::{Bytes, H256, U128};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::{ShardId, WorldId};
use super::error::Error;

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetOutPoint {
    pub transaction_hash: H256,
    pub index: usize,
    pub asset_type: H256,
    pub amount: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferInput {
    pub prev_out: AssetOutPoint,
    pub lock_script: Bytes,
    pub unlock_script: Bytes,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferOutput {
    pub lock_script_hash: H256,
    pub parameters: Vec<Bytes>,
    pub asset_type: H256,
    pub amount: u64,
}

/// Parcel transaction type.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum Transaction {
    #[serde(rename_all = "camelCase")]
    CreateWorld {
        network_id: NetworkId,
        shard_id: ShardId,
        nonce: u64,
        owners: Vec<Address>,
    },
    #[serde(rename_all = "camelCase")]
    SetWorldOwners {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        nonce: u64,
        owners: Vec<Address>,
    },
    #[serde(rename_all = "camelCase")]
    SetWorldUsers {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        nonce: u64,
        users: Vec<Address>,
    },
    #[serde(rename_all = "camelCase")]
    AssetMint {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        metadata: String,
        registrar: Option<Address>,
        nonce: u64,

        output: AssetMintOutput,
    },
    #[serde(rename_all = "camelCase")]
    AssetTransfer {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        nonce: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMintOutput {
    pub lock_script_hash: H256,
    pub parameters: Vec<Bytes>,
    pub amount: Option<u64>,
}

impl Transaction {
    pub fn without_script(&self) -> Self {
        match self {
            Transaction::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                nonce,
            } => {
                let new_burns: Vec<_> = burns
                    .iter()
                    .map(|input| AssetTransferInput {
                        prev_out: input.prev_out.clone(),
                        lock_script: Vec::new(),
                        unlock_script: Vec::new(),
                    })
                    .collect();
                let new_inputs: Vec<_> = inputs
                    .iter()
                    .map(|input| AssetTransferInput {
                        prev_out: input.prev_out.clone(),
                        lock_script: Vec::new(),
                        unlock_script: Vec::new(),
                    })
                    .collect();
                Transaction::AssetTransfer {
                    network_id: *network_id,
                    burns: new_burns,
                    inputs: new_inputs,
                    outputs: outputs.clone(),
                    nonce: *nonce,
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn hash(&self) -> H256 {
        blake256(&*self.rlp_bytes())
    }

    pub fn hash_without_script(&self) -> H256 {
        blake256(&*self.without_script().rlp_bytes())
    }

    pub fn network_id(&self) -> NetworkId {
        match self {
            Transaction::CreateWorld {
                network_id,
                ..
            } => *network_id,
            Transaction::SetWorldOwners {
                network_id,
                ..
            } => *network_id,
            Transaction::SetWorldUsers {
                network_id,
                ..
            } => *network_id,
            Transaction::AssetTransfer {
                network_id,
                ..
            } => *network_id,
            Transaction::AssetMint {
                network_id,
                ..
            } => *network_id,
        }
    }

    pub fn related_shards(&self) -> Vec<ShardId> {
        match self {
            Transaction::CreateWorld {
                shard_id,
                ..
            } => vec![*shard_id],
            Transaction::SetWorldOwners {
                shard_id,
                ..
            } => vec![*shard_id],
            Transaction::SetWorldUsers {
                shard_id,
                ..
            } => vec![*shard_id],
            Transaction::AssetTransfer {
                burns,
                inputs,
                ..
            } => {
                let mut shards: Vec<ShardId> = burns
                    .iter()
                    .map(AssetTransferInput::related_shard)
                    .chain(inputs.iter().map(AssetTransferInput::related_shard))
                    .collect();
                shards.sort_unstable();
                shards.dedup();
                shards
            }
            Transaction::AssetMint {
                shard_id,
                ..
            } => vec![*shard_id],
        }
    }

    pub fn verify(&self) -> Result<(), Error> {
        match self {
            Transaction::CreateWorld {
                ..
            } => Ok(()),
            Transaction::SetWorldOwners {
                ..
            } => Ok(()),
            Transaction::SetWorldUsers {
                ..
            } => Ok(()),
            Transaction::AssetTransfer {
                inputs,
                outputs,
                ..
            } => {
                // FIXME: check burns
                if !is_input_and_output_consistent(inputs, outputs) {
                    return Err(Error::InconsistentTransactionInOut)
                }
                Ok(())
            }
            Transaction::AssetMint {
                ..
            } => Ok(()),
        }
    }
}

fn is_input_and_output_consistent(inputs: &[AssetTransferInput], outputs: &[AssetTransferOutput]) -> bool {
    let mut sum: HashMap<H256, U128> = HashMap::new();

    for input in inputs {
        let ref asset_type = input.prev_out.asset_type;
        let ref amount = input.prev_out.amount;
        let current_amount = sum.get(&asset_type).cloned().unwrap_or(U128::zero());
        sum.insert(asset_type.clone(), current_amount + U128::from(*amount));
    }
    for output in outputs {
        let ref asset_type = output.asset_type;
        let ref amount = output.amount;
        let current_amount = if let Some(current_amount) = sum.get(&asset_type) {
            if current_amount < &U128::from(*amount) {
                return false
            }
            current_amount.clone()
        } else {
            return false
        };
        let t = sum.insert(asset_type.clone(), current_amount - From::from(*amount));
        debug_assert!(t.is_some());
    }

    sum.iter().all(|(_, sum)| sum.is_zero())
}

type TransactionId = u8;
const CREATE_WORLD_ID: TransactionId = 0x01;
const SET_WORLD_OWNERS_ID: TransactionId = 0x02;
const ASSET_MINT_ID: TransactionId = 0x03;
const ASSET_TRANSFER_ID: TransactionId = 0x04;
const SET_WORLD_USERS_ID: TransactionId = 0x05;

impl Decodable for Transaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        match d.val_at(0)? {
            CREATE_WORLD_ID => {
                if d.item_count()? != 5 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }

                Ok(Transaction::CreateWorld {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    nonce: d.val_at(3)?,
                    owners: d.list_at(4)?,
                })
            }
            SET_WORLD_OWNERS_ID => {
                if d.item_count()? != 6 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }

                Ok(Transaction::SetWorldOwners {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    world_id: d.val_at(3)?,
                    nonce: d.val_at(4)?,
                    owners: d.list_at(5)?,
                })
            }
            SET_WORLD_USERS_ID => {
                if d.item_count()? != 6 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }

                Ok(Transaction::SetWorldUsers {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    world_id: d.val_at(3)?,
                    nonce: d.val_at(4)?,
                    users: d.list_at(5)?,
                })
            }
            ASSET_MINT_ID => {
                if d.item_count()? != 10 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetMint {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    world_id: d.val_at(3)?,
                    metadata: d.val_at(4)?,
                    output: AssetMintOutput {
                        lock_script_hash: d.val_at(5)?,
                        parameters: d.val_at(6)?,
                        amount: d.val_at(7)?,
                    },
                    registrar: d.val_at(8)?,
                    nonce: d.val_at(9)?,
                })
            }
            ASSET_TRANSFER_ID => {
                if d.item_count()? != 6 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetTransfer {
                    network_id: d.val_at(1)?,
                    burns: d.list_at(2)?,
                    inputs: d.list_at(3)?,
                    outputs: d.list_at(4)?,
                    nonce: d.val_at(5)?,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected transaction")),
        }
    }
}

impl Encodable for Transaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Transaction::CreateWorld {
                network_id,
                shard_id,
                nonce,
                owners,
            } => s
                .begin_list(5)
                .append(&CREATE_WORLD_ID)
                .append(network_id)
                .append(shard_id)
                .append(nonce)
                .append_list(&owners),
            Transaction::SetWorldOwners {
                network_id,
                shard_id,
                world_id,
                nonce,
                owners,
            } => s
                .begin_list(6)
                .append(&SET_WORLD_OWNERS_ID)
                .append(network_id)
                .append(shard_id)
                .append(world_id)
                .append(nonce)
                .append_list(&owners),
            Transaction::SetWorldUsers {
                network_id,
                shard_id,
                world_id,
                nonce,
                users,
            } => s
                .begin_list(6)
                .append(&SET_WORLD_OWNERS_ID)
                .append(network_id)
                .append(shard_id)
                .append(world_id)
                .append(nonce)
                .append_list(&users),
            Transaction::AssetMint {
                network_id,
                shard_id,
                world_id,
                metadata,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        parameters,
                        amount,
                    },
                registrar,
                nonce,
            } => s
                .begin_list(10)
                .append(&ASSET_MINT_ID)
                .append(network_id)
                .append(shard_id)
                .append(world_id)
                .append(metadata)
                .append(lock_script_hash)
                .append(parameters)
                .append(amount)
                .append(registrar)
                .append(nonce),
            Transaction::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                nonce,
            } => s
                .begin_list(6)
                .append(&ASSET_TRANSFER_ID)
                .append(network_id)
                .append_list(burns)
                .append_list(inputs)
                .append_list(outputs)
                .append(nonce),
        };
    }
}

impl AssetOutPoint {
    pub fn related_shard(&self) -> ShardId {
        debug_assert_eq!(::std::mem::size_of::<u16>(), ::std::mem::size_of::<ShardId>());
        Cursor::new(&self.asset_type[2..4]).read_u16::<BigEndian>().unwrap()
    }
}

impl AssetTransferInput {
    pub fn related_shard(&self) -> ShardId {
        self.prev_out.related_shard()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn related_shard_of_asset_out_point() {
        let mut asset_type = H256::new();
        asset_type[2..4].clone_from_slice(&[0xBE, 0xEF]);

        let p = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 3,
            asset_type,
            amount: 34,
        };

        assert_eq!(0xBEEF, p.related_shard());
    }

    #[test]
    fn related_shard_of_asset_transfer_input() {
        let mut asset_type = H256::new();
        asset_type[2..4].clone_from_slice(&[0xBE, 0xEF]);

        let prev_out = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 3,
            asset_type,
            amount: 34,
        };

        let input = AssetTransferInput {
            prev_out,
            lock_script: vec![],
            unlock_script: vec![],
        };

        assert_eq!(0xBEEF, input.related_shard());
    }

    #[test]
    fn _is_input_and_output_consistent() {
        let asset_type = H256::random();
        let amount = 100;

        assert!(is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount,
            }]
        ));
    }

    #[test]
    fn multiple_asset_is_input_and_output_consistent() {
        let asset_type1 = H256::random();
        let asset_type2 = {
            let mut asset_type = H256::random();
            while asset_type == asset_type1 {
                asset_type = H256::random();
            }
            asset_type
        };
        let amount1 = 100;
        let amount2 = 200;

        assert!(is_input_and_output_consistent(
            &[
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        amount: amount1,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type2,
                        amount: amount2,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
            ],
            &[
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type1,
                    amount: amount1,
                },
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type2,
                    amount: amount2,
                },
            ]
        ));
    }

    #[test]
    fn multiple_asset_different_order_is_input_and_output_consistent() {
        let asset_type1 = H256::random();
        let asset_type2 = {
            let mut asset_type = H256::random();
            while asset_type == asset_type1 {
                asset_type = H256::random();
            }
            asset_type
        };
        let amount1 = 100;
        let amount2 = 200;

        assert!(is_input_and_output_consistent(
            &[
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        amount: amount1,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type2,
                        amount: amount2,
                    },
                    lock_script: vec![],
                    unlock_script: vec![],
                },
            ],
            &[
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type2,
                    amount: amount2,
                },
                AssetTransferOutput {
                    lock_script_hash: H256::random(),
                    parameters: vec![],
                    asset_type: asset_type1,
                    amount: amount1,
                },
            ]
        ));
    }

    #[test]
    fn empty_is_input_and_output_consistent() {
        assert!(is_input_and_output_consistent(&[], &[]));
    }

    #[test]
    fn fail_if_output_has_more_asset() {
        let asset_type = H256::random();
        let output_amount = 100;
        assert!(!is_input_and_output_consistent(
            &[],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount: output_amount,
            }]
        ));
    }

    #[test]
    fn fail_if_input_has_more_asset() {
        let asset_type = H256::random();
        let input_amount = 100;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[]
        ));
    }

    #[test]
    fn fail_if_input_is_larger_than_output() {
        let asset_type = H256::random();
        let input_amount = 100;
        let output_amount = 80;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount: output_amount,
            }]
        ));
    }

    #[test]
    fn fail_if_input_is_smaller_than_output() {
        let asset_type = H256::random();
        let input_amount = 80;
        let output_amount = 100;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H256::random(),
                parameters: vec![],
                asset_type,
                amount: output_amount,
            }]
        ));
    }

    #[test]
    fn encode_and_decode_create_world_without_owners() {
        let transaction = Transaction::CreateWorld {
            network_id: "tc".into(),
            shard_id: 0xFE,
            nonce: 0xFE,
            owners: vec![],
        };
        rlp_encode_and_decode_test!(transaction);
    }

    #[test]
    fn encode_and_decode_create_world_with_owners() {
        let transaction = Transaction::CreateWorld {
            network_id: "tc".into(),
            shard_id: 0xFE,
            nonce: 0xFE,
            owners: vec![Address::random(), Address::random(), Address::random()],
        };
        rlp_encode_and_decode_test!(transaction);
    }

    #[test]
    fn encode_and_decode_set_world_owners_with_empty_owners() {
        let transaction = Transaction::SetWorldOwners {
            network_id: "tc".into(),
            shard_id: 0xFE,
            world_id: 0xB,
            nonce: 0xEE,
            owners: vec![],
        };
        rlp_encode_and_decode_test!(transaction);
    }

    #[test]
    fn encode_and_decode_set_world_owners() {
        let transaction = Transaction::SetWorldOwners {
            network_id: "tc".into(),
            shard_id: 0xFE,
            world_id: 0xB,
            nonce: 0xEE,
            owners: vec![Address::random(), Address::random(), Address::random()],
        };
        rlp_encode_and_decode_test!(transaction);
    }
}
