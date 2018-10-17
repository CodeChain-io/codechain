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
use ccrypto::{blake128, blake256, blake256_with_key};
use ckey::{Address, NetworkId};
use heapsize::HeapSizeOf;
use primitives::{Bytes, H160, H256, U128};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::util::tag::Tag;
use super::super::{ShardId, WorldId};
use super::error::Error;


pub trait PartialHashing {
    fn hash_partially(&self, tag: Tag, cur: &AssetOutPoint, burn: bool) -> Result<H256, HashingError>;
}

#[derive(Debug, PartialEq)]
pub enum HashingError {
    InvalidFilter,
}

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
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub asset_type: H256,
    pub amount: u64,
}

/// Parcel transaction type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transaction {
    CreateWorld {
        network_id: NetworkId,
        shard_id: ShardId,
        nonce: u64,
        owners: Vec<Address>,
    },
    SetWorldOwners {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        nonce: u64,
        owners: Vec<Address>,
    },
    SetWorldUsers {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        nonce: u64,
        users: Vec<Address>,
    },
    AssetMint {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        metadata: String,
        registrar: Option<Address>,
        nonce: u64,

        output: AssetMintOutput,
    },
    AssetTransfer {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        nonce: u64,
    },
    AssetCompose {
        network_id: NetworkId,
        shard_id: ShardId,
        world_id: WorldId,
        nonce: u64,
        metadata: String,
        registrar: Option<Address>,
        inputs: Vec<AssetTransferInput>,
        output: AssetMintOutput,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMintOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub amount: Option<u64>,
}

impl Transaction {
    pub fn hash(&self) -> H256 {
        blake256(&*self.rlp_bytes())
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
            Transaction::AssetCompose {
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
            Transaction::AssetCompose {
                inputs,
                shard_id,
                ..
            } => {
                let mut shards: Vec<ShardId> = inputs.iter().map(AssetTransferInput::related_shard).collect();
                shards.push(shard_id.clone());
                shards.sort_unstable();
                shards.dedup();
                shards
            }
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
                burns,
                inputs,
                outputs,
                ..
            } => {
                if outputs.len() > 512 {
                    return Err(Error::TooManyOutputs(outputs.len()))
                }
                // FIXME: check burns
                if !is_input_and_output_consistent(inputs, outputs) {
                    return Err(Error::InconsistentTransactionInOut)
                }
                for burn in burns {
                    if burn.prev_out.amount == 0 {
                        return Err(Error::ZeroAmount)
                    }
                }
                for input in inputs {
                    if input.prev_out.amount == 0 {
                        return Err(Error::ZeroAmount)
                    }
                }
                for output in outputs {
                    if output.amount == 0 {
                        return Err(Error::ZeroAmount)
                    }
                }
                Ok(())
            }
            Transaction::AssetMint {
                output,
                ..
            } => match output.amount {
                Some(amount) if amount == 0 => Err(Error::ZeroAmount),
                _ => Ok(()),
            },
            Transaction::AssetCompose {
                inputs,
                output,
                ..
            } => {
                if inputs.is_empty() {
                    return Err(Error::EmptyInput)
                }
                for input in inputs {
                    if input.prev_out.amount == 0 {
                        return Err(Error::ZeroAmount)
                    }
                }
                match output.amount {
                    Some(amount) if amount == 0 => Err(Error::ZeroAmount),
                    _ => Ok(()),
                }
            }
        }
    }
}

impl HeapSizeOf for AssetTransferInput {
    fn heap_size_of_children(&self) -> usize {
        self.lock_script.heap_size_of_children() + self.unlock_script.heap_size_of_children()
    }
}

impl HeapSizeOf for AssetTransferOutput {
    fn heap_size_of_children(&self) -> usize {
        self.parameters.heap_size_of_children()
    }
}

impl HeapSizeOf for AssetMintOutput {
    fn heap_size_of_children(&self) -> usize {
        self.parameters.heap_size_of_children() + self.amount.heap_size_of_children()
    }
}

