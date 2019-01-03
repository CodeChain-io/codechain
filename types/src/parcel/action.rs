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

use ccrypto::Blake;
use ckey::{Address, NetworkId, Public, Signature};
use heapsize::HeapSizeOf;
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::parcel::Error as ParcelError;
use crate::transaction::{
    AssetMintOutput, AssetTransferInput, AssetTransferOutput, Error as TransactionError, OrderOnTransfer,
    ShardTransaction,
};
use crate::ShardId;

const ASSET_TRANSACTION: u8 = 0x01;
const PAY: u8 = 0x02;
const SET_REGULAR_KEY: u8 = 0x03;
const CREATE_SHARD: u8 = 0x04;
const SET_SHARD_OWNERS: u8 = 0x05;
const SET_SHARD_USERS: u8 = 0x06;
const WRAP_CCC: u8 = 0x07;
const STORE: u8 = 0x08;
const REMOVE: u8 = 0x09;

const MINT_ASSET: u8 = 0x03;
const TRANSFER_ASSET: u8 = 0x04;
const CHANGE_ASSET_SCHEME: u8 = 0x05;
const COMPOSE_ASSET: u8 = 0x06;
const DECOMPOSE_ASSET: u8 = 0x07;
const UNWRAP_CCC: u8 = 0x01;

const CUSTOM: u8 = 0xFF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,

        output: AssetMintOutput,
        approvals: Vec<Signature>,
    },
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,
        approvals: Vec<Signature>,
    },
    ChangeAssetScheme {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        approvals: Vec<Signature>,
    },
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        inputs: Vec<AssetTransferInput>,
        output: AssetMintOutput,
        approvals: Vec<Signature>,
    },
    DecomposeAsset {
        network_id: NetworkId,
        input: AssetTransferInput,
        outputs: Vec<AssetTransferOutput>,
        approvals: Vec<Signature>,
    },
    UnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
        approvals: Vec<Signature>,
    },
    Pay {
        receiver: Address,
        /// Transferred amount.
        amount: u64,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard,
    SetShardOwners {
        shard_id: ShardId,
        owners: Vec<Address>,
    },
    SetShardUsers {
        shard_id: ShardId,
        users: Vec<Address>,
    },
    WrapCCC {
        shard_id: ShardId,
        lock_script_hash: H160,
        parameters: Vec<Bytes>,
        amount: u64,
    },
    Custom {
        handler_id: u64,
        bytes: Bytes,
    },
    Store {
        content: String,
        certifier: Address,
        signature: Signature,
    },
    Remove {
        hash: H256,
        signature: Signature,
    },
}

impl Action {
    pub fn hash(&self) -> H256 {
        let rlp = self.rlp_bytes();
        Blake::blake(rlp)
    }

    pub fn asset_transaction(&self) -> Option<ShardTransaction> {
        match self {
            Action::MintAsset {
                ..
            } => self.clone().into(),
            Action::TransferAsset {
                ..
            } => self.clone().into(),
            Action::ChangeAssetScheme {
                ..
            } => self.clone().into(),
            Action::ComposeAsset {
                ..
            } => self.clone().into(),
            Action::DecomposeAsset {
                ..
            } => self.clone().into(),
            Action::UnwrapCCC {
                ..
            } => self.clone().into(),
            _ => None,
        }
    }

    pub fn tracker(&self) -> Option<H256> {
        self.asset_transaction().map(|tx| tx.tracker())
    }

