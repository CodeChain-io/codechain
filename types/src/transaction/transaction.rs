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

use std::collections::{HashMap, HashSet};

use ccrypto::{blake128, blake256, blake256_with_key};
use ckey::{Address, NetworkId};
use heapsize::HeapSizeOf;
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::error::Error;
use super::{
    AssetMintOutput, AssetOutPoint, AssetTransferInput, AssetTransferOutput, HashingError, Order, OrderOnTransfer,
    PartialHashing,
};
use crate::util::tag::Tag;
use crate::ShardId;


/// Parcel transaction type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transaction {
    AssetMint {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,

        output: AssetMintOutput,
    },
    AssetTransfer {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,
    },
    AssetSchemeChange {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
    },
    AssetCompose {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        inputs: Vec<AssetTransferInput>,
        output: AssetMintOutput,
    },
    AssetDecompose {
        network_id: NetworkId,
        input: AssetTransferInput,
        outputs: Vec<AssetTransferOutput>,
    },
    AssetUnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
    },
}

/// Parcel transaction type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerTransaction {
    General(Transaction),
    AssetWrapCCC {
        network_id: NetworkId,
        shard_id: ShardId,
        parcel_hash: H256,
        output: AssetWrapCCCOutput,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetWrapCCCOutput {
    pub lock_script_hash: H160,
    pub parameters: Vec<Bytes>,
    pub amount: u64,
}

impl Transaction {
    pub fn hash(&self) -> H256 {
        blake256(&*self.rlp_bytes())
    }

    pub fn network_id(&self) -> NetworkId {
        match self {
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
            Transaction::AssetSchemeChange {
                network_id,
                ..
            } => *network_id,
            Transaction::AssetDecompose {
                network_id,
                ..
            } => *network_id,
            Transaction::AssetUnwrapCCC {
                network_id,
                ..
            } => *network_id,
        }
    }

    pub fn related_shards(&self) -> Vec<ShardId> {
        match self {
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
            Transaction::AssetSchemeChange {
                asset_type,
                ..
            } => vec![(ShardId::from(asset_type[2]) << 8) + ShardId::from(asset_type[3])],
            Transaction::AssetCompose {
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
            Transaction::AssetDecompose {
                outputs,
                ..
            } => {
                let mut shards: Vec<ShardId> = outputs.iter().map(AssetTransferOutput::related_shard).collect();
                shards.sort_unstable();
                shards.dedup();
                shards
            }
            Transaction::AssetUnwrapCCC {
                burn,
                ..
            } => vec![burn.related_shard()],
        }
    }

    pub fn verify(&self) -> Result<(), Error> {
        match self {
            Transaction::AssetTransfer {
                burns,
                inputs,
                outputs,
                orders,
                ..
            } => {
                if outputs.len() > 512 {
                    return Err(Error::TooManyOutputs(outputs.len()))
                }
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
                check_duplication_in_prev_out(burns, inputs)?;
                for output in outputs {
                    if output.amount == 0 {
                        return Err(Error::ZeroAmount)
                    }
                }
                for order in orders {
                    order.order.verify()?;
                }
                verify_order_indices(orders, inputs.len(), outputs.len())?;
                verify_input_and_output_consistent_with_order(orders, inputs, outputs)?;
                Ok(())
            }
            Transaction::AssetMint {
                output,
                ..
            } => match output.amount {
                Some(amount) if amount == 0 => Err(Error::ZeroAmount),
                _ => Ok(()),
            },
            Transaction::AssetSchemeChange {
                ..
            } => Ok(()),
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
                check_duplication_in_prev_out(&[], inputs)?;
                match output.amount {
                    Some(amount) if amount == 1 => Ok(()),
                    _ => Err(Error::InvalidComposedOutput {
                        got: output.amount.unwrap_or_default(),
                    }),
                }
            }
            Transaction::AssetDecompose {
                input,
                outputs,
                ..
            } => {
                if input.prev_out.amount != 1 {
                    return Err(Error::InvalidDecomposedInput {
                        address: input.prev_out.asset_type,
                        got: input.prev_out.amount,
                    })
                }
                if outputs.is_empty() {
                    return Err(Error::EmptyOutput)
                }
                for output in outputs {
                    if output.amount == 0 {
                        return Err(Error::ZeroAmount)
                    }
                }
                Ok(())
            }
            Transaction::AssetUnwrapCCC {
                burn,
                ..
            } => {
                if burn.prev_out.amount == 0 {
                    return Err(Error::ZeroAmount)
                }
                if !burn.prev_out.asset_type.ends_with(&[0; 28]) {
                    return Err(Error::InvalidAssetType(burn.prev_out.asset_type))
                }
                Ok(())
            }
        }
    }

    pub fn unwrapped_amount(&self) -> u64 {
        match self {
            Transaction::AssetUnwrapCCC {
                burn,
                ..
            } => burn.prev_out.amount,
            _ => 0,
        }
    }

    fn is_valid_output_index(&self, index: usize) -> bool {
        match self {
            Transaction::AssetMint {
                ..
            } => index == 0,
            Transaction::AssetTransfer {
                outputs,
                ..
            } => index < outputs.len(),
            Transaction::AssetSchemeChange {
                ..
            } => false,
            Transaction::AssetCompose {
                ..
            } => index == 0,
            Transaction::AssetDecompose {
                outputs,
                ..
            } => index < outputs.len(),
            Transaction::AssetUnwrapCCC {
                ..
            } => false,
        }
    }

    pub fn is_valid_shard_id_index(&self, index: usize, id: ShardId) -> bool {
        if !self.is_valid_output_index(index) {
            return false
        }
        match self {
            Transaction::AssetMint {
                shard_id,
                ..
            } => &id == shard_id,
            Transaction::AssetTransfer {
                outputs,
                ..
            } => id == outputs[index].related_shard(),
            Transaction::AssetSchemeChange {
                ..
            } => unreachable!("AssetSchemeChange doesn't have a valid index"),
            Transaction::AssetCompose {
                shard_id,
                ..
            } => &id == shard_id,
            Transaction::AssetDecompose {
                outputs,
                ..
            } => id == outputs[index].related_shard(),
            Transaction::AssetUnwrapCCC {
                ..
            } => unreachable!("UnwrapCCC doesn't have a valid index"),
        }
    }
}

impl InnerTransaction {
    pub fn hash(&self) -> H256 {
        match self {
            InnerTransaction::General(transaction) => transaction.hash(),
            InnerTransaction::AssetWrapCCC {
                parcel_hash,
                ..
            } => *parcel_hash,
        }
    }

    pub fn verify(&self) -> Result<(), Error> {
        match self {
            InnerTransaction::General(transaction) => transaction.verify(),
            InnerTransaction::AssetWrapCCC {
                output,
                ..
            } => {
                if output.amount == 0 {
                    return Err(Error::ZeroAmount)
                }
                Ok(())
            }
        }
    }
}

impl From<Transaction> for InnerTransaction {
    fn from(transaction: Transaction) -> Self {
        InnerTransaction::General(transaction)
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

impl HeapSizeOf for Transaction {
    fn heap_size_of_children(&self) -> usize {
        match self {
            Transaction::AssetMint {
                metadata,
                approver,
                output,
                ..
            } => metadata.heap_size_of_children() + approver.heap_size_of_children() + output.heap_size_of_children(),
            Transaction::AssetTransfer {
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
            Transaction::AssetSchemeChange {
                metadata,
                ..
            } => metadata.heap_size_of_children(),
            Transaction::AssetCompose {
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
            Transaction::AssetDecompose {
                input,
                outputs,
                ..
            } => input.heap_size_of_children() + outputs.heap_size_of_children(),
            Transaction::AssetUnwrapCCC {
                burn,
                ..
            } => burn.heap_size_of_children(),
        }
    }
}

fn check_duplication_in_prev_out(burns: &[AssetTransferInput], inputs: &[AssetTransferInput]) -> Result<(), Error> {
    let mut prev_out_set = HashSet::new();
    for input in inputs.iter().chain(burns) {
        let prev_out = (input.prev_out.transaction_hash, input.prev_out.index);
        if !prev_out_set.insert(prev_out) {
            return Err(Error::DuplicatedPreviousOutput {
                transaction_hash: input.prev_out.transaction_hash,
                index: input.prev_out.index,
            })
        }
    }
    Ok(())
}

fn verify_order_indices(orders: &[OrderOnTransfer], input_len: usize, output_len: usize) -> Result<(), Error> {
    let mut input_check = vec![false; input_len];
    let mut output_check = vec![false; output_len];

    for order in orders {
        for input_idx in order.input_indices.iter() {
            if *input_idx >= input_len || input_check[*input_idx] {
                return Err(Error::InvalidOrderInOutIndices)
            }
            input_check[*input_idx] = true;
        }

        for output_idx in order.output_indices.iter() {
            if *output_idx >= output_len || output_check[*output_idx] {
                return Err(Error::InvalidOrderInOutIndices)
            }
            output_check[*output_idx] = true;
        }
    }
    Ok(())
}

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

fn verify_input_and_output_consistent_with_order(
    orders: &[OrderOnTransfer],
    inputs: &[AssetTransferInput],
    outputs: &[AssetTransferOutput],
) -> Result<(), Error> {
    for order_tx in orders {
        let mut input_amount_from: u64 = 0;
        let mut input_amount_fee: u64 = 0;
        let mut output_amount_from: u64 = 0;
        let mut output_amount_to: u64 = 0;
        let mut output_amount_fee_remaining: u64 = 0;
        let mut output_amount_fee_given: u64 = 0;

        let order = &order_tx.order;

        // NOTE: If asset_amount_fee is zero, asset_type_fee can be same as asset_type_from or asset_type_to.
        // But, asset_type_fee is compared at the last, so here's safe by the logic.

        for input_idx in order_tx.input_indices.iter() {
            let prev_out = &inputs[*input_idx].prev_out;
            if prev_out.asset_type == order.asset_type_from {
                input_amount_from += prev_out.amount;
            } else if prev_out.asset_type == order.asset_type_fee {
                input_amount_fee += prev_out.amount;
            } else {
                return Err(Error::InconsistentTransactionInOutWithOrders)
            }
        }

        for output_idx in order_tx.output_indices.iter() {
            let output = &outputs[*output_idx];
            let owned_by_taker = order.check_transfer_output(output)?;
            if output.asset_type == order.asset_type_from {
                if output_amount_from != 0 {
                    return Err(Error::InconsistentTransactionInOutWithOrders)
                }
                output_amount_from = output.amount;
            } else if output.asset_type == order.asset_type_to {
                if output_amount_to != 0 {
                    return Err(Error::InconsistentTransactionInOutWithOrders)
                }
                output_amount_to = output.amount;
            } else if output.asset_type == order.asset_type_fee {
                if owned_by_taker {
                    if output_amount_fee_remaining != 0 {
                        return Err(Error::InconsistentTransactionInOutWithOrders)
                    }
                    output_amount_fee_remaining = output.amount;
                } else {
                    if output_amount_fee_given != 0 {
                        return Err(Error::InconsistentTransactionInOutWithOrders)
                    }
                    output_amount_fee_given = output.amount;
                }
            } else {
                return Err(Error::InconsistentTransactionInOutWithOrders)
            }
        }

        // NOTE: If input_amount_from == output_amount_from, it means the asset is not spent as the order.
        // If it's allowed, everyone can move the asset from one to another without permission.
        if input_amount_from <= output_amount_from || input_amount_from - output_amount_from != order_tx.spent_amount {
            return Err(Error::InconsistentTransactionInOutWithOrders)
        }
        if !is_ratio_greater_or_equal(
            order.asset_amount_from,
            order.asset_amount_to,
            order_tx.spent_amount,
            output_amount_to,
        ) {
            return Err(Error::InconsistentTransactionInOutWithOrders)
        }
        if input_amount_fee < output_amount_fee_remaining
            || input_amount_fee - output_amount_fee_remaining != output_amount_fee_given
            || !is_ratio_equal(
                order.asset_amount_from,
                order.asset_amount_fee,
                order_tx.spent_amount,
                output_amount_fee_given,
            )
        {
            return Err(Error::InconsistentTransactionInOutWithOrders)
        }
    }
    Ok(())
}

fn is_ratio_equal(a: u64, b: u64, c: u64, d: u64) -> bool {
    // a:b = c:d
    u128::from(a) * u128::from(d) == u128::from(b) * u128::from(c)
}

fn is_ratio_greater_or_equal(a: u64, b: u64, c: u64, d: u64) -> bool {
    // a:b <= c:d
    u128::from(a) * u128::from(d) >= u128::from(b) * u128::from(c)
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

impl PartialHashing for Transaction {
    fn hash_partially(&self, tag: Tag, cur: &AssetTransferInput, is_burn: bool) -> Result<H256, HashingError> {
        match self {
            Transaction::AssetTransfer {
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
                    &Transaction::AssetTransfer {
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
            Transaction::AssetCompose {
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
                    &Transaction::AssetCompose {
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
            Transaction::AssetDecompose {
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
                    &Transaction::AssetDecompose {
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
            Transaction::AssetUnwrapCCC {
                network_id,
                burn,
            } => {
                if !tag.sign_all_inputs || !tag.sign_all_outputs {
                    return Err(HashingError::InvalidFilter)
                }

                Ok(blake256_with_key(
                    &Transaction::AssetUnwrapCCC {
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
const ASSET_UNWRAP_CCC_ID: TransactionId = 0x01;
const ASSET_MINT_ID: TransactionId = 0x03;
const ASSET_TRANSFER_ID: TransactionId = 0x04;
const ASSET_SCHEME_CHANGE_ID: TransactionId = 0x05;
const ASSET_COMPOSE_ID: TransactionId = 0x06;
const ASSET_DECOMPOSE_ID: TransactionId = 0x07;

impl Decodable for Transaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        match d.val_at(0)? {
            ASSET_MINT_ID => {
                if d.item_count()? != 9 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetMint {
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
                Ok(Transaction::AssetTransfer {
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
                Ok(Transaction::AssetSchemeChange {
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
                Ok(Transaction::AssetCompose {
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
                Ok(Transaction::AssetDecompose {
                    network_id: d.val_at(1)?,
                    input: d.val_at(2)?,
                    outputs: d.list_at(3)?,
                })
            }
            ASSET_UNWRAP_CCC_ID => {
                if d.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetUnwrapCCC {
                    network_id: d.val_at(1)?,
                    burn: d.val_at(2)?,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected transaction")),
        }
    }
}

impl Encodable for Transaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Transaction::AssetMint {
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
            } => s
                .begin_list(9)
                .append(&ASSET_MINT_ID)
                .append(network_id)
                .append(shard_id)
                .append(metadata)
                .append(lock_script_hash)
                .append(parameters)
                .append(amount)
                .append(approver)
                .append(administrator),
            Transaction::AssetTransfer {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
            } => s
                .begin_list(6)
                .append(&ASSET_TRANSFER_ID)
                .append(network_id)
                .append_list(burns)
                .append_list(inputs)
                .append_list(outputs)
                .append_list(orders),
            Transaction::AssetSchemeChange {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
            } => s
                .begin_list(6)
                .append(&ASSET_SCHEME_CHANGE_ID)
                .append(network_id)
                .append(asset_type)
                .append(metadata)
                .append(approver)
                .append(administrator),
            Transaction::AssetCompose {
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
            } => s
                .begin_list(10)
                .append(&ASSET_COMPOSE_ID)
                .append(network_id)
                .append(shard_id)
                .append(metadata)
                .append(approver)
                .append(administrator)
                .append_list(inputs)
                .append(lock_script_hash)
                .append(parameters)
                .append(amount),
            Transaction::AssetDecompose {
                network_id,
                input,
                outputs,
            } => s.begin_list(4).append(&ASSET_DECOMPOSE_ID).append(network_id).append(input).append_list(outputs),
            Transaction::AssetUnwrapCCC {
                network_id,
                burn,
            } => s.begin_list(3).append(&ASSET_UNWRAP_CCC_ID).append(network_id).append(burn),
        };
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

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
                    transaction_hash: H256::random(),
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
                        transaction_hash: H256::random(),
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
                        transaction_hash: H256::random(),
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
                        transaction_hash: H256::random(),
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
                        transaction_hash: H256::random(),
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
                    transaction_hash: H256::random(),
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
                    transaction_hash: H256::random(),
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
                    transaction_hash: H256::random(),
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
        let tx = Transaction::AssetDecompose {
            network_id: NetworkId::default(),
            input: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::default(),
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
        let tx = Transaction::AssetUnwrapCCC {
            network_id: NetworkId::default(),
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::default(),
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
        let tx = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::random(),
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
                        transaction_hash: H256::random(),
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
    fn verify_wrap_ccc_transaction_should_fail() {
        let tx_zero_amount = InnerTransaction::AssetWrapCCC {
            network_id: NetworkId::default(),
            shard_id: 0,
            parcel_hash: H256::random(),
            output: AssetWrapCCCOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                amount: 0,
            },
        };
        assert_eq!(tx_zero_amount.verify(), Err(Error::ZeroAmount));
    }

    #[test]
    fn verify_unwrap_ccc_transaction_should_fail() {
        let tx_zero_amount = Transaction::AssetUnwrapCCC {
            network_id: NetworkId::default(),
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::default(),
                    index: 0,
                    asset_type: H256::zero(),
                    amount: 0,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
        };
        assert_eq!(tx_zero_amount.verify(), Err(Error::ZeroAmount));

        let invalid_asset_type = H256::random();
        let tx_invalid_asset_type = Transaction::AssetUnwrapCCC {
            network_id: NetworkId::default(),
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    transaction_hash: H256::default(),
                    index: 0,
                    asset_type: invalid_asset_type,
                    amount: 1,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
        };
        assert_eq!(tx_invalid_asset_type.verify(), Err(Error::InvalidAssetType(invalid_asset_type)));
    }

    #[test]
    fn verify_transfer_transaction_with_order() {
        let asset_type_a = H256::random();
        let asset_type_b = H256::random();
        let lock_script_hash = H160::random();
        let parameters = vec![vec![1]];
        let origin_output = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            amount: 30,
        };
        let order = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: H256::zero(),
            asset_amount_from: 30,
            asset_amount_to: 10,
            asset_amount_fee: 0,
            origin_outputs: vec![origin_output.clone()],
            expiration: 10,
            lock_script_hash_from: lock_script_hash,
            parameters_from: parameters.clone(),
            lock_script_hash_fee: lock_script_hash,
            parameters_fee: parameters.clone(),
        };

        let tx = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![
                AssetTransferInput {
                    prev_out: origin_output,
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        amount: 10,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    amount: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order,
                spent_amount: 30,
                input_indices: vec![0],
                output_indices: vec![0],
            }],
        };
        assert_eq!(tx.verify(), Ok(()));
    }

    #[test]
    fn verify_partial_fill_transfer_transaction_with_order() {
        let asset_type_a = H256::random();
        let asset_type_b = H256::random();
        let asset_type_c = H256::random();
        let lock_script_hash = H160::random();
        let parameters1 = vec![vec![1]];
        let parameters2 = vec![vec![2]];

        let origin_output_1 = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            amount: 40,
        };
        let origin_output_2 = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 0,
            asset_type: asset_type_c,
            amount: 30,
        };

        let order = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: asset_type_c,
            asset_amount_from: 30,
            asset_amount_to: 20,
            asset_amount_fee: 30,
            origin_outputs: vec![origin_output_1.clone(), origin_output_2.clone()],
            expiration: 10,
            lock_script_hash_from: lock_script_hash,
            parameters_from: parameters1.clone(),
            lock_script_hash_fee: lock_script_hash,
            parameters_fee: parameters2.clone(),
        };

        let tx = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![
                AssetTransferInput {
                    prev_out: origin_output_1,
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: origin_output_2,
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        amount: 10,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters1.clone(),
                    asset_type: asset_type_a,
                    amount: 25,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters1.clone(),
                    asset_type: asset_type_b,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters1.clone(),
                    asset_type: asset_type_c,
                    amount: 15,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    amount: 15,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters2.clone(),
                    asset_type: asset_type_c,
                    amount: 15,
                },
            ],
            orders: vec![OrderOnTransfer {
                order,
                spent_amount: 15,
                input_indices: vec![0, 1],
                output_indices: vec![0, 1, 2, 4],
            }],
        };

        assert_eq!(tx.verify(), Ok(()));
    }

    #[test]
    fn verify_inconsistent_transfer_transaction_with_order() {
        let asset_type_a = H256::random();
        let asset_type_b = H256::random();
        let asset_type_c = H256::random();
        let lock_script_hash = H160::random();
        let parameters = vec![vec![1]];
        let parameters_fee = vec![vec![2]];

        // Case 1: ratio is wrong
        let origin_output = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            amount: 30,
        };
        let order = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: H256::zero(),
            asset_amount_from: 25,
            asset_amount_to: 10,
            asset_amount_fee: 0,
            origin_outputs: vec![origin_output.clone()],
            expiration: 10,
            lock_script_hash_from: lock_script_hash,
            parameters_from: parameters.clone(),
            lock_script_hash_fee: lock_script_hash,
            parameters_fee: parameters_fee.clone(),
        };

        let tx = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![
                AssetTransferInput {
                    prev_out: origin_output,
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        amount: 10,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    amount: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order,
                spent_amount: 25,
                input_indices: vec![0],
                output_indices: vec![0],
            }],
        };
        assert_eq!(tx.verify(), Err(Error::InconsistentTransactionInOutWithOrders));

        // Case 2: multiple outputs with same order and asset_type
        let origin_output_1 = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            amount: 40,
        };
        let origin_output_2 = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 0,
            asset_type: asset_type_c,
            amount: 40,
        };
        let order = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: asset_type_c,
            asset_amount_from: 30,
            asset_amount_to: 10,
            asset_amount_fee: 30,
            origin_outputs: vec![origin_output_1.clone(), origin_output_2.clone()],
            expiration: 10,
            lock_script_hash_from: lock_script_hash,
            parameters_from: parameters.clone(),
            lock_script_hash_fee: lock_script_hash,
            parameters_fee: parameters_fee.clone(),
        };

        // Case 2-1: asset_type_from
        let tx = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![
                AssetTransferInput {
                    prev_out: origin_output_1.clone(),
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: origin_output_2.clone(),
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        amount: 10,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_a,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_a,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_c,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    amount: 30,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters_fee.clone(),
                    asset_type: asset_type_c,
                    amount: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order: order.clone(),
                spent_amount: 30,
                input_indices: vec![0, 1],
                output_indices: vec![0, 1, 2, 3, 5],
            }],
        };
        assert_eq!(tx.verify(), Err(Error::InconsistentTransactionInOutWithOrders));

        // Case 2-2: asset_type_to
        let tx = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![
                AssetTransferInput {
                    prev_out: origin_output_1.clone(),
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: origin_output_2.clone(),
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        amount: 10,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_a,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_c,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    amount: 30,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters_fee.clone(),
                    asset_type: asset_type_c,
                    amount: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order: order.clone(),
                spent_amount: 30,
                input_indices: vec![0, 1],
                output_indices: vec![0, 1, 2, 3, 5],
            }],
        };
        assert_eq!(tx.verify(), Err(Error::InconsistentTransactionInOutWithOrders));

        // Case 2-3: asset_type_fee
        let tx = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![
                AssetTransferInput {
                    prev_out: origin_output_1.clone(),
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: origin_output_2.clone(),
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: AssetOutPoint {
                        transaction_hash: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        amount: 10,
                    },
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_a,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_c,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_c,
                    amount: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    amount: 30,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters_fee.clone(),
                    asset_type: asset_type_c,
                    amount: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order: order.clone(),
                spent_amount: 30,
                input_indices: vec![0, 1],
                output_indices: vec![0, 1, 2, 3, 5],
            }],
        };
        assert_eq!(tx.verify(), Err(Error::InconsistentTransactionInOutWithOrders));
    }

    #[test]
    fn verify_transfer_transaction_with_two_orders() {
        let asset_type_a = H256::random();
        let asset_type_b = H256::random();
        let lock_script_hash = H160::random();
        let parameters = vec![vec![1]];
        let origin_output_1 = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            amount: 30,
        };
        let origin_output_2 = AssetOutPoint {
            transaction_hash: H256::random(),
            index: 0,
            asset_type: asset_type_b,
            amount: 10,
        };

        let order_1 = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: H256::zero(),
            asset_amount_from: 30,
            asset_amount_to: 10,
            asset_amount_fee: 0,
            origin_outputs: vec![origin_output_1.clone()],
            expiration: 10,
            lock_script_hash_from: lock_script_hash,
            parameters_from: parameters.clone(),
            lock_script_hash_fee: lock_script_hash,
            parameters_fee: parameters.clone(),
        };
        let order_2 = Order {
            asset_type_from: asset_type_b,
            asset_type_to: asset_type_a,
            asset_type_fee: H256::zero(),
            asset_amount_from: 10,
            asset_amount_to: 20,
            asset_amount_fee: 0,
            origin_outputs: vec![origin_output_2.clone()],
            expiration: 10,
            lock_script_hash_from: lock_script_hash,
            parameters_from: parameters.clone(),
            lock_script_hash_fee: lock_script_hash,
            parameters_fee: parameters.clone(),
        };

        let tx = Transaction::AssetTransfer {
            network_id: NetworkId::default(),
            burns: vec![],
            inputs: vec![
                AssetTransferInput {
                    prev_out: origin_output_1,
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
                AssetTransferInput {
                    prev_out: origin_output_2,
                    timelock: None,
                    lock_script: vec![0x30, 0x01],
                    unlock_script: vec![],
                },
            ],
            outputs: vec![
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    amount: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_a,
                    amount: 20,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    amount: 10,
                },
            ],
            orders: vec![
                OrderOnTransfer {
                    order: order_1,
                    spent_amount: 30,
                    input_indices: vec![0],
                    output_indices: vec![0],
                },
                OrderOnTransfer {
                    order: order_2,
                    spent_amount: 10,
                    input_indices: vec![1],
                    output_indices: vec![1],
                },
            ],
        };
        assert_eq!(tx.verify(), Ok(()));
    }

    #[test]
    fn apply_long_filter() {
        let input = AssetTransferInput {
            prev_out: AssetOutPoint {
                transaction_hash: H256::default(),
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
                    transaction_hash: H256::default(),
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

        let transaction = Transaction::AssetTransfer {
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
        let transaction_aux = Transaction::AssetTransfer {
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
        let transaction_aux = Transaction::AssetTransfer {
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
}
