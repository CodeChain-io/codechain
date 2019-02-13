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

use ccrypto::{blake128, blake256, blake256_with_key};
use ckey::{Address, NetworkId};
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::{
    AssetMintOutput, AssetTransferInput, AssetTransferOutput, HashingError, Order, OrderOnTransfer, PartialHashing,
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
        allowed_script_hashes: Vec<H160>,
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
        shard_id: ShardId,
        asset_type: H160,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        allowed_script_hashes: Vec<H160>,
    },
    IncreaseAssetSupply {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        output: AssetMintOutput,
    },
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        allowed_script_hashes: Vec<H160>,
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
    pub quantity: u64,
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
            }
            | ShardTransaction::MintAsset {
                network_id,
                ..
            }
            | ShardTransaction::IncreaseAssetSupply {
                network_id,
                ..
            }
            | ShardTransaction::ComposeAsset {
                network_id,
                ..
            }
            | ShardTransaction::ChangeAssetScheme {
                network_id,
                ..
            }
            | ShardTransaction::DecomposeAsset {
                network_id,
                ..
            }
            | ShardTransaction::UnwrapCCC {
                network_id,
                ..
            }
            | ShardTransaction::WrapCCC {
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
                    .map(|v| v.prev_out.shard_id)
                    .chain(inputs.iter().map(|v| v.prev_out.shard_id))
                    .collect();
                shards.sort_unstable();
                shards.dedup();
                shards
            }
            ShardTransaction::MintAsset {
                shard_id,
                ..
            } => vec![*shard_id],
            ShardTransaction::IncreaseAssetSupply {
                shard_id,
                ..
            } => vec![*shard_id],
            ShardTransaction::ChangeAssetScheme {
                shard_id,
                ..
            } => vec![*shard_id],
            ShardTransaction::ComposeAsset {
                inputs,
                shard_id,
                ..
            } => {
                let mut shards: Vec<ShardId> = inputs.iter().map(|v| v.prev_out.shard_id).collect();
                shards.push(*shard_id);
                shards.sort_unstable();
                shards.dedup();
                shards
            }
            ShardTransaction::DecomposeAsset {
                outputs,
                ..
            } => {
                let mut shards: Vec<ShardId> = outputs.iter().map(|v| v.shard_id).collect();
                shards.sort_unstable();
                shards.dedup();
                shards
            }
            ShardTransaction::UnwrapCCC {
                burn,
                ..
            } => vec![burn.prev_out.shard_id],
            ShardTransaction::WrapCCC {
                shard_id,
                ..
            } => vec![*shard_id],
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
            ShardTransaction::IncreaseAssetSupply {
                ..
            } => index == 0,
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
            } => id == outputs[index].shard_id,
            ShardTransaction::IncreaseAssetSupply {
                shard_id,
                ..
            } => &id == shard_id,
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
            } => id == outputs[index].shard_id,
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
                allowed_script_hashes,
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
                    AssetMintOutput::default()
                };

                Ok(blake256_with_key(
                    &ShardTransaction::ComposeAsset {
                        network_id: *network_id,
                        shard_id: *shard_id,
                        metadata: metadata.to_string(),
                        approver: *approver,
                        administrator: *administrator,
                        allowed_script_hashes: allowed_script_hashes.to_vec(),
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
const ASSET_INCREASE_SUPPLY_ID: TransactionId = 0x18;

impl Decodable for ShardTransaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        match d.val_at(0)? {
            ASSET_MINT_ID => {
                if d.item_count()? != 10 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::MintAsset {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    metadata: d.val_at(3)?,
                    output: AssetMintOutput {
                        lock_script_hash: d.val_at(4)?,
                        parameters: d.val_at(5)?,
                        supply: d.val_at(6)?,
                    },
                    approver: d.val_at(7)?,
                    administrator: d.val_at(8)?,
                    allowed_script_hashes: d.list_at(9)?,
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
                if d.item_count()? != 8 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::ChangeAssetScheme {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    asset_type: d.val_at(3)?,
                    metadata: d.val_at(4)?,
                    approver: d.val_at(5)?,
                    administrator: d.val_at(6)?,
                    allowed_script_hashes: d.list_at(7)?,
                })
            }
            ASSET_INCREASE_SUPPLY_ID => {
                if d.item_count()? != 7 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::IncreaseAssetSupply {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    asset_type: d.val_at(3)?,
                    output: AssetMintOutput {
                        lock_script_hash: d.val_at(4)?,
                        parameters: d.val_at(5)?,
                        supply: d.val_at(6)?,
                    },
                })
            }
            ASSET_COMPOSE_ID => {
                if d.item_count()? != 11 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(ShardTransaction::ComposeAsset {
                    network_id: d.val_at(1)?,
                    shard_id: d.val_at(2)?,
                    metadata: d.val_at(3)?,
                    approver: d.val_at(4)?,
                    administrator: d.val_at(5)?,
                    allowed_script_hashes: d.list_at(6)?,
                    inputs: d.list_at(7)?,
                    output: AssetMintOutput {
                        lock_script_hash: d.val_at(8)?,
                        parameters: d.val_at(9)?,
                        supply: d.val_at(10)?,
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
                        supply,
                    },
                approver,
                administrator,
                allowed_script_hashes,
            } => {
                s.begin_list(10)
                    .append(&ASSET_MINT_ID)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(lock_script_hash)
                    .append(parameters)
                    .append(supply)
                    .append(approver)
                    .append(administrator)
                    .append_list(allowed_script_hashes);
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
                shard_id,
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
            } => {
                s.begin_list(8)
                    .append(&ASSET_SCHEME_CHANGE_ID)
                    .append(network_id)
                    .append(shard_id)
                    .append(asset_type)
                    .append(metadata)
                    .append(approver)
                    .append(administrator)
                    .append_list(allowed_script_hashes);
            }
            ShardTransaction::IncreaseAssetSupply {
                network_id,
                shard_id,
                asset_type,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        parameters,
                        supply,
                    },
            } => {
                s.begin_list(7)
                    .append(&ASSET_INCREASE_SUPPLY_ID)
                    .append(network_id)
                    .append(shard_id)
                    .append(asset_type)
                    .append(lock_script_hash)
                    .append(parameters)
                    .append(supply);
            }
            ShardTransaction::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        parameters,
                        supply,
                    },
            } => {
                s.begin_list(11)
                    .append(&ASSET_COMPOSE_ID)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(approver)
                    .append(administrator)
                    .append_list(allowed_script_hashes)
                    .append_list(inputs)
                    .append(lock_script_hash)
                    .append(parameters)
                    .append(supply);
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

    use super::super::AssetOutPoint;
    use super::*;

    #[test]
    fn _is_input_and_output_consistent() {
        let asset_type = H160::random();
        let quantity = 100;

        assert!(is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: H256::random(),
                    index: 0,
                    asset_type,
                    shard_id: 0,
                    quantity,
                },
                timelock: None,
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                asset_type,
                shard_id: 0,
                quantity,
            }]
        ));
    }

    #[test]
    fn multiple_asset_is_input_and_output_consistent() {
        let asset_type1 = H160::random();
        let asset_type2 = {
            let mut asset_type = H160::random();
            while asset_type == asset_type1 {
                asset_type = H160::random();
            }
            asset_type
        };
        let quantity1 = 100;
        let quantity2 = 200;

        assert!(is_input_and_output_consistent(
            &[
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        shard_id: 0,
                        quantity: quantity1,
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
                        shard_id: 0,
                        quantity: quantity2,
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
                    shard_id: 0,
                    quantity: quantity1,
                },
                AssetTransferOutput {
                    lock_script_hash: H160::random(),
                    parameters: vec![],
                    asset_type: asset_type2,
                    shard_id: 0,
                    quantity: quantity2,
                },
            ]
        ));
    }

    #[test]
    fn multiple_asset_different_order_is_input_and_output_consistent() {
        let asset_type1 = H160::random();
        let asset_type2 = {
            let mut asset_type = H160::random();
            while asset_type == asset_type1 {
                asset_type = H160::random();
            }
            asset_type
        };
        let quantity1 = 100;
        let quantity2 = 200;

        assert!(is_input_and_output_consistent(
            &[
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type1,
                        shard_id: 0,
                        quantity: quantity1,
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
                        shard_id: 0,
                        quantity: quantity2,
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
                    shard_id: 0,
                    quantity: quantity2,
                },
                AssetTransferOutput {
                    lock_script_hash: H160::random(),
                    parameters: vec![],
                    asset_type: asset_type1,
                    shard_id: 0,
                    quantity: quantity1,
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
        let asset_type = H160::random();
        let output_quantity = 100;
        assert!(!is_input_and_output_consistent(
            &[],
            &[AssetTransferOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                asset_type,
                shard_id: 0,
                quantity: output_quantity,
            }]
        ));
    }

    #[test]
    fn fail_if_input_has_more_asset() {
        let asset_type = H160::random();
        let input_quantity = 100;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: H256::random(),
                    index: 0,
                    asset_type,
                    shard_id: 0,
                    quantity: input_quantity,
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
        let asset_type = H160::random();
        let input_quantity = 100;
        let output_quantity = 80;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: H256::random(),
                    index: 0,
                    asset_type,
                    shard_id: 0,
                    quantity: input_quantity,
                },
                timelock: None,
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                asset_type,
                shard_id: 0,
                quantity: output_quantity,
            }]
        ));
    }

    #[test]
    fn fail_if_input_is_smaller_than_output() {
        let asset_type = H160::random();
        let input_quantity = 80;
        let output_quantity = 100;

        assert!(!is_input_and_output_consistent(
            &[AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: H256::random(),
                    index: 0,
                    asset_type,
                    shard_id: 0,
                    quantity: input_quantity,
                },
                timelock: None,
                lock_script: vec![],
                unlock_script: vec![],
            }],
            &[AssetTransferOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                asset_type,
                shard_id: 0,
                quantity: output_quantity,
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
                    asset_type: H160::default(),
                    shard_id: 0,
                    quantity: 30,
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
                    asset_type: H160::zero(),
                    shard_id: 0,
                    quantity: 30,
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
                    asset_type: H160::random(),
                    shard_id: 0,
                    quantity: 30,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            }],
            outputs: vec![AssetTransferOutput {
                lock_script_hash: H160::random(),
                parameters: vec![vec![1]],
                asset_type: H160::random(),
                shard_id: 0,
                quantity: 30,
            }],
            orders: vec![OrderOnTransfer {
                order: Order {
                    asset_type_from: H160::random(),
                    asset_type_to: H160::random(),
                    asset_type_fee: H160::random(),
                    shard_id_from: 0,
                    shard_id_to: 0,
                    shard_id_fee: 0,
                    asset_quantity_from: 10,
                    asset_quantity_to: 10,
                    asset_quantity_fee: 0,
                    origin_outputs: vec![AssetOutPoint {
                        tracker: H256::random(),
                        index: 0,
                        asset_type: H160::random(),
                        shard_id: 0,
                        quantity: 30,
                    }],
                    expiration: 10,
                    lock_script_hash_from: H160::random(),
                    parameters_from: vec![vec![1]],
                    lock_script_hash_fee: H160::random(),
                    parameters_fee: vec![vec![1]],
                },
                spent_quantity: 10,
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
                asset_type: H160::default(),
                shard_id: 0,
                quantity: 0,
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
                    asset_type: H160::default(),
                    shard_id: 0,
                    quantity: 0,
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
                asset_type: H160::default(),
                shard_id: 0,
                quantity: 0,
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
        let mut sum: HashMap<H160, u128> = HashMap::new();

        for input in inputs {
            let asset_type = input.prev_out.asset_type;
            let quantity = u128::from(input.prev_out.quantity);
            *sum.entry(asset_type).or_insert_with(Default::default) += quantity;
        }
        for output in outputs {
            let asset_type = output.asset_type;
            let quantity = u128::from(output.quantity);
            let current_quantity = if let Some(current_quantity) = sum.get(&asset_type) {
                if *current_quantity < quantity {
                    return false
                }
                *current_quantity
            } else {
                return false
            };
            let t = sum.insert(asset_type, current_quantity - quantity);
            debug_assert!(t.is_some());
        }

        sum.iter().all(|(_, sum)| *sum == 0)
    }
}