    pub fn verify(
        &self,
        system_network_id: NetworkId,
        max_metadata_size: usize,
        max_text_size: usize,
    ) -> Result<(), ParcelError> {
        match self {
            Action::MintAsset {
                network_id,
                metadata,
                output,
                ..
            } => {
                if *network_id != system_network_id {
                    return Err(ParcelError::InvalidNetworkId(*network_id))
                }
                if metadata.len() > max_metadata_size {
                    return Err(ParcelError::MetadataTooBig)
                }
                match output.amount {
                    Some(amount) if amount == 0 => return Err(TransactionError::ZeroAmount.into()),
                    _ => {}
                }
            }
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
                ..
            } => {
                if outputs.len() > 512 {
                    return Err(TransactionError::TooManyOutputs(outputs.len()).into())
                }
                if !is_input_and_output_consistent(inputs, outputs) {
                    return Err(TransactionError::InconsistentTransactionInOut.into())
                }
                if burns.iter().any(|burn| burn.prev_out.amount == 0) {
                    return Err(TransactionError::ZeroAmount.into())
                }
                if inputs.iter().any(|input| input.prev_out.amount == 0) {
                    return Err(TransactionError::ZeroAmount.into())
                }
                check_duplication_in_prev_out(burns, inputs)?;
                if outputs.iter().any(|output| output.amount == 0) {
                    return Err(TransactionError::ZeroAmount.into())
                }
                for order in orders {
                    order.order.verify()?;
                }
                verify_order_indices(orders, inputs.len(), outputs.len())?;
                verify_input_and_output_consistent_with_order(orders, inputs, outputs)?;
                if *network_id != system_network_id {
                    return Err(ParcelError::InvalidNetworkId(*network_id))
                }
            }
            Action::ChangeAssetScheme {
                network_id,
                metadata,
                ..
            } => {
                if *network_id != system_network_id {
                    return Err(ParcelError::InvalidNetworkId(*network_id))
                }
                if metadata.len() > max_metadata_size {
                    return Err(ParcelError::MetadataTooBig)
                }
            }
            Action::ComposeAsset {
                network_id,
                metadata,
                inputs,
                output,
                ..
            } => {
                if inputs.is_empty() {
                    return Err(TransactionError::EmptyInput.into())
                }
                if inputs.iter().any(|input| input.prev_out.amount == 0) {
                    return Err(TransactionError::ZeroAmount.into())
                }
                check_duplication_in_prev_out(&[], inputs)?;
                match output.amount {
                    Some(amount) if amount == 1 => {}
                    _ => {
                        return Err(TransactionError::InvalidComposedOutput {
                            got: output.amount.unwrap_or_default(),
                        }
                        .into())
                    }
                }
                if *network_id != system_network_id {
                    return Err(ParcelError::InvalidNetworkId(*network_id))
                }
                if metadata.len() > max_metadata_size {
                    return Err(ParcelError::MetadataTooBig)
                }
            }
            Action::DecomposeAsset {
                input,
                outputs,
                network_id,
                ..
            } => {
                if input.prev_out.amount != 1 {
                    return Err(TransactionError::InvalidDecomposedInput {
                        address: input.prev_out.asset_type,
                        got: input.prev_out.amount,
                    }
                    .into())
                }
                if outputs.is_empty() {
                    return Err(TransactionError::EmptyOutput.into())
                }
                if outputs.iter().any(|output| output.amount == 0) {
                    return Err(TransactionError::ZeroAmount.into())
                }
                if *network_id != system_network_id {
                    return Err(ParcelError::InvalidNetworkId(*network_id))
                }
            }
            Action::UnwrapCCC {
                burn,
                network_id,
                ..
            } => {
                if burn.prev_out.amount == 0 {
                    return Err(TransactionError::ZeroAmount.into())
                }
                if !burn.prev_out.asset_type.ends_with(&[0; 28]) {
                    return Err(TransactionError::InvalidAssetType(burn.prev_out.asset_type).into())
                }
                if *network_id != system_network_id {
                    return Err(ParcelError::InvalidNetworkId(*network_id))
                }
            }
            Action::WrapCCC {
                amount,
                ..
            } => {
                if *amount == 0 {
                    return Err(ParcelError::ZeroAmount)
                }
            }
            Action::Store {
                content,
                ..
            } => {
                if content.len() > max_text_size {
                    return Err(ParcelError::TextContentTooBig)
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl From<Action> for Option<ShardTransaction> {
    fn from(action: Action) -> Self {
        match action {
            Action::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                output,
                ..
            } => Some(ShardTransaction::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                output,
            }),
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
                ..
            } => Some(ShardTransaction::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
            }),
            Action::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
                ..
            } => Some(ShardTransaction::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
            }),
            Action::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                inputs,
                output,
                ..
            } => Some(ShardTransaction::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                inputs,
                output,
            }),
            Action::DecomposeAsset {
                network_id,
                input,
                outputs,
                ..
            } => Some(ShardTransaction::DecomposeAsset {
                network_id,
                input,
                outputs,
            }),
            Action::UnwrapCCC {
                network_id,
                burn,
                ..
            } => Some(ShardTransaction::UnwrapCCC {
                network_id,
                burn,
            }),
            _ => None,
        }
    }
}

