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

use std::collections::{HashMap, HashSet};

use ccrypto::Blake;
use ckey::{Address, NetworkId, Public, Signature};
use heapsize::HeapSizeOf;
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use crate::transaction::{
    AssetMintOutput, AssetTransferInput, AssetTransferOutput, Error as TransactionError, OrderOnTransfer, ParcelError,
    ShardTransaction,
};
use crate::ShardId;

const PAY: u8 = 0x02;
const SET_REGULAR_KEY: u8 = 0x03;
const CREATE_SHARD: u8 = 0x04;
const SET_SHARD_OWNERS: u8 = 0x05;
const SET_SHARD_USERS: u8 = 0x06;
const WRAP_CCC: u8 = 0x07;
const STORE: u8 = 0x08;
const REMOVE: u8 = 0x09;
const UNWRAP_CCC: u8 = 0x11;
const MINT_ASSET: u8 = 0x13;
const TRANSFER_ASSET: u8 = 0x14;
const CHANGE_ASSET_SCHEME: u8 = 0x15;
const COMPOSE_ASSET: u8 = 0x16;
const DECOMPOSE_ASSET: u8 = 0x17;

const CUSTOM: u8 = 0xFF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        allowed_script_hashes: Vec<H160>,
        output: Box<AssetMintOutput>,
        approvals: Vec<Signature>,
    },
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        orders: Vec<OrderOnTransfer>,
        metadata: String,
        approvals: Vec<Signature>,
    },
    ChangeAssetScheme {
        network_id: NetworkId,
        asset_type: H256,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        allowed_script_hashes: Vec<H160>,
        approvals: Vec<Signature>,
    },
    ComposeAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        administrator: Option<Address>,
        allowed_script_hashes: Vec<H160>,
        inputs: Vec<AssetTransferInput>,
        output: Box<AssetMintOutput>,
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
        /// Transferred quantity.
        quantity: u64,
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
        quantity: u64,
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
        max_asset_scheme_metadata_size: usize,
        max_transfer_metadata_size: usize,
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
                if metadata.len() > max_asset_scheme_metadata_size {
                    return Err(ParcelError::MetadataTooBig)
                }
                match output.supply {
                    Some(supply) if supply == 0 => return Err(TransactionError::ZeroQuantity.into()),
                    _ => {}
                }
            }
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
                metadata,
                ..
            } => {
                if metadata.len() > max_transfer_metadata_size {
                    return Err(ParcelError::MetadataTooBig)
                }
                if outputs.len() > 512 {
                    return Err(TransactionError::TooManyOutputs(outputs.len()).into())
                }
                if !is_input_and_output_consistent(inputs, outputs) {
                    return Err(TransactionError::InconsistentTransactionInOut.into())
                }
                if burns.iter().any(|burn| burn.prev_out.quantity == 0) {
                    return Err(TransactionError::ZeroQuantity.into())
                }
                if inputs.iter().any(|input| input.prev_out.quantity == 0) {
                    return Err(TransactionError::ZeroQuantity.into())
                }
                check_duplication_in_prev_out(burns, inputs)?;
                if outputs.iter().any(|output| output.quantity == 0) {
                    return Err(TransactionError::ZeroQuantity.into())
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
                if metadata.len() > max_asset_scheme_metadata_size {
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
                if inputs.iter().any(|input| input.prev_out.quantity == 0) {
                    return Err(TransactionError::ZeroQuantity.into())
                }
                check_duplication_in_prev_out(&[], inputs)?;
                match output.supply {
                    Some(supply) if supply == 1 => {}
                    _ => {
                        return Err(TransactionError::InvalidComposedOutput {
                            got: output.supply.unwrap_or_default(),
                        }
                        .into())
                    }
                }
                if *network_id != system_network_id {
                    return Err(ParcelError::InvalidNetworkId(*network_id))
                }
                if metadata.len() > max_asset_scheme_metadata_size {
                    return Err(ParcelError::MetadataTooBig)
                }
            }
            Action::DecomposeAsset {
                input,
                outputs,
                network_id,
                ..
            } => {
                if input.prev_out.quantity != 1 {
                    return Err(TransactionError::InvalidDecomposedInput {
                        address: input.prev_out.asset_type,
                        got: input.prev_out.quantity,
                    }
                    .into())
                }
                if outputs.is_empty() {
                    return Err(TransactionError::EmptyOutput.into())
                }
                if outputs.iter().any(|output| output.quantity == 0) {
                    return Err(TransactionError::ZeroQuantity.into())
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
                if burn.prev_out.quantity == 0 {
                    return Err(TransactionError::ZeroQuantity.into())
                }
                if !burn.prev_out.asset_type.ends_with(&[0; 28]) {
                    return Err(TransactionError::InvalidAssetType(burn.prev_out.asset_type).into())
                }
                if *network_id != system_network_id {
                    return Err(ParcelError::InvalidNetworkId(*network_id))
                }
            }
            Action::WrapCCC {
                quantity,
                ..
            } => {
                if *quantity == 0 {
                    return Err(ParcelError::ZeroQuantity)
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
                allowed_script_hashes,
                output,
                ..
            } => Some(ShardTransaction::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                output: *output,
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
                allowed_script_hashes,
                ..
            } => Some(ShardTransaction::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
            }),
            Action::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output,
                ..
            } => Some(ShardTransaction::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output: *output,
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
                allowed_script_hashes,
                ..
            } => {
                metadata.heap_size_of_children()
                    + output.heap_size_of_children()
                    + approvals.heap_size_of_children()
                    + allowed_script_hashes.heap_size_of_children()
            }
            Action::TransferAsset {
                burns,
                inputs,
                outputs,
                orders,
                metadata,
                approvals,
                ..
            } => {
                burns.heap_size_of_children()
                    + inputs.heap_size_of_children()
                    + outputs.heap_size_of_children()
                    + orders.heap_size_of_children()
                    + metadata.heap_size_of_children()
                    + approvals.heap_size_of_children()
            }
            Action::ChangeAssetScheme {
                metadata,
                approvals,
                allowed_script_hashes,
                ..
            } => {
                metadata.heap_size_of_children()
                    + approvals.heap_size_of_children()
                    + allowed_script_hashes.heap_size_of_children()
            }
            Action::ComposeAsset {
                metadata,
                inputs,
                output,
                approvals,
                allowed_script_hashes,
                ..
            } => {
                metadata.heap_size_of_children()
                    + inputs.heap_size_of_children()
                    + output.heap_size_of_children()
                    + approvals.heap_size_of_children()
                    + allowed_script_hashes.heap_size_of_children()
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
                allowed_script_hashes,
                output,
                approvals,
            } => {
                s.begin_list(11)
                    .append(&MINT_ASSET)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(&output.lock_script_hash)
                    .append(&output.parameters)
                    .append(&output.supply)
                    .append(approver)
                    .append(administrator)
                    .append_list(allowed_script_hashes)
                    .append_list(approvals);
            }
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                orders,
                metadata,
                approvals,
            } => {
                s.begin_list(8)
                    .append(&TRANSFER_ASSET)
                    .append(network_id)
                    .append_list(burns)
                    .append_list(inputs)
                    .append_list(outputs)
                    .append_list(orders)
                    .append(metadata)
                    .append_list(approvals);
            }
            Action::ChangeAssetScheme {
                network_id,
                asset_type,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                approvals,
            } => {
                s.begin_list(8)
                    .append(&CHANGE_ASSET_SCHEME)
                    .append(network_id)
                    .append(asset_type)
                    .append(metadata)
                    .append(approver)
                    .append(administrator)
                    .append_list(allowed_script_hashes)
                    .append_list(approvals);
            }
            Action::ComposeAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                administrator,
                allowed_script_hashes,
                inputs,
                output,
                approvals,
            } => {
                s.begin_list(12)
                    .append(&COMPOSE_ASSET)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(approver)
                    .append(administrator)
                    .append_list(allowed_script_hashes)
                    .append_list(inputs)
                    .append(&output.lock_script_hash)
                    .append(&output.parameters)
                    .append(&output.supply)
                    .append_list(approvals);
            }
            Action::DecomposeAsset {
                network_id,
                input,
                outputs,
                approvals,
            } => {
                s.begin_list(5)
                    .append(&DECOMPOSE_ASSET)
                    .append(network_id)
                    .append(input)
                    .append_list(outputs)
                    .append_list(approvals);
            }
            Action::UnwrapCCC {
                network_id,
                burn,
                approvals,
            } => {
                s.begin_list(4).append(&UNWRAP_CCC).append(network_id).append(burn).append_list(approvals);
            }
            Action::Pay {
                receiver,
                quantity,
            } => {
                s.begin_list(3);
                s.append(&PAY);
                s.append(receiver);
                s.append(quantity);
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
                quantity,
            } => {
                s.begin_list(5);
                s.append(&WRAP_CCC);
                s.append(shard_id);
                s.append(lock_script_hash);
                s.append(parameters);
                s.append(quantity);
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
            MINT_ASSET => {
                if rlp.item_count()? != 11 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::MintAsset {
                    network_id: rlp.val_at(1)?,
                    shard_id: rlp.val_at(2)?,
                    metadata: rlp.val_at(3)?,
                    output: Box::new(AssetMintOutput {
                        lock_script_hash: rlp.val_at(4)?,
                        parameters: rlp.val_at(5)?,
                        supply: rlp.val_at(6)?,
                    }),
                    approver: rlp.val_at(7)?,
                    administrator: rlp.val_at(8)?,
                    allowed_script_hashes: rlp.list_at(9)?,
                    approvals: rlp.list_at(10)?,
                })
            }
            TRANSFER_ASSET => {
                if rlp.item_count()? != 8 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::TransferAsset {
                    network_id: rlp.val_at(1)?,
                    burns: rlp.list_at(2)?,
                    inputs: rlp.list_at(3)?,
                    outputs: rlp.list_at(4)?,
                    orders: rlp.list_at(5)?,
                    metadata: rlp.val_at(6)?,
                    approvals: rlp.list_at(7)?,
                })
            }
            CHANGE_ASSET_SCHEME => {
                if rlp.item_count()? != 8 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::ChangeAssetScheme {
                    network_id: rlp.val_at(1)?,
                    asset_type: rlp.val_at(2)?,
                    metadata: rlp.val_at(3)?,
                    approver: rlp.val_at(4)?,
                    administrator: rlp.val_at(5)?,
                    allowed_script_hashes: rlp.list_at(6)?,
                    approvals: rlp.list_at(7)?,
                })
            }
            COMPOSE_ASSET => {
                if rlp.item_count()? != 12 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::ComposeAsset {
                    network_id: rlp.val_at(1)?,
                    shard_id: rlp.val_at(2)?,
                    metadata: rlp.val_at(3)?,
                    approver: rlp.val_at(4)?,
                    administrator: rlp.val_at(5)?,
                    allowed_script_hashes: rlp.list_at(6)?,
                    inputs: rlp.list_at(7)?,
                    output: Box::new(AssetMintOutput {
                        lock_script_hash: rlp.val_at(8)?,
                        parameters: rlp.list_at(9)?,
                        supply: rlp.val_at(10)?,
                    }),
                    approvals: rlp.list_at(11)?,
                })
            }
            DECOMPOSE_ASSET => {
                if rlp.item_count()? != 5 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::DecomposeAsset {
                    network_id: rlp.val_at(1)?,
                    input: rlp.val_at(2)?,
                    outputs: rlp.list_at(3)?,
                    approvals: rlp.list_at(4)?,
                })
            }
            UNWRAP_CCC => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::UnwrapCCC {
                    network_id: rlp.val_at(1)?,
                    burn: rlp.val_at(2)?,
                    approvals: rlp.list_at(3)?,
                })
            }
            PAY => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Action::Pay {
                    receiver: rlp.val_at(1)?,
                    quantity: rlp.val_at(2)?,
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
                    quantity: rlp.val_at(4)?,
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
        let quantity = u128::from(input.prev_out.quantity);
        let current_quantity = sum.get(&asset_type).cloned().unwrap_or_default();
        sum.insert(asset_type, current_quantity + quantity);
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

fn check_duplication_in_prev_out(
    burns: &[AssetTransferInput],
    inputs: &[AssetTransferInput],
) -> Result<(), TransactionError> {
    let mut prev_out_set = HashSet::new();
    for input in inputs.iter().chain(burns) {
        let prev_out = (input.prev_out.tracker, input.prev_out.index);
        if !prev_out_set.insert(prev_out) {
            return Err(TransactionError::DuplicatedPreviousOutput {
                transaction_hash: input.prev_out.tracker,
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
        let mut input_quantity_from: u64 = 0;
        let mut input_quantity_fee: u64 = 0;
        let mut output_quantity_from: u64 = 0;
        let mut output_quantity_to: u64 = 0;
        let mut output_quantity_fee_remaining: u64 = 0;
        let mut output_quantity_fee_given: u64 = 0;

        let order = &order_tx.order;

        // NOTE: If asset_quantity_fee is zero, asset_type_fee can be same as asset_type_from or asset_type_to.
        // But, asset_type_fee is compared at the last, so here's safe by the logic.

        for input_idx in order_tx.input_indices.iter() {
            let prev_out = &inputs[*input_idx].prev_out;
            if prev_out.asset_type == order.asset_type_from {
                input_quantity_from += prev_out.quantity;
            } else if prev_out.asset_type == order.asset_type_fee {
                input_quantity_fee += prev_out.quantity;
            } else {
                return Err(TransactionError::InconsistentTransactionInOutWithOrders)
            }
        }

        for output_idx in order_tx.output_indices.iter() {
            let output = &outputs[*output_idx];
            let owned_by_taker = order.check_transfer_output(output)?;
            if output.asset_type == order.asset_type_from {
                if output_quantity_from != 0 {
                    return Err(TransactionError::InconsistentTransactionInOutWithOrders)
                }
                output_quantity_from = output.quantity;
            } else if output.asset_type == order.asset_type_to {
                if output_quantity_to != 0 {
                    return Err(TransactionError::InconsistentTransactionInOutWithOrders)
                }
                output_quantity_to = output.quantity;
            } else if output.asset_type == order.asset_type_fee {
                if owned_by_taker {
                    if output_quantity_fee_remaining != 0 {
                        return Err(TransactionError::InconsistentTransactionInOutWithOrders)
                    }
                    output_quantity_fee_remaining = output.quantity;
                } else {
                    if output_quantity_fee_given != 0 {
                        return Err(TransactionError::InconsistentTransactionInOutWithOrders)
                    }
                    output_quantity_fee_given = output.quantity;
                }
            } else {
                return Err(TransactionError::InconsistentTransactionInOutWithOrders)
            }
        }

        // NOTE: If input_quantity_from == output_quantity_from, it means the asset is not spent as the order.
        // If it's allowed, everyone can move the asset from one to another without permission.
        if input_quantity_from <= output_quantity_from
            || input_quantity_from - output_quantity_from != order_tx.spent_quantity
        {
            return Err(TransactionError::InconsistentTransactionInOutWithOrders)
        }
        if !is_ratio_greater_or_equal(
            order.asset_quantity_from,
            order.asset_quantity_to,
            order_tx.spent_quantity,
            output_quantity_to,
        ) {
            return Err(TransactionError::InconsistentTransactionInOutWithOrders)
        }
        if input_quantity_fee < output_quantity_fee_remaining
            || input_quantity_fee - output_quantity_fee_remaining != output_quantity_fee_given
            || !is_ratio_equal(
                order.asset_quantity_from,
                order.asset_quantity_fee,
                order_tx.spent_quantity,
                output_quantity_fee_given,
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
            output: Box::new(AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                supply: Some(10000),
            }),
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],
            approvals: vec![Signature::random(), Signature::random(), Signature::random(), Signature::random()],
        });
    }

    #[test]
    fn encode_and_decode_mint_asset_with_parameters() {
        rlp_encode_and_decode_test!(Action::MintAsset {
            network_id: "tc".into(),
            shard_id: 3,
            metadata: "mint test".to_string(),
            output: Box::new(AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![vec![1, 2, 3], vec![4, 5, 6], vec![0, 7]],
                supply: Some(10000),
            }),
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],
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
        let metadata = "".into();
        rlp_encode_and_decode_test!(Action::TransferAsset {
            network_id,
            burns,
            inputs,
            outputs,
            orders,
            metadata,
            approvals: vec![Signature::random(), Signature::random()],
        });
    }

    #[test]
    fn encode_and_decode_pay_action() {
        rlp_encode_and_decode_test!(Action::Pay {
            receiver: Address::random(),
            quantity: 300,
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
            tracker: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            quantity: 30,
        };
        let order = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: H256::zero(),
            asset_quantity_from: 30,
            asset_quantity_to: 10,
            asset_quantity_fee: 0,
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
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        quantity: 10,
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
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    quantity: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order,
                spent_quantity: 30,
                input_indices: vec![0],
                output_indices: vec![0],
            }],
            metadata: "".into(),
            approvals: vec![],
        };
        assert_eq!(action.verify(NetworkId::default(), 1000, 1000, 1000), Ok(()));
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
            tracker: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            quantity: 40,
        };
        let origin_output_2 = AssetOutPoint {
            tracker: H256::random(),
            index: 0,
            asset_type: asset_type_c,
            quantity: 30,
        };

        let order = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: asset_type_c,
            asset_quantity_from: 30,
            asset_quantity_to: 20,
            asset_quantity_fee: 30,
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
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        quantity: 10,
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
                    quantity: 25,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters1.clone(),
                    asset_type: asset_type_b,
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters1.clone(),
                    asset_type: asset_type_c,
                    quantity: 15,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    quantity: 15,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters2.clone(),
                    asset_type: asset_type_c,
                    quantity: 15,
                },
            ],
            orders: vec![OrderOnTransfer {
                order,
                spent_quantity: 15,
                input_indices: vec![0, 1],
                output_indices: vec![0, 1, 2, 4],
            }],
            metadata: "".into(),
            approvals: vec![],
        };

        assert_eq!(action.verify(NetworkId::default(), 1000, 1000, 1000), Ok(()));
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
            tracker: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            quantity: 30,
        };
        let order = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: H256::zero(),
            asset_quantity_from: 25,
            asset_quantity_to: 10,
            asset_quantity_fee: 0,
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
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        quantity: 10,
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
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    quantity: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order,
                spent_quantity: 25,
                input_indices: vec![0],
                output_indices: vec![0],
            }],
            metadata: "".into(),
            approvals: vec![],
        };
        assert_eq!(
            action.verify(NetworkId::default(), 1000, 1000, 1000),
            Err(TransactionError::InconsistentTransactionInOutWithOrders.into())
        );

        // Case 2: multiple outputs with same order and asset_type
        let origin_output_1 = AssetOutPoint {
            tracker: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            quantity: 40,
        };
        let origin_output_2 = AssetOutPoint {
            tracker: H256::random(),
            index: 0,
            asset_type: asset_type_c,
            quantity: 40,
        };
        let order = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: asset_type_c,
            asset_quantity_from: 30,
            asset_quantity_to: 10,
            asset_quantity_fee: 30,
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
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        quantity: 10,
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
                    quantity: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_a,
                    quantity: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_c,
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    quantity: 30,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters_fee.clone(),
                    asset_type: asset_type_c,
                    quantity: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order: order.clone(),
                spent_quantity: 30,
                input_indices: vec![0, 1],
                output_indices: vec![0, 1, 2, 3, 5],
            }],
            metadata: "".into(),
            approvals: vec![],
        };
        assert_eq!(
            action.verify(NetworkId::default(), 1000, 1000, 1000),
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
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        quantity: 10,
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
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    quantity: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    quantity: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_c,
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    quantity: 30,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters_fee.clone(),
                    asset_type: asset_type_c,
                    quantity: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order: order.clone(),
                spent_quantity: 30,
                input_indices: vec![0, 1],
                output_indices: vec![0, 1, 2, 3, 5],
            }],
            metadata: "".into(),
            approvals: vec![],
        };
        assert_eq!(
            action.verify(NetworkId::default(), 1000, 1000, 1000),
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
                        tracker: H256::random(),
                        index: 0,
                        asset_type: asset_type_b,
                        quantity: 10,
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
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_b,
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_c,
                    quantity: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_c,
                    quantity: 5,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    quantity: 30,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters_fee.clone(),
                    asset_type: asset_type_c,
                    quantity: 30,
                },
            ],
            orders: vec![OrderOnTransfer {
                order: order.clone(),
                spent_quantity: 30,
                input_indices: vec![0, 1],
                output_indices: vec![0, 1, 2, 3, 5],
            }],
            metadata: "".into(),
            approvals: vec![],
        };
        assert_eq!(
            action.verify(NetworkId::default(), 1000, 1000, 1000),
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
            tracker: H256::random(),
            index: 0,
            asset_type: asset_type_a,
            quantity: 30,
        };
        let origin_output_2 = AssetOutPoint {
            tracker: H256::random(),
            index: 0,
            asset_type: asset_type_b,
            quantity: 10,
        };

        let order_1 = Order {
            asset_type_from: asset_type_a,
            asset_type_to: asset_type_b,
            asset_type_fee: H256::zero(),
            asset_quantity_from: 30,
            asset_quantity_to: 10,
            asset_quantity_fee: 0,
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
            asset_quantity_from: 10,
            asset_quantity_to: 20,
            asset_quantity_fee: 0,
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
                    quantity: 10,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: parameters.clone(),
                    asset_type: asset_type_a,
                    quantity: 20,
                },
                AssetTransferOutput {
                    lock_script_hash,
                    parameters: vec![],
                    asset_type: asset_type_a,
                    quantity: 10,
                },
            ],
            orders: vec![
                OrderOnTransfer {
                    order: order_1,
                    spent_quantity: 30,
                    input_indices: vec![0],
                    output_indices: vec![0],
                },
                OrderOnTransfer {
                    order: order_2,
                    spent_quantity: 10,
                    input_indices: vec![1],
                    output_indices: vec![1],
                },
            ],
            metadata: "".into(),
            approvals: vec![],
        };
        assert_eq!(action.verify(NetworkId::default(), 1000, 1000, 1000), Ok(()));
    }

    #[test]
    fn verify_unwrap_ccc_transaction_should_fail() {
        let tx_zero_quantity = Action::UnwrapCCC {
            network_id: NetworkId::default(),
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: Default::default(),
                    index: 0,
                    asset_type: H256::zero(),
                    quantity: 0,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            approvals: vec![],
        };
        assert_eq!(
            tx_zero_quantity.verify(NetworkId::default(), 1000, 1000, 1000),
            Err(TransactionError::ZeroQuantity.into())
        );

        let invalid_asset_type = H256::random();
        let tx_invalid_asset_type = Action::UnwrapCCC {
            network_id: NetworkId::default(),
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: Default::default(),
                    index: 0,
                    asset_type: invalid_asset_type,
                    quantity: 1,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            approvals: vec![],
        };
        assert_eq!(
            tx_invalid_asset_type.verify(NetworkId::default(), 1000, 1000, 1000),
            Err(TransactionError::InvalidAssetType(invalid_asset_type).into())
        );
    }

    #[test]
    fn verify_wrap_ccc_transaction_should_fail() {
        let tx_zero_quantity = Action::WrapCCC {
            shard_id: 0,
            lock_script_hash: H160::random(),
            parameters: vec![],
            quantity: 0,
        };
        assert_eq!(tx_zero_quantity.verify(NetworkId::default(), 1000, 1000, 1000), Err(ParcelError::ZeroQuantity));
    }
}