impl HeapSizeOf for Transaction {
    fn heap_size_of_children(&self) -> usize {
        match self {
            Transaction::CreateWorld {
                network_id: _,
                shard_id: _,
                nonce: _,
                owners,
            } => owners.heap_size_of_children(),
            Transaction::SetWorldOwners {
                network_id: _,
                shard_id: _,
                world_id: _,
                nonce: _,
                owners,
            } => owners.heap_size_of_children(),
            Transaction::SetWorldUsers {
                network_id: _,
                shard_id: _,
                world_id: _,
                nonce: _,
                users,
            } => users.heap_size_of_children(),
            Transaction::AssetMint {
                network_id: _,
                shard_id: _,
                world_id: _,
                metadata,
                registrar,
                nonce: _,
                output,
            } => metadata.heap_size_of_children() + registrar.heap_size_of_children() + output.heap_size_of_children(),
            Transaction::AssetTransfer {
                network_id: _,
                burns,
                inputs,
                outputs,
                nonce: _,
            } => burns.heap_size_of_children() + inputs.heap_size_of_children() + outputs.heap_size_of_children(),
            Transaction::AssetCompose {
                network_id: _,
                shard_id: _,
                world_id: _,
                nonce: _,
                metadata,
                registrar,
                inputs,
                output,
            } => {
                metadata.heap_size_of_children()
                    + registrar.heap_size_of_children()
                    + inputs.heap_size_of_children()
                    + output.heap_size_of_children()
            }
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

fn apply_bitmask_to_output(
    mut bitmask: Vec<u8>,
    outputs: Vec<AssetTransferOutput>,
    mut result: Vec<AssetTransferOutput>,
) -> Result<Vec<AssetTransferOutput>, HashingError> {
    let mut index = 0;
    let output_len = outputs.len();

    while let Some(e) = bitmask.pop() {
        let mut filter = e;
        for i in 0..8 {
            if (8 * index + i) == output_len as usize {
                return Ok(result)
            }

            if (filter & 0x1) == 1 {
                result.push(outputs[8 * index + i].clone());
            }

            filter = filter >> 1;
        }
        index += 1;
    }
    return Ok(result)
}

fn apply_input_scheme(
    inputs: &Vec<AssetTransferInput>,
    is_sign_all: bool,
    is_sign_single: bool,
    cur: &AssetOutPoint,
) -> Vec<AssetTransferInput> {
    if is_sign_all {
        return inputs
            .iter()
            .map(|input| AssetTransferInput {
                prev_out: input.prev_out.clone(),
                lock_script: Vec::new(),
                unlock_script: Vec::new(),
            })
            .collect()
    }

    if is_sign_single {
        return vec![AssetTransferInput {
            prev_out: cur.clone(),
            lock_script: Vec::new(),
            unlock_script: Vec::new(),
        }]
    }

    Vec::new()
}

impl PartialHashing for Transaction {
    fn hash_partially(&self, tag: Tag, cur: &AssetOutPoint, is_burn: bool) -> Result<H256, HashingError> {
        match self {
            Transaction::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                nonce,
            } => {
                let new_burns = apply_input_scheme(burns, tag.sign_all_inputs, is_burn, cur);
                let new_inputs = apply_input_scheme(inputs, tag.sign_all_inputs, !is_burn, cur);

                let new_outputs = if tag.sign_all_outputs {
                    outputs.clone()
                } else {
                    apply_bitmask_to_output(tag.filter.clone(), outputs.to_vec(), Vec::new())?
                };

                Ok(blake256_with_key(
                    &Transaction::AssetTransfer {
                        network_id: *network_id,
                        burns: new_burns,
                        inputs: new_inputs,
                        outputs: new_outputs,
                        nonce: *nonce,
                    }.rlp_bytes(),
                    &blake128(tag.get_tag()),
                ))
            }
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
                if tag.filter_len != 0 {
                    return Err(HashingError::InvalidFilter)
                }

                let new_inputs = apply_input_scheme(inputs, tag.sign_all_inputs, !is_burn, cur);

                let new_output = if tag.sign_all_outputs {
                    output.clone()
                } else {
                    AssetMintOutput {
                        lock_script_hash: H160::default(),
                        parameters: Vec::new(),
                        amount: None,
                    }
                };

                Ok(blake256_with_key(
                    &Transaction::AssetCompose {
                        network_id: *network_id,
                        shard_id: *shard_id,
                        world_id: *world_id,
                        nonce: *nonce,
                        metadata: metadata.to_string(),
                        registrar: *registrar,
                        inputs: new_inputs,
                        output: new_output,
                    }.rlp_bytes(),
                    &blake128(tag.get_tag()),
                ))
            }
            _ => unreachable!(),
        }
    }
}

type TransactionId = u8;
const CREATE_WORLD_ID: TransactionId = 0x01;
const SET_WORLD_OWNERS_ID: TransactionId = 0x02;
const ASSET_MINT_ID: TransactionId = 0x03;
const ASSET_TRANSFER_ID: TransactionId = 0x04;
const SET_WORLD_USERS_ID: TransactionId = 0x05;
const ASSET_COMPOSE_ID: TransactionId = 0x06;

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
            ASSET_COMPOSE_ID => {
                if d.item_count()? != 11 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetCompose {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    world_id: d.val_at(3)?,
                    metadata: d.val_at(4)?,
                    registrar: d.val_at(5)?,
                    inputs: d.list_at(6)?,
                    output: AssetMintOutput {
                        lock_script_hash: d.val_at(7)?,
                        parameters: d.val_at(8)?,
                        amount: d.val_at(9)?,
                    },
                    nonce: d.val_at(10)?,
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
            Transaction::AssetCompose {
                network_id,
                shard_id,
                world_id,
                nonce,
                metadata,
                registrar,
                inputs,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        parameters,
                        amount,
                    },
            } => s
                .begin_list(11)
                .append(&ASSET_COMPOSE_ID)
                .append(network_id)
                .append(shard_id)
                .append(world_id)
                .append(metadata)
                .append(registrar)
                .append_list(inputs)
                .append(lock_script_hash)
                .append(parameters)
                .append(amount)
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
                lock_script_hash: H160::random(),
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
                    lock_script_hash: H160::random(),
                    parameters: vec![],
                    asset_type: asset_type1,
                    amount: amount1,
                },
                AssetTransferOutput {
                    lock_script_hash: H160::random(),
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
                    lock_script_hash: H160::random(),
                    parameters: vec![],
                    asset_type: asset_type2,
                    amount: amount2,
                },
                AssetTransferOutput {
                    lock_script_hash: H160::random(),
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
                lock_script_hash: H160::random(),
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
                lock_script_hash: H160::random(),
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
                lock_script_hash: H160::random(),
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

    #[test]
    fn apply_long_filter() {
        let out = AssetOutPoint {
            transaction_hash: H256::default(),
            index: 0,
            asset_type: H256::default(),
            amount: 0,
        };
        let inputs: Vec<AssetTransferInput> = (0..100)
            .map(|_| AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::default(),
                    index: 0,
                    asset_type: H256::default(),
                    amount: 0,
                },
                lock_script: Vec::new(),
                unlock_script: Vec::new(),
            })
            .collect();
        let mut outputs: Vec<AssetTransferOutput> = (0..100)
            .map(|_| AssetTransferOutput {
                lock_script_hash: H160::default(),
                parameters: Vec::new(),
                asset_type: H256::default(),
                amount: 0,
            })
            .collect();