impl HeapSizeOf for Action {
    fn heap_size_of_children(&self) -> usize {
        match self {
            Action::MintAsset {
                metadata,
                output,
                approvals,
                ..
            } => metadata.heap_size_of_children() + output.heap_size_of_children() + approvals.heap_size_of_children(),
            Action::TransferAsset {
                burns,
                inputs,
                outputs,
                orders,
                approvals,
                ..
            } => {
                burns.heap_size_of_children()
                    + inputs.heap_size_of_children()
                    + outputs.heap_size_of_children()
                    + orders.heap_size_of_children()
                    + approvals.heap_size_of_children()
            }
            Action::ChangeAssetScheme {
                metadata,
                approvals,
                ..
            } => metadata.heap_size_of_children() + approvals.heap_size_of_children(),
            Action::ComposeAsset {
                metadata,
                inputs,
                output,
                approvals,
                ..
            } => {
                metadata.heap_size_of_children()
                    + inputs.heap_size_of_children()
                    + output.heap_size_of_children()
                    + approvals.heap_size_of_children()
            }
            Action::DecomposeAsset {
                input,
                outputs,
                approvals,
                ..
            } => input.heap_size_of_children() + outputs.heap_size_of_children() + approvals.heap_size_of_children(),
            Action::UnwrapCCC {
                burn,
                approvals,
                ..
            } => burn.heap_size_of_children() + approvals.heap_size_of_children(),
            Action::SetShardOwners {
                owners,
                ..
            } => owners.heap_size_of_children(),
            Action::SetShardUsers {
                users,
                ..
            } => users.heap_size_of_children(),
            Action::WrapCCC {
                parameters,
                ..
            } => parameters.heap_size_of_children(),
            _ => 0,
        }
    }
}

