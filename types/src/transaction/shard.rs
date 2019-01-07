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

use ccrypto::{blake128, blake256, blake256_with_key};
use ckey::{Address, NetworkId};
use heapsize::HeapSizeOf;
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::{
    AssetMintOutput, AssetOutPoint, AssetTransferInput, AssetTransferOutput, HashingError, Order, OrderOnTransfer,
    PartialHashing,
};
use crate::util::tag::Tag;
use crate::ShardId;


/// Shard Transaction type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardTransaction {
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,

        output: AssetMintOutput,
    },
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,
    },
    ChangeAssetScheme {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
    },
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        inputs: Vec<AssetTransferInput>,
        output: AssetMintOutput,
    },
    DecomposeAsset {
        network_id: NetworkId,
        input: AssetTransferInput,
        outputs: Vec<AssetTransferOutput>,
    },
    UnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
    },
    WrapCCC {
        network_id: NetworkId,
        shard_id: ShardId,
        tx_hash: H256,
        output: AssetWrapCCCOutput,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetWrapCCCOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub amount: u64,
}

impl ShardTransaction {
    pub fn tracker(&self) -> H256 {
        if let ShardTransaction::WrapCCC {
            tx_hash,
            ..
        } = self
        {
            return *tx_hash
        }
        blake256(&*self.rlp_bytes())
    }

    pub fn network_id(&self) -> NetworkId {
        match self {
            ShardTransaction::TransferAsset {
                network_id,
                ..
            } => *network_id,
            ShardTransaction::MintAsset {
                network_id,
                ..
            } => *network_id,
            ShardTransaction::ComposeAsset {
                network_id,
                ..
            } => *network_id,
            ShardTransaction::ChangeAssetScheme {
                network_id,
                ..
            } => *network_id,
            ShardTransaction::DecomposeAsset {
                network_id,
                ..
            } => *network_id,
            ShardTransaction::UnwrapCCC {
                network_id,
                ..
            } => *network_id,
            ShardTransaction::WrapCCC {
                network_id,
                ..
            } => *network_id,
        }
    }