        let transaction = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: inputs.clone(),
            outputs: outputs.clone(),
            nonce: 0,
        };
        let mut tag: Vec<u8> = vec![0b00001111 as u8];
        for _i in 0..12 {
            tag.push(0b11111111 as u8);
        }
        tag.push(0b00110101);
        assert_eq!(
            transaction.hash_partially(Tag::try_new(tag.clone()).unwrap(), &out, false),
            Ok(blake256_with_key(&transaction.rlp_bytes(), &blake128(&tag)))
        );

        // Sign except for last element
        outputs.pop();
        let transaction_aux = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: inputs.clone(),
            outputs: outputs.clone(),
            nonce: 0,
        };
        tag = vec![0b00000111 as u8];
        for _i in 0..12 {
            tag.push(0b11111111 as u8);
        }
        tag.push(0b00110101);
        assert_eq!(
            transaction.hash_partially(Tag::try_new(tag.clone()).unwrap(), &out, false),
            Ok(blake256_with_key(&transaction_aux.rlp_bytes(), &blake128(&tag)))
        );

        // Sign except for last two elements
        outputs.pop();
        let transaction_aux = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs,
            outputs,
            nonce: 0,
        };
        tag = vec![0b00000011 as u8];
        for _i in 0..12 {
            tag.push(0b11111111 as u8);
        }
        tag.push(0b00110101);
        assert_eq!(
            transaction.hash_partially(Tag::try_new(tag.clone()).unwrap(), &out, false),
            Ok(blake256_with_key(&transaction_aux.rlp_bytes(), &blake128(&tag)))
        );
    }
}