impl Encodable for Action {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Action::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                output:
                    AssetMintOutput {
                        lock_script_hash,
                        parameters,
                        amount,
                    },
                approvals,
            } => {
                s.begin_list(3);
                s.append(&ASSET_TRANSACTION);
                s.begin_list(9)
                    .append(&MINT_ASSET)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(lock_script_hash)
                    .append(parameters)
                    .append(amount)
                    .append(approver)
                    .append(administrator);
                s.append_list(approvals);
            }
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
                approvals,
            } => {
                s.begin_list(3);
                s.append(&ASSET_TRANSACTION);
                s.begin_list(6)
                    .append(&TRANSFER_ASSET)
                    .append(network_id)
                    .append_list(burns)
                    .append_list(inputs)
                    .append_list(outputs)
                    .append_list(orders);
                s.append_list(approvals);
            }
            Action::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
                approvals,
            } => {
                s.begin_list(3);
                s.append(&ASSET_TRANSACTION);
                s.begin_list(6)
                    .append(&CHANGE_ASSET_SCHEME)
                    .append(network_id)
                    .append(asset_type)
                    .append(metadata)
                    .append(approver)
                    .append(administrator);
                s.append_list(approvals);
            }
            Action::ComposeAsset {
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
                approvals,
            } => {
                s.begin_list(3);
                s.append(&ASSET_TRANSACTION);
                s.begin_list(10)
                    .append(&COMPOSE_ASSET)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(approver)
                    .append(administrator)
                    .append_list(inputs)
                    .append(lock_script_hash)
                    .append(parameters)
                    .append(amount);
                s.append_list(approvals);
            }
            Action::DecomposeAsset {
                network_id,
                input,
                outputs,
                approvals,
            } => {
                s.begin_list(3);
                s.append(&ASSET_TRANSACTION);
                s.begin_list(4).append(&DECOMPOSE_ASSET).append(network_id).append(input).append_list(outputs);
                s.append_list(approvals);
            }
            Action::UnwrapCCC {
                network_id,
                burn,
                approvals,
            } => {
                s.begin_list(3);
                s.append(&ASSET_TRANSACTION);
                s.begin_list(3).append(&UNWRAP_CCC).append(network_id).append(burn);
                s.append_list(approvals);
            }
            Action::Pay {
                receiver,
                amount,
            } => {
                s.begin_list(3);
                s.append(&PAY);
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
            Action::SetShardOwners {
                shard_id,
                owners,
            } => {
                s.begin_list(3);
                s.append(&SET_SHARD_OWNERS);
                s.append(shard_id);
                s.append_list(owners);
            }
            Action::SetShardUsers {
                shard_id,
                users,
            } => {
                s.begin_list(3);
                s.append(&SET_SHARD_USERS);
                s.append(shard_id);
                s.append_list(users);
            }
            Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                amount,
            } => {
                s.begin_list(5);
                s.append(&WRAP_CCC);
                s.append(shard_id);
                s.append(lock_script_hash);
                s.append(parameters);
                s.append(amount);
            }
            Action::Store {
                content,
                certifier,
                signature,
            } => {
                s.begin_list(4);
                s.append(&STORE);
                s.append(content);
                s.append(certifier);
                s.append(signature);
            }
            Action::Remove {
                hash,
                signature,
            } => {
                s.begin_list(3);
                s.append(&REMOVE);
                s.append(hash);
                s.append(signature);
            }
            Action::Custom {
                handler_id,
                bytes,
            } => {
                s.begin_list(3);
                s.append(&CUSTOM);
                s.append(handler_id);
                s.append(bytes);
            }
        }
    }
}