    pub fn related_shards(&self) -> Vec<ShardId> {
        match self {
            ShardTransaction::TransferAsset {
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
            ShardTransaction::MintAsset {
                shard_id,
                ..
            } => vec![*shard_id],
            ShardTransaction::ChangeAssetScheme {
                asset_type,
                ..
            } => vec![(ShardId::from(asset_type[2]) << 8) + ShardId::from(asset_type[3])],
            ShardTransaction::ComposeAsset {
                inputs,
                shard_id,
                ..
            } => {
                let mut shards: Vec<ShardId> = inputs.iter().map(AssetTransferInput::related_shard).collect();
                shards.push(*shard_id);
                shards.sort_unstable();
                shards.dedup();
                shards
            }
            ShardTransaction::DecomposeAsset {
                outputs,
                ..
            } => {
                let mut shards: Vec<ShardId> = outputs.iter().map(AssetTransferOutput::related_shard).collect();
                shards.sort_unstable();
                shards.dedup();
                shards
            }
            ShardTransaction::UnwrapCCC {
                burn,
                ..
            } => vec![burn.related_shard()],
            ShardTransaction::WrapCCC {
                shard_id,
                ..
            } => vec![*shard_id],
        }
    }

    pub fn unwrapped_amount(&self) -> u64 {
        match self {
            ShardTransaction::UnwrapCCC {
                burn,
                ..
            } => burn.prev_out.amount,
            _ => 0,
        }
    }

    fn is_valid_output_index(&self, index: usize) -> bool {
        match self {
            ShardTransaction::MintAsset {
                ..
            } => index == 0,
            ShardTransaction::TransferAsset {
                outputs,
                ..
            } => index < outputs.len(),
            ShardTransaction::ChangeAssetScheme {
                ..
            } => false,
            ShardTransaction::ComposeAsset {
                ..
            } => index == 0,
            ShardTransaction::DecomposeAsset {
                outputs,
                ..
            } => index < outputs.len(),
            ShardTransaction::UnwrapCCC {
                ..
            } => false,
            ShardTransaction::WrapCCC {
                ..
            } => index == 0,
        }
    }

    pub fn is_valid_shard_id_index(&self, index: usize, id: ShardId) -> bool {
        if !self.is_valid_output_index(index) {
            return false
        }
        match self {
            ShardTransaction::MintAsset {
                shard_id,
                ..
            } => &id == shard_id,
            ShardTransaction::TransferAsset {
                outputs,
                ..
            } => id == outputs[index].related_shard(),
            ShardTransaction::ChangeAssetScheme {
                ..
            } => unreachable!("AssetSchemeChange doesn't have a valid index"),
            ShardTransaction::ComposeAsset {
                shard_id,
                ..
            } => &id == shard_id,
            ShardTransaction::DecomposeAsset {
                outputs,
                ..
            } => id == outputs[index].related_shard(),
            ShardTransaction::UnwrapCCC {
                ..
            } => unreachable!("UnwrapCCC doesn't have a valid index"),
            ShardTransaction::WrapCCC {
                shard_id,
                ..
            } => &id == shard_id,
        }
    }
}

impl HeapSizeOf for AssetOutPoint {
    fn heap_size_of_children(&self) -> usize {
        0
    }
}

impl HeapSizeOf for AssetMintOutput {
    fn heap_size_of_children(&self) -> usize {
        self.parameters.heap_size_of_children() + self.amount.heap_size_of_children()
    }
}

impl HeapSizeOf for ShardTransaction {
    fn heap_size_of_children(&self) -> usize {
        match self {
            ShardTransaction::MintAsset {
                metadata,
                approver,
                output,
                ..
            } => metadata.heap_size_of_children() + approver.heap_size_of_children() + output.heap_size_of_children(),
            ShardTransaction::TransferAsset {
                burns,
                inputs,
                outputs,
                orders,
                ..
            } => {
                burns.heap_size_of_children()
                    + inputs.heap_size_of_children()
                    + outputs.heap_size_of_children()
                    + orders.heap_size_of_children()
            }
            ShardTransaction::ChangeAssetScheme {
                metadata,
                ..
            } => metadata.heap_size_of_children(),
            ShardTransaction::ComposeAsset {
                metadata,
                approver,
                inputs,
                output,
                ..
            } => {
                metadata.heap_size_of_children()
                    + approver.heap_size_of_children()
                    + inputs.heap_size_of_children()
                    + output.heap_size_of_children()
            }
            ShardTransaction::DecomposeAsset {
                input,
                outputs,
                ..
            } => input.heap_size_of_children() + outputs.heap_size_of_children(),
            ShardTransaction::UnwrapCCC {
                burn,
                ..
            } => burn.heap_size_of_children(),
            ShardTransaction::WrapCCC {
                output,
                ..
            } => output.heap_size_of_children(),
        }
    }
}

fn apply_bitmask_to_output(
    mut bitmask: Vec<u8>,
    outputs: &[AssetTransferOutput],
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

            filter >>= 1;
        }
        index += 1;
    }
    Ok(result)
}

fn apply_input_scheme(
    inputs: &[AssetTransferInput],
    is_sign_all: bool,
    is_sign_single: bool,
    cur: &AssetTransferInput,
) -> Vec<AssetTransferInput> {
    if is_sign_all {
        return inputs
            .iter()
            .map(|input| AssetTransferInput {
                prev_out: input.prev_out.clone(),
                timelock: input.timelock,
                lock_script: Vec::new(),
                unlock_script: Vec::new(),
            })
            .collect()
    }

    if is_sign_single {
        return vec![AssetTransferInput {
            prev_out: cur.prev_out.clone(),
            timelock: cur.timelock,
            lock_script: Vec::new(),
            unlock_script: Vec::new(),
        }]
    }

    Vec::new()
}