impl Decodable for Action {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        match rlp.val_at(0)? {
            ASSET_TRANSACTION => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                let tx = rlp.at(1)?;
                let approvals = rlp.list_at(2)?;
                match tx.val_at(0)? {
                    MINT_ASSET => {
                        if tx.item_count()? != 9 {
                            return Err(DecoderError::RlpIncorrectListLen)
                        }
                        Ok(Action::MintAsset {
                            network_id: tx.val_at(1)?,
                            shard_id: tx.val_at(2)?,
                            metadata: tx.val_at(3)?,
                            output: AssetMintOutput {
                                lock_script_hash: tx.val_at(4)?,
                                parameters: tx.val_at(5)?,
                                amount: tx.val_at(6)?,
                            },
                            approver: tx.val_at(7)?,
                            administrator: tx.val_at(8)?,
                            approvals,
                        })
                    }
                    TRANSFER_ASSET => {
                        if tx.item_count()? != 6 {
                            return Err(DecoderError::RlpIncorrectListLen)
                        }
                        Ok(Action::TransferAsset {
                            network_id: tx.val_at(1)?,
                            burns: tx.list_at(2)?,
                            inputs: tx.list_at(3)?,
                            outputs: tx.list_at(4)?,
                            orders: tx.list_at(5)?,
                            approvals,
                        })
                    }
                    CHANGE_ASSET_SCHEME => {
                        if tx.item_count()? != 6 {
                            return Err(DecoderError::RlpIncorrectListLen)
                        }
                        Ok(Action::ChangeAssetScheme {
                            network_id: tx.val_at(1)?,
                            asset_type: tx.val_at(2)?,
                            metadata: tx.val_at(3)?,
                            approver: tx.val_at(4)?,
                            administrator: tx.val_at(5)?,
                            approvals,
                        })
                    }
                    COMPOSE_ASSET => {
                        if tx.item_count()? != 10 {
                            return Err(DecoderError::RlpIncorrectListLen)
                        }
                        Ok(Action::ComposeAsset {
                            network_id: tx.val_at(1)?,
                            shard_id: tx.val_at(2)?,
                            metadata: tx.val_at(3)?,
                            approver: tx.val_at(4)?,
                            administrator: tx.val_at(5)?,
                            inputs: tx.list_at(6)?,
                            output: AssetMintOutput {
                                lock_script_hash: tx.val_at(7)?,
                                parameters: tx.list_at(8)?,
                                amount: tx.val_at(9)?,
                            },
                            approvals,
                        })
                    }
                    DECOMPOSE_ASSET => {
                        if tx.item_count()? != 4 {
                            return Err(DecoderError::RlpIncorrectListLen)
                        }
                        Ok(Action::DecomposeAsset {
                            network_id: tx.val_at(1)?,
                            input: tx.val_at(2)?,
                            outputs: tx.list_at(3)?,
                            approvals,
                        })
                    }
                    UNWRAP_CCC => {
                        if tx.item_count()? != 3 {
                            return Err(DecoderError::RlpIncorrectListLen)
                        }
                        Ok(Action::UnwrapCCC {
                            network_id: tx.val_at(1)?,
                            burn: tx.val_at(2)?,
                            approvals,
                        })
                    }
                    _ => Err(DecoderError::Custom("Unexpected transaction")),
                }
            }
            PAY => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::Pay {
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
            SET_SHARD_OWNERS => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::SetShardOwners {
                    shard_id: rlp.val_at(1)?,
                    owners: rlp.list_at(2)?,
                })
            }
            SET_SHARD_USERS => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::SetShardUsers {
                    shard_id: rlp.val_at(1)?,
                    users: rlp.list_at(2)?,
                })
            }
            WRAP_CCC => {
                if rlp.item_count()? != 5 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::WrapCCC {
                    shard_id: rlp.val_at(1)?,
                    lock_script_hash: rlp.val_at(2)?,
                    parameters: rlp.val_at(3)?,
                    amount: rlp.val_at(4)?,
                })
            }
            STORE => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::Store {
                    content: rlp.val_at(1)?,
                    certifier: rlp.val_at(2)?,
                    signature: rlp.val_at(3)?,
                })
            }
            REMOVE => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::Remove {
                    hash: rlp.val_at(1)?,
                    signature: rlp.val_at(2)?,
                })
            }
            CUSTOM => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::Custom {
                    handler_id: rlp.val_at(1)?,
                    bytes: rlp.val_at(2)?,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected action prefix")),
        }
    }
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

fn check_duplication_in_prev_out(
    burns: &[AssetTransferInput],
    inputs: &[AssetTransferInput],
) -> Result<(), TransactionError> {
    let mut prev_out_set = HashSet::new();
    for input in inputs.iter().chain(burns) {
        let prev_out = (input.prev_out.transaction_hash, input.prev_out.index);
        if !prev_out_set.insert(prev_out) {
            return Err(TransactionError::DuplicatedPreviousOutput {
                transaction_hash: input.prev_out.transaction_hash,
                index: input.prev_out.index,
            })
        }
    }
    Ok(())
}