impl PartialHashing for ShardTransaction {
    fn hash_partially(&self, tag: Tag, cur: &AssetTransferInput, is_burn: bool) -> Result<H256, HashingError> {
        match self {
            ShardTransaction::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
            } => {
                if !orders.is_empty() && (!tag.sign_all_inputs || !tag.sign_all_outputs) {
                    return Err(HashingError::InvalidFilter)
                }

                let new_burns = apply_input_scheme(burns, tag.sign_all_inputs, is_burn, cur);
                let new_inputs = apply_input_scheme(inputs, tag.sign_all_inputs, !is_burn, cur);

                let new_outputs = if tag.sign_all_outputs {
                    outputs.clone()
                } else {
                    apply_bitmask_to_output(tag.filter.clone(), &outputs, Vec::new())?
                };

                Ok(blake256_with_key(
                    &ShardTransaction::TransferAsset {
                        network_id: *network_id,
                        burns: new_burns,
                        inputs: new_inputs,
                        outputs: new_outputs,
                        orders: orders.to_vec(),
                    }
                    .rlp_bytes(),
                    &blake128(tag.get_tag()),
                ))
            }
            ShardTransaction::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
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
                    &ShardTransaction::ComposeAsset {
                        network_id: *network_id,
                        shard_id: *shard_id,
                        metadata: metadata.to_string(),
                        approver: *approver,
                        administrator: *administrator,
                        inputs: new_inputs,
                        output: new_output,
                    }
                    .rlp_bytes(),
                    &blake128(tag.get_tag()),
                ))
            }
            ShardTransaction::DecomposeAsset {
                network_id,
                input,
                outputs,
            } => {
                let new_outputs = if tag.sign_all_outputs {
                    outputs.clone()
                } else {
                    apply_bitmask_to_output(tag.filter.clone(), &outputs, Vec::new())?
                };

                Ok(blake256_with_key(
                    &ShardTransaction::DecomposeAsset {
                        network_id: *network_id,
                        input: AssetTransferInput {
                            prev_out: input.prev_out.clone(),
                            timelock: input.timelock,
                            lock_script: Vec::new(),
                            unlock_script: Vec::new(),
                        },
                        outputs: new_outputs,
                    }
                    .rlp_bytes(),
                    &blake128(tag.get_tag()),
                ))
            }
            ShardTransaction::UnwrapCCC {
                network_id,
                burn,
            } => {
                if !tag.sign_all_inputs || !tag.sign_all_outputs {
                    return Err(HashingError::InvalidFilter)
                }

                Ok(blake256_with_key(
                    &ShardTransaction::UnwrapCCC {
                        network_id: *network_id,
                        burn: AssetTransferInput {
                            prev_out: burn.prev_out.clone(),
                            timelock: burn.timelock,
                            lock_script: Vec::new(),
                            unlock_script: Vec::new(),
                        },
                    }
                    .rlp_bytes(),
                    &blake128(tag.get_tag()),
                ))
            }
            _ => unreachable!(),
        }
    }
}

impl Order {
    pub fn hash(&self) -> H256 {
        blake256(&self.rlp_bytes())
    }
}

impl PartialHashing for Order {
    fn hash_partially(&self, tag: Tag, _cur: &AssetTransferInput, is_burn: bool) -> Result<H256, HashingError> {
        assert!(tag.sign_all_inputs);
        assert!(tag.sign_all_outputs);
        assert!(!is_burn);
        Ok(self.hash())
    }
}

type TransactionId = u8;
const ASSET_UNWRAP_CCC_ID: TransactionId = 0x11;
const ASSET_MINT_ID: TransactionId = 0x13;
const ASSET_TRANSFER_ID: TransactionId = 0x14;
const ASSET_SCHEME_CHANGE_ID: TransactionId = 0x15;
const ASSET_COMPOSE_ID: TransactionId = 0x16;
const ASSET_DECOMPOSE_ID: TransactionId = 0x17;

impl Decodable for ShardTransaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        match d.val_at(0)? {
            ASSET_MINT_ID => {
                if d.item_count()? != 9 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::MintAsset {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    metadata: d.val_at(3)?,
                    output: AssetMintOutput {
                        lock_script_hash: d.val_at(4)?,
                        parameters: d.val_at(5)?,
                        amount: d.val_at(6)?,
                    },
                    approver: d.val_at(7)?,
                    administrator: d.val_at(8)?,
                })
            }
            ASSET_TRANSFER_ID => {
                if d.item_count()? != 6 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::TransferAsset {
                    network_id: d.val_at(1)?,
                    burns: d.list_at(2)?,
                    inputs: d.list_at(3)?,
                    outputs: d.list_at(4)?,
                    orders: d.list_at(5)?,
                })
            }
            ASSET_SCHEME_CHANGE_ID => {
                if d.item_count()? != 6 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::ChangeAssetScheme {
                    network_id: d.val_at(1)?,
                    asset_type: d.val_at(2)?,
                    metadata: d.val_at(3)?,
                    approver: d.val_at(4)?,
                    administrator: d.val_at(5)?,
                })
            }
            ASSET_COMPOSE_ID => {
                if d.item_count()? != 10 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::ComposeAsset {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    metadata: d.val_at(3)?,
                    approver: d.val_at(4)?,
                    administrator: d.val_at(5)?,
                    inputs: d.list_at(6)?,
                    output: AssetMintOutput {
                        lock_script_hash: d.val_at(7)?,
                        parameters: d.val_at(8)?,
                        amount: d.val_at(9)?,
                    },
                })
            }
            ASSET_DECOMPOSE_ID => {
                if d.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::DecomposeAsset {
                    network_id: d.val_at(1)?,
                    input: d.val_at(2)?,
                    outputs: d.list_at(3)?,
                })
            }
            ASSET_UNWRAP_CCC_ID => {
                if d.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::UnwrapCCC {
                    network_id: d.val_at(1)?,
                    burn: d.val_at(2)?,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected transaction")),
        }
    }
}