fn verify_order_indices(
    orders: &[OrderOnTransfer],
    input_len: usize,
    output_len: usize,
) -> Result<(), TransactionError> {
    let mut input_check = vec![false; input_len];
    let mut output_check = vec![false; output_len];

    for order in orders {
        for input_idx in order.input_indices.iter() {
            if *input_idx >= input_len || input_check[*input_idx] {
                return Err(TransactionError::InvalidOrderInOutIndices)
            }
            input_check[*input_idx] = true;
        }

        for output_idx in order.output_indices.iter() {
            if *output_idx >= output_len || output_check[*output_idx] {
                return Err(TransactionError::InvalidOrderInOutIndices)
            }
            output_check[*output_idx] = true;
        }
    }
    Ok(())
}

fn verify_input_and_output_consistent_with_order(
    orders: &[OrderOnTransfer],
    inputs: &[AssetTransferInput],
    outputs: &[AssetTransferOutput],
) -> Result<(), TransactionError> {
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
                return Err(TransactionError::InconsistentTransactionInOutWithOrders)
            }
        }

        for output_idx in order_tx.output_indices.iter() {
            let output = &outputs[*output_idx];
            let owned_by_taker = order.check_transfer_output(output)?;
            if output.asset_type == order.asset_type_from {
                if output_amount_from != 0 {
                    return Err(TransactionError::InconsistentTransactionInOutWithOrders)
                }
                output_amount_from = output.amount;
            } else if output.asset_type == order.asset_type_to {
                if output_amount_to != 0 {
                    return Err(TransactionError::InconsistentTransactionInOutWithOrders)
                }
                output_amount_to = output.amount;
            } else if output.asset_type == order.asset_type_fee {
                if owned_by_taker {
                    if output_amount_fee_remaining != 0 {
                        return Err(TransactionError::InconsistentTransactionInOutWithOrders)
                    }
                    output_amount_fee_remaining = output.amount;
                } else {
                    if output_amount_fee_given != 0 {
                        return Err(TransactionError::InconsistentTransactionInOutWithOrders)
                    }
                    output_amount_fee_given = output.amount;
                }
            } else {
                return Err(TransactionError::InconsistentTransactionInOutWithOrders)
            }
        }

        // NOTE: If input_amount_from == output_amount_from, it means the asset is not spent as the order.
        // If it's allowed, everyone can move the asset from one to another without permission.
        if input_amount_from <= output_amount_from || input_amount_from - output_amount_from != order_tx.spent_amount {
            return Err(TransactionError::InconsistentTransactionInOutWithOrders)
        }
        if !is_ratio_greater_or_equal(
            order.asset_amount_from,
            order.asset_amount_to,
            order_tx.spent_amount,
            output_amount_to,
        ) {
            return Err(TransactionError::InconsistentTransactionInOutWithOrders)
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
            return Err(TransactionError::InconsistentTransactionInOutWithOrders)
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

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;
    use crate::transaction::{AssetOutPoint, Order};

    #[test]
    fn encode_and_decode_mint_asset() {
        rlp_encode_and_decode_test!(Action::MintAsset {
            network_id: "tc".into(),
            shard_id: 0xc,
            metadata: "mint test".to_string(),
            output: AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                amount: Some(10000),
            },
            approver: None,
            administrator: None,
            approvals: vec![Signature::random(), Signature::random(), Signature::random(), Signature::random()],
        });
    }

    #[test]
    fn encode_and_decode_mint_asset_with_parameters() {
        rlp_encode_and_decode_test!(Action::MintAsset {
            network_id: "tc".into(),
            shard_id: 3,
            metadata: "mint test".to_string(),
            output: AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![vec![1, 2, 3], vec![4, 5, 6], vec![0, 7]],
                amount: Some(10000),
            },
            approver: None,
            administrator: None,
            approvals: vec![Signature::random()],
        });
    }


    #[test]
    fn encode_and_decode_transfer_asset() {
        let burns = vec![];
        let inputs = vec![];
        let outputs = vec![];
        let orders = vec![];
        let network_id = "tc".into();
        rlp_encode_and_decode_test!(Action::TransferAsset {
            network_id,
            burns,
            inputs,
            outputs,
            orders,
            approvals: vec![Signature::random(), Signature::random()],
        });
    }

    #[test]
    fn encode_and_decode_pay_action() {
        rlp_encode_and_decode_test!(Action::Pay {
            receiver: Address::random(),
            amount: 300,
        });
    }

    #[test]
    fn encode_and_decode_set_shard_owners() {
        rlp_encode_and_decode_test!(Action::SetShardOwners {
            shard_id: 1,
            owners: vec![Address::random(), Address::random()],
        });
    }

    #[test]
    fn encode_and_decode_set_shard_users() {
        rlp_encode_and_decode_test!(Action::SetShardUsers {
            shard_id: 1,
            users: vec![Address::random(), Address::random()],
        });
    }

    #[test]
    fn encode_and_decode_store() {
        rlp_encode_and_decode_test!(Action::Store {
            content: "CodeChain".to_string(),
            certifier: Address::random(),
            signature: Signature::random(),
        });
    }

    #[test]
    fn encode_and_decode_remove() {
        rlp_encode_and_decode_test!(Action::Remove {
            hash: H256::random(),
            signature: Signature::random(),
        });
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

        let action = Action::TransferAsset {
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
            approvals: vec![],
        };
        assert_eq!(action.verify(NetworkId::default(), 1000, 1000), Ok(()));
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

        let action = Action::TransferAsset {
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
            approvals: vec![],
        };

        assert_eq!(action.verify(NetworkId::default(), 1000, 1000), Ok(()));
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

        let action = Action::TransferAsset {
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
            approvals: vec![],
        };
        assert_eq!(
            action.verify(NetworkId::default(), 1000, 1000),
            Err(TransactionError::InconsistentTransactionInOutWithOrders.into())
        );

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
        let action = Action::TransferAsset {
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
            approvals: vec![],
        };
        assert_eq!(
            action.verify(NetworkId::default(), 1000, 1000),
            Err(TransactionError::InconsistentTransactionInOutWithOrders.into())
        );

        // Case 2-2: asset_type_to
        let action = Action::TransferAsset {
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
            approvals: vec![],
        };
        assert_eq!(
            action.verify(NetworkId::default(), 1000, 1000),
            Err(TransactionError::InconsistentTransactionInOutWithOrders.into())
        );

        // Case 2-3: asset_type_fee
        let action = Action::TransferAsset {
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
            approvals: vec![],
        };
        assert_eq!(
            action.verify(NetworkId::default(), 1000, 1000),
            Err(TransactionError::InconsistentTransactionInOutWithOrders.into())
        );
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

        let action = Action::TransferAsset {
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
            approvals: vec![],
        };
        assert_eq!(action.verify(NetworkId::default(), 1000, 1000), Ok(()));
    }

    #[test]
    fn verify_unwrap_ccc_transaction_should_fail() {
        let tx_zero_amount = Action::UnwrapCCC {
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
            approvals: vec![],
        };
        assert_eq!(tx_zero_amount.verify(NetworkId::default(), 1000, 1000), Err(TransactionError::ZeroAmount.into()));

        let invalid_asset_type = H256::random();
        let tx_invalid_asset_type = Action::UnwrapCCC {
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
            approvals: vec![],
        };
        assert_eq!(
            tx_invalid_asset_type.verify(NetworkId::default(), 1000, 1000),
            Err(TransactionError::InvalidAssetType(invalid_asset_type).into())
        );
    }

    #[test]
    fn verify_wrap_ccc_transaction_should_fail() {
        let tx_zero_amount = Action::WrapCCC {
            shard_id: 0,
            lock_script_hash: H160::random(),
            parameters: vec![],
            amount: 0,
        };
        assert_eq!(tx_zero_amount.verify(NetworkId::default(), 1000, 1000), Err(ParcelError::ZeroAmount));
    }
}