impl Encodable for ShardTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            ShardTransaction::MintAsset {
                network_id,
                shard_id,
                metadata,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        parameters,
                        amount,
                    },
                approver,
                administrator,
            } => {
                s.begin_list(9)
                    .append(&ASSET_MINT_ID)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(lock_script_hash)
                    .append(parameters)
                    .append(amount)
                    .append(approver)
                    .append(administrator);
            }
            ShardTransaction::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
            } => {
                s.begin_list(6)
                    .append(&ASSET_TRANSFER_ID)
                    .append(network_id)
                    .append_list(burns)
                    .append_list(inputs)
                    .append_list(outputs)
                    .append_list(orders);
            }
            ShardTransaction::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
            } => {
                s.begin_list(6)
                    .append(&ASSET_SCHEME_CHANGE_ID)
                    .append(network_id)
                    .append(asset_type)
                    .append(metadata)
                    .append(approver)
                    .append(administrator);
            }
            ShardTransaction::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                inputs,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        parameters,
                        amount,
                    },
            } => {
                s.begin_list(10)
                    .append(&ASSET_COMPOSE_ID)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(approver)
                    .append(administrator)
                    .append_list(inputs)
                    .append(lock_script_hash)
                    .append(parameters)
                    .append(amount);
            }
            ShardTransaction::DecomposeAsset {
                network_id,
                input,
                outputs,
            } => {
                s.begin_list(4).append(&ASSET_DECOMPOSE_ID).append(network_id).append(input).append_list(outputs);
            }
            ShardTransaction::UnwrapCCC {
                network_id,
                burn,
            } => {
                s.begin_list(3).append(&ASSET_UNWRAP_CCC_ID).append(network_id).append(burn);
            }
            ShardTransaction::WrapCCC {
                ..
            } => {
                unreachable!("No reason to get a RLP encoding of WrapCCC");
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn related_shard_of_asset_transfer_input() {
        let mut asset_type = H256::new();
        asset_type[2..4].clone_from_slice(&[0xBE, 0xEF]);

        let prev_out = AssetOutPoint {
            tracker: H256::random(),
            index: 3,
            asset_type,
            amount: 34,
        };

        let input = AssetTransferInput {
            prev_out,
            timelock: None,
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
                    tracker: H256::random(),
                    index: 0,
                    asset_type,
                    amount,
                },
                timelock: None,
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
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        amount: amount1,
                    },
                    timelock: None,
                    lock_script: vec![],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type2,
                        amount: amount2,
                    },
                    timelock: None,
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
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        amount: amount1,
                    },
                    timelock: None,
                    lock_script: vec![],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type2,
                        amount: amount2,
                    },
                    timelock: None,
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
                    tracker: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                timelock: None,
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
                    tracker: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                timelock: None,
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
                    tracker: H256::random(),
                    index: 0,
                    asset_type,
                    amount: input_amount,
                },
                timelock: None,
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
    fn encode_and_decode_decompose_transaction() {
        let tx = ShardTransaction::DecomposeAsset {
            network_id: NetworkId::default(),
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: Default::default(),
                    index: 0,
                    asset_type: H256::default(),
                    amount: 30,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            outputs: Vec::new(),
        };
        rlp_encode_and_decode_test!(tx);
    }

    #[test]
    fn encode_and_decode_unwrapccc_transaction() {
        let tx = ShardTransaction::UnwrapCCC {
            network_id: NetworkId::default(),
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: Default::default(),
                    index: 0,
                    asset_type: H256::zero(),
                    amount: 30,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
        };
        rlp_encode_and_decode_test!(tx);
    }

    #[test]
    fn encode_and_decode_transfer_transaction_with_order() {
        let tx = ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: H256::random(),
                    index: 0,
                    asset_type: H256::random(),
                    amount: 30,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash: H160::random(),
                parameters: vec![vec![1]],
                asset_type: H256::random(),
                amount: 30,
            }],
            orders: vec![OrderOnTransfer {
                order: Order {
                    asset_type_from: H256::random(),
                    asset_type_to: H256::random(),
                    asset_type_fee: H256::random(),
                    asset_amount_from: 10,
                    asset_amount_to: 10,
                    asset_amount_fee: 0,
                    origin_outputs: vec![AssetOutPoint {
                        tracker: H256::random(),
                        index: 0,
                        asset_type: H256::random(),
                        amount: 30,
                    }],
                    expiration: 10,
                    lock_script_hash_from: H160::random(),
                    parameters_from: vec![vec![1]],
                    lock_script_hash_fee: H160::random(),
                    parameters_fee: vec![vec![1]],
                },
                spent_amount: 10,
                input_indices: vec![0],
                output_indices: vec![0],
            }],
        };
        rlp_encode_and_decode_test!(tx);
    }

    #[test]
    fn apply_long_filter() {
        let input = AssetTransferInput {
            prev_out: AssetOutPoint {
                tracker: Default::default(),
                index: 0,
                asset_type: H256::default(),
                amount: 0,
            },
            timelock: None,
            lock_script: Vec::new(),
            unlock_script: Vec::new(),
        };
        let inputs: Vec<AssetTransferInput> = (0..100)
            .map(|_| AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: Default::default(),
                    index: 0,
                    asset_type: H256::default(),
                    amount: 0,
                },
                timelock: None,
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

        let transaction = ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: inputs.clone(),
            outputs: outputs.clone(),
            orders: vec![],
        };
        let mut tag: Vec<u8> = vec![0b0000_1111 as u8];
        for _i in 0..12 {
            tag.push(0b1111_1111 as u8);
        }
        tag.push(0b0011_0101);
        assert_eq!(
            transaction.hash_partially(Tag::try_new(tag.clone()).unwrap(), &input, false),
            Ok(blake256_with_key(&transaction.rlp_bytes(), &blake128(&tag)))
        );

        // Sign except for last element
        outputs.pop();
        let transaction_aux = ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs: inputs.clone(),
            outputs: outputs.clone(),
            orders: vec![],
        };
        tag = vec![0b0000_0111 as u8];
        for _i in 0..12 {
            tag.push(0b1111_1111 as u8);
        }
        tag.push(0b0011_0101);
        assert_eq!(
            transaction.hash_partially(Tag::try_new(tag.clone()).unwrap(), &input, false),
            Ok(blake256_with_key(&transaction_aux.rlp_bytes(), &blake128(&tag)))
        );

        // Sign except for last two elements
        outputs.pop();
        let transaction_aux = ShardTransaction::TransferAsset {
            network_id: NetworkId::default(),
            burns: Vec::new(),
            inputs,
            outputs,
            orders: vec![],
        };
        tag = vec![0b0000_0011 as u8];
        for _i in 0..12 {
            tag.push(0b1111_1111 as u8);
        }
        tag.push(0b0011_0101);
        assert_eq!(
            transaction.hash_partially(Tag::try_new(tag.clone()).unwrap(), &input, false),
            Ok(blake256_with_key(&transaction_aux.rlp_bytes(), &blake128(&tag)))
        );
    }

    // FIXME: Remove it and reuse the same function declared in action.rs
    fn is_input_and_output_consistent(inputs: &[AssetTransferInput], outputs: &[AssetTransferOutput]) -> bool {
        let mut sum: HashMap<H256, u128> = HashMap::new();

        for input in inputs {
            let asset_type = input.prev_out.asset_type;
            let amount = u128::from(input.prev_out.amount);
            let current_amount = sum.get(&asset_type).cloned().unwrap_or_default();
            sum.insert(asset_type, current_amount + amount);
        }
        for output in outputs {
            let asset_type = output.asset_type;
            let amount = u128::from(output.amount);
            let current_amount = if let Some(current_amount) = sum.get(&asset_type) {
                if *current_amount < amount {
                    return false
                }
                *current_amount
            } else {
                return false
            };
            let t = sum.insert(asset_type, current_amount - amount);
            debug_assert!(t.is_some());
        }

        sum.iter().all(|(_, sum)| *sum == 0)
    }
}
