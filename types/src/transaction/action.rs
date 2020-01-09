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

use crate::errors::SyntaxError;
use crate::transaction::{AssetMintOutput, AssetTransferInput, AssetTransferOutput, ShardTransaction};
use crate::{CommonParams, ShardId, Tracker, TxHash};
use ccrypto::Blake;
use ckey::{recover, Address, NetworkId, Public, Signature};
use primitives::{Bytes, H160, H256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
#[repr(u8)]
enum ActionTag {
    Pay = 0x02,
    SetRegularKey = 0x03,
    CreateShard = 0x04,
    SetShardOwners = 0x05,
    SetShardUsers = 0x06,
    WrapCcc = 0x07,
    Store = 0x08,
    Remove = 0x09,
    UnwrapCcc = 0x11,
    MintAsset = 0x13,
    TransferAsset = 0x14,
    ChangeAssetScheme = 0x15,
    // Derepcated
    // ComposeAsset = 0x16,
    // Derepcated
    // DecomposeAsset = 0x17,
    IncreaseAssetSupply = 0x18,
    Custom = 0xFF,
}

impl Encodable for ActionTag {
    fn rlp_append(&self, s: &mut RlpStream) {
        (*self as u8).rlp_append(s)
    }
}

impl Decodable for ActionTag {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let tag = rlp.as_val()?;
        match tag {
            0x02u8 => Ok(Self::Pay),
            0x03u8 => Ok(Self::SetRegularKey),
            0x04u8 => Ok(Self::CreateShard),
            0x05u8 => Ok(Self::SetShardOwners),
            0x06u8 => Ok(Self::SetShardUsers),
            0x07u8 => Ok(Self::WrapCcc),
            0x08u8 => Ok(Self::Store),
            0x09u8 => Ok(Self::Remove),
            0x11u8 => Ok(Self::UnwrapCcc),
            0x13u8 => Ok(Self::MintAsset),
            0x14u8 => Ok(Self::TransferAsset),
            0x15u8 => Ok(Self::ChangeAssetScheme),
            0x18u8 => Ok(Self::IncreaseAssetSupply),
            0xFFu8 => Ok(Self::Custom),
            _ => Err(DecoderError::Custom("Unexpected action prefix")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    MintAsset {
        network_id: NetworkId,
        shard_id: ShardId,
        metadata: String,
        approver: Option<Address>,
        registrar: Option<Address>,
        allowed_script_hashes: Vec<H160>,
        output: Box<AssetMintOutput>,
        approvals: Vec<Signature>,
    },
    TransferAsset {
        network_id: NetworkId,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        metadata: String,
        approvals: Vec<Signature>,
        expiration: Option<u64>,
    },
    ChangeAssetScheme {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        seq: usize,
        metadata: String,
        approver: Option<Address>,
        registrar: Option<Address>,
        allowed_script_hashes: Vec<H160>,
        approvals: Vec<Signature>,
    },
    IncreaseAssetSupply {
        network_id: NetworkId,
        shard_id: ShardId,
        asset_type: H160,
        seq: usize,
        output: Box<AssetMintOutput>,
        approvals: Vec<Signature>,
    },
    UnwrapCCC {
        network_id: NetworkId,
        burn: AssetTransferInput,
        receiver: Address,
    },
    Pay {
        receiver: Address,
        /// Transferred quantity.
        quantity: u64,
    },
    SetRegularKey {
        key: Public,
    },
    CreateShard {
        users: Vec<Address>,
    },
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
        payer: Address,
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
        hash: TxHash,
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
            }
            | Action::TransferAsset {
                ..
            }
            | Action::ChangeAssetScheme {
                ..
            }
            | Action::IncreaseAssetSupply {
                ..
            }
            | Action::UnwrapCCC {
                ..
            } => self.clone().into(),
            _ => None,
        }
    }

    pub fn tracker(&self) -> Option<Tracker> {
        self.asset_transaction().map(|tx| tx.tracker())
    }

    pub fn verify(&self) -> Result<(), SyntaxError> {
        match self {
            Action::MintAsset {
                output,
                ..
            } => {
                if output.supply == 0 {
                    return Err(SyntaxError::ZeroQuantity)
                }
            }
            Action::TransferAsset {
                burns,
                inputs,
                outputs,
                ..
            } => {
                if outputs.len() > 512 {
                    return Err(SyntaxError::TooManyOutputs(outputs.len()))
                }
                if !is_input_and_output_consistent(inputs, outputs) {
                    return Err(SyntaxError::InconsistentTransactionInOut)
                }
                if burns.iter().any(|burn| burn.prev_out.quantity == 0) {
                    return Err(SyntaxError::ZeroQuantity)
                }
                if inputs.iter().any(|input| input.prev_out.quantity == 0) {
                    return Err(SyntaxError::ZeroQuantity)
                }
                check_duplication_in_prev_out(burns, inputs)?;

                if outputs.iter().any(|output| output.quantity == 0) {
                    return Err(SyntaxError::ZeroQuantity)
                }
            }
            Action::ChangeAssetScheme {
                asset_type,
                ..
            } => {
                if asset_type.is_zero() {
                    return Err(SyntaxError::CannotChangeWcccAssetScheme)
                }
            }
            Action::IncreaseAssetSupply {
                asset_type,
                output,
                ..
            } => {
                if output.supply == 0 {
                    return Err(SyntaxError::ZeroQuantity)
                }
                if asset_type.is_zero() {
                    return Err(SyntaxError::CannotChangeWcccAssetScheme)
                }
            }
            Action::UnwrapCCC {
                burn,
                ..
            } => {
                if burn.prev_out.quantity == 0 {
                    return Err(SyntaxError::ZeroQuantity)
                }
                if !burn.prev_out.asset_type.is_zero() {
                    return Err(SyntaxError::InvalidAssetType(burn.prev_out.asset_type))
                }
            }
            Action::WrapCCC {
                quantity,
                ..
            } => {
                if *quantity == 0 {
                    return Err(SyntaxError::ZeroQuantity)
                }
            }
            Action::Store {
                ..
            } => {}
            _ => {}
        }
        Ok(())
    }

    pub fn verify_with_params(&self, common_params: &CommonParams) -> Result<(), SyntaxError> {
        if let Some(network_id) = self.network_id() {
            let system_network_id = common_params.network_id();
            if network_id != system_network_id {
                return Err(SyntaxError::InvalidNetworkId(network_id))
            }
        }

        match self {
            Action::MintAsset {
                metadata,
                ..
            } => {
                let max_asset_scheme_metadata_size = common_params.max_asset_scheme_metadata_size();
                if metadata.len() > max_asset_scheme_metadata_size {
                    return Err(SyntaxError::MetadataTooBig)
                }
            }
            Action::TransferAsset {
                metadata,
                ..
            } => {
                let max_transfer_metadata_size = common_params.max_transfer_metadata_size();
                if metadata.len() > max_transfer_metadata_size {
                    return Err(SyntaxError::MetadataTooBig)
                }
            }
            Action::ChangeAssetScheme {
                metadata,
                ..
            } => {
                let max_asset_scheme_metadata_size = common_params.max_asset_scheme_metadata_size();
                if metadata.len() > max_asset_scheme_metadata_size {
                    return Err(SyntaxError::MetadataTooBig)
                }
            }
            Action::IncreaseAssetSupply {
                ..
            } => {}
            Action::UnwrapCCC {
                ..
            } => {}
            Action::WrapCCC {
                ..
            } => {}
            Action::Store {
                content,
                ..
            } => {
                let max_text_size = common_params.max_text_content_size();
                if content.len() > max_text_size {
                    return Err(SyntaxError::TextContentTooBig)
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn verify_with_signer_address(&self, signer: &Address) -> Result<(), SyntaxError> {
        if let Action::WrapCCC {
            payer,
            ..
        } = self
        {
            if payer != signer {
                return Err(SyntaxError::InvalidSignerOfWrapCCC)
            }
        }
        if let Some(approvals) = self.approvals() {
            let tracker = self.tracker().unwrap();

            for approval in approvals {
                recover(approval, &tracker).map_err(|err| SyntaxError::InvalidApproval(err.to_string()))?;
            }
        }
        Ok(())
    }

    fn approvals(&self) -> Option<&[Signature]> {
        match self {
            Action::MintAsset {
                approvals,
                ..
            }
            | Action::TransferAsset {
                approvals,
                ..
            }
            | Action::ChangeAssetScheme {
                approvals,
                ..
            }
            | Action::IncreaseAssetSupply {
                approvals,
                ..
            } => Some(approvals),
            _ => None,
        }
    }

    fn network_id(&self) -> Option<NetworkId> {
        match self {
            Action::MintAsset {
                network_id,
                ..
            }
            | Action::TransferAsset {
                network_id,
                ..
            }
            | Action::ChangeAssetScheme {
                network_id,
                ..
            }
            | Action::IncreaseAssetSupply {
                network_id,
                ..
            }
            | Action::UnwrapCCC {
                network_id,
                ..
            } => Some(*network_id),
            _ => None,
        }
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
                registrar,
                allowed_script_hashes,
                output,
                ..
            } => Some(ShardTransaction::MintAsset {
                network_id,
                shard_id,
                metadata,
                approver,
                registrar,
                allowed_script_hashes,
                output: *output,
            }),
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                ..
            } => Some(ShardTransaction::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
            }),
            Action::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                seq,
                metadata,
                approver,
                registrar,
                allowed_script_hashes,
                ..
            } => Some(ShardTransaction::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                seq,
                metadata,
                approver,
                registrar,
                allowed_script_hashes,
            }),
            Action::IncreaseAssetSupply {
                network_id,
                shard_id,
                asset_type,
                seq,
                output,
                ..
            } => Some(ShardTransaction::IncreaseAssetSupply {
                network_id,
                shard_id,
                asset_type,
                seq,
                output: *output,
            }),
            Action::UnwrapCCC {
                network_id,
                burn,
                receiver,
            } => Some(ShardTransaction::UnwrapCCC {
                network_id,
                burn,
                receiver,
            }),
            _ => None,
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
                registrar,
                allowed_script_hashes,
                output,
                approvals,
            } => {
                s.begin_list(11)
                    .append(&ActionTag::MintAsset)
                    .append(network_id)
                    .append(shard_id)
                    .append(metadata)
                    .append(&output.lock_script_hash)
                    .append(&output.parameters)
                    .append(&output.supply)
                    .append(approver)
                    .append(registrar)
                    .append_list(allowed_script_hashes)
                    .append_list(approvals);
            }
            Action::TransferAsset {
                network_id,
                burns,
                inputs,
                outputs,
                metadata,
                approvals,
                expiration,
            } => {
                let empty: Vec<AssetTransferOutput> = vec![];
                s.begin_list(9)
                    .append(&ActionTag::TransferAsset)
                    .append(network_id)
                    .append_list(burns)
                    .append_list(inputs)
                    .append_list(outputs)
                    // NOTE: The orders field removed.
                    .append_list(&empty)
                    .append(metadata)
                    .append_list(approvals)
                    .append(expiration);
            }
            Action::ChangeAssetScheme {
                network_id,
                shard_id,
                asset_type,
                seq,
                metadata,
                approver,
                registrar,
                allowed_script_hashes,
                approvals,
            } => {
                s.begin_list(10)
                    .append(&ActionTag::ChangeAssetScheme)
                    .append(network_id)
                    .append(shard_id)
                    .append(asset_type)
                    .append(seq)
                    .append(metadata)
                    .append(approver)
                    .append(registrar)
                    .append_list(allowed_script_hashes)
                    .append_list(approvals);
            }
            Action::IncreaseAssetSupply {
                network_id,
                shard_id,
                asset_type,
                seq,
                output,
                approvals,
            } => {
                s.begin_list(9)
                    .append(&ActionTag::IncreaseAssetSupply)
                    .append(network_id)
                    .append(shard_id)
                    .append(asset_type)
                    .append(seq)
                    .append(&output.lock_script_hash)
                    .append(&output.parameters)
                    .append(&output.supply)
                    .append_list(approvals);
            }
            Action::UnwrapCCC {
                network_id,
                burn,
                receiver,
            } => {
                s.begin_list(4).append(&ActionTag::UnwrapCcc).append(network_id).append(burn).append(receiver);
            }
            Action::Pay {
                receiver,
                quantity,
            } => {
                s.begin_list(3);
                s.append(&ActionTag::Pay);
                s.append(receiver);
                s.append(quantity);
            }
            Action::SetRegularKey {
                key,
            } => {
                s.begin_list(2);
                s.append(&ActionTag::SetRegularKey);
                s.append(key);
            }
            Action::CreateShard {
                users,
            } => {
                s.begin_list(2);
                s.append(&ActionTag::CreateShard);
                s.append_list(users);
            }
            Action::SetShardOwners {
                shard_id,
                owners,
            } => {
                s.begin_list(3);
                s.append(&ActionTag::SetShardOwners);
                s.append(shard_id);
                s.append_list(owners);
            }
            Action::SetShardUsers {
                shard_id,
                users,
            } => {
                s.begin_list(3);
                s.append(&ActionTag::SetShardUsers);
                s.append(shard_id);
                s.append_list(users);
            }
            Action::WrapCCC {
                shard_id,
                lock_script_hash,
                parameters,
                quantity,
                payer,
            } => {
                s.begin_list(6);
                s.append(&ActionTag::WrapCcc);
                s.append(shard_id);
                s.append(lock_script_hash);
                s.append(parameters);
                s.append(quantity);
                s.append(payer);
            }
            Action::Store {
                content,
                certifier,
                signature,
            } => {
                s.begin_list(4);
                s.append(&ActionTag::Store);
                s.append(content);
                s.append(certifier);
                s.append(signature);
            }
            Action::Remove {
                hash,
                signature,
            } => {
                s.begin_list(3);
                s.append(&ActionTag::Remove);
                s.append(hash);
                s.append(signature);
            }
            Action::Custom {
                handler_id,
                bytes,
            } => {
                s.begin_list(3);
                s.append(&ActionTag::Custom);
                s.append(handler_id);
                s.append(bytes);
            }
        }
    }
}

impl Decodable for Action {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        match rlp.val_at(0)? {
            ActionTag::MintAsset => {
                let item_count = rlp.item_count()?;
                if item_count != 11 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 11,
                    })
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
                    registrar: rlp.val_at(8)?,
                    allowed_script_hashes: rlp.list_at(9)?,
                    approvals: rlp.list_at(10)?,
                })
            }
            ActionTag::TransferAsset => {
                let item_count = rlp.item_count()?;
                if item_count != 9 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 9,
                    })
                }
                Ok(Action::TransferAsset {
                    network_id: rlp.val_at(1)?,
                    burns: rlp.list_at(2)?,
                    inputs: rlp.list_at(3)?,
                    outputs: rlp.list_at(4)?,
                    metadata: rlp.val_at(6)?,
                    approvals: rlp.list_at(7)?,
                    expiration: rlp.val_at(8)?,
                })
            }
            ActionTag::ChangeAssetScheme => {
                let item_count = rlp.item_count()?;
                if item_count != 10 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 10,
                    })
                }
                Ok(Action::ChangeAssetScheme {
                    network_id: rlp.val_at(1)?,
                    shard_id: rlp.val_at(2)?,
                    asset_type: rlp.val_at(3)?,
                    seq: rlp.val_at(4)?,
                    metadata: rlp.val_at(5)?,
                    approver: rlp.val_at(6)?,
                    registrar: rlp.val_at(7)?,
                    allowed_script_hashes: rlp.list_at(8)?,
                    approvals: rlp.list_at(9)?,
                })
            }
            ActionTag::IncreaseAssetSupply => {
                let item_count = rlp.item_count()?;
                if item_count != 9 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 9,
                    })
                }
                Ok(Action::IncreaseAssetSupply {
                    network_id: rlp.val_at(1)?,
                    shard_id: rlp.val_at(2)?,
                    asset_type: rlp.val_at(3)?,
                    seq: rlp.val_at(4)?,
                    output: Box::new(AssetMintOutput {
                        lock_script_hash: rlp.val_at(5)?,
                        parameters: rlp.val_at(6)?,
                        supply: rlp.val_at(7)?,
                    }),
                    approvals: rlp.list_at(8)?,
                })
            }
            ActionTag::UnwrapCcc => {
                let item_count = rlp.item_count()?;
                if item_count != 4 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 4,
                    })
                }
                Ok(Action::UnwrapCCC {
                    network_id: rlp.val_at(1)?,
                    burn: rlp.val_at(2)?,
                    receiver: rlp.val_at(3)?,
                })
            }
            ActionTag::Pay => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                Ok(Action::Pay {
                    receiver: rlp.val_at(1)?,
                    quantity: rlp.val_at(2)?,
                })
            }
            ActionTag::SetRegularKey => {
                let item_count = rlp.item_count()?;
                if item_count != 2 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 2,
                    })
                }
                Ok(Action::SetRegularKey {
                    key: rlp.val_at(1)?,
                })
            }
            ActionTag::CreateShard => {
                let item_count = rlp.item_count()?;
                if item_count != 2 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 2,
                    })
                }
                Ok(Action::CreateShard {
                    users: rlp.list_at(1)?,
                })
            }
            ActionTag::SetShardOwners => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                Ok(Action::SetShardOwners {
                    shard_id: rlp.val_at(1)?,
                    owners: rlp.list_at(2)?,
                })
            }
            ActionTag::SetShardUsers => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                Ok(Action::SetShardUsers {
                    shard_id: rlp.val_at(1)?,
                    users: rlp.list_at(2)?,
                })
            }
            ActionTag::WrapCcc => {
                let item_count = rlp.item_count()?;
                if item_count != 6 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 6,
                    })
                }
                Ok(Action::WrapCCC {
                    shard_id: rlp.val_at(1)?,
                    lock_script_hash: rlp.val_at(2)?,
                    parameters: rlp.val_at(3)?,
                    quantity: rlp.val_at(4)?,
                    payer: rlp.val_at(5)?,
                })
            }
            ActionTag::Store => {
                let item_count = rlp.item_count()?;
                if item_count != 4 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 4,
                    })
                }
                Ok(Action::Store {
                    content: rlp.val_at(1)?,
                    certifier: rlp.val_at(2)?,
                    signature: rlp.val_at(3)?,
                })
            }
            ActionTag::Remove => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                Ok(Action::Remove {
                    hash: rlp.val_at(1)?,
                    signature: rlp.val_at(2)?,
                })
            }
            ActionTag::Custom => {
                let item_count = rlp.item_count()?;
                if item_count != 3 {
                    return Err(DecoderError::RlpIncorrectListLen {
                        got: item_count,
                        expected: 3,
                    })
                }
                Ok(Action::Custom {
                    handler_id: rlp.val_at(1)?,
                    bytes: rlp.val_at(2)?,
                })
            }
        }
    }
}

fn is_input_and_output_consistent(inputs: &[AssetTransferInput], outputs: &[AssetTransferOutput]) -> bool {
    let mut sum: HashMap<(H160, ShardId), u128> = HashMap::new();

    for input in inputs {
        let shard_asset_type = (input.prev_out.asset_type, input.prev_out.shard_id);
        let quantity = u128::from(input.prev_out.quantity);
        *sum.entry(shard_asset_type).or_insert_with(Default::default) += quantity;
    }
    for output in outputs {
        let shard_asset_type = (output.asset_type, output.shard_id);
        let quantity = u128::from(output.quantity);
        let current_quantity = if let Some(current_quantity) = sum.get(&shard_asset_type) {
            if *current_quantity < quantity {
                return false
            }
            *current_quantity
        } else {
            return false
        };
        let t = sum.insert(shard_asset_type, current_quantity - quantity);
        debug_assert!(t.is_some());
    }

    sum.iter().all(|(_, sum)| *sum == 0)
}

fn check_duplication_in_prev_out(
    burns: &[AssetTransferInput],
    inputs: &[AssetTransferInput],
) -> Result<(), SyntaxError> {
    let mut prev_out_set = HashSet::new();
    for input in inputs.iter().chain(burns) {
        let prev_out = (input.prev_out.tracker, input.prev_out.index);
        if !prev_out_set.insert(prev_out) {
            return Err(SyntaxError::DuplicatedPreviousOutput {
                tracker: input.prev_out.tracker,
                index: input.prev_out.index,
            })
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;
    use crate::transaction::AssetOutPoint;

    #[test]
    fn encode_and_decode_mint_asset() {
        rlp_encode_and_decode_test!(Action::MintAsset {
            network_id: "tc".into(),
            shard_id: 0xc,
            metadata: "mint test".to_string(),
            output: Box::new(AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![],
                supply: 10000,
            }),
            approver: None,
            registrar: None,
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
                supply: 10000,
            }),
            approver: None,
            registrar: None,
            allowed_script_hashes: vec![],
            approvals: vec![Signature::random()],
        });
    }

    #[test]
    fn encode_and_decode_mint_with_single_quotation() {
        rlp_encode_and_decode_test!(Action::MintAsset {
            network_id: "tc".into(),
            shard_id: 3,
            metadata: "metadata has a single quotation(')".to_string(),
            output: Box::new(AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![vec![1, 2, 3], vec![4, 5, 6], vec![0, 7]],
                supply: 10000,
            }),
            approver: None,
            registrar: None,
            allowed_script_hashes: vec![],
            approvals: vec![Signature::random()],
        });
    }

    #[test]
    fn encode_and_decode_mint_with_apostrophe() {
        rlp_encode_and_decode_test!(Action::MintAsset {
            network_id: "tc".into(),
            shard_id: 3,
            metadata: "metadata has an apostrophe(’)".to_string(),
            output: Box::new(AssetMintOutput {
                lock_script_hash: H160::random(),
                parameters: vec![vec![1, 2, 3], vec![4, 5, 6], vec![0, 7]],
                supply: 10000,
            }),
            approver: None,
            registrar: None,
            allowed_script_hashes: vec![],
            approvals: vec![Signature::random()],
        });
    }

    #[test]
    fn encode_and_decode_transfer_asset() {
        let burns = vec![];
        let inputs = vec![];
        let outputs = vec![];
        let network_id = "tc".into();
        let metadata = "".into();
        rlp_encode_and_decode_test!(Action::TransferAsset {
            network_id,
            burns,
            inputs,
            outputs,
            metadata,
            approvals: vec![Signature::random(), Signature::random()],
            expiration: Some(10),
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
            hash: H256::random().into(),
            signature: Signature::random(),
        });
    }

    #[test]
    fn encode_and_decode_change_asset_scheme_action() {
        rlp_encode_and_decode_test!(Action::ChangeAssetScheme {
            network_id: "ab".into(),
            shard_id: 1,
            asset_type: H160::random(),
            seq: 0,
            metadata: "some asset scheme metadata".to_string(),
            approver: Some(Address::random()),
            registrar: Some(Address::random()),
            allowed_script_hashes: vec![H160::random(), H160::random(), H160::random()],
            approvals: vec![],
        });
    }

    #[test]
    fn verify_unwrap_ccc_transaction_should_fail() {
        let tx_zero_quantity = Action::UnwrapCCC {
            network_id: NetworkId::default(),
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: Default::default(),
                    index: 0,
                    asset_type: H160::zero(),
                    shard_id: 0,
                    quantity: 0,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            receiver: Address::random(),
        };
        assert_eq!(tx_zero_quantity.verify(), Err(SyntaxError::ZeroQuantity));

        let invalid_asset_type = H160::random();
        let tx_invalid_asset_type = Action::UnwrapCCC {
            network_id: NetworkId::default(),
            burn: AssetTransferInput {
                prev_out: AssetOutPoint {
                    tracker: Default::default(),
                    index: 0,
                    asset_type: invalid_asset_type,
                    shard_id: 0,
                    quantity: 1,
                },
                timelock: None,
                lock_script: vec![0x30, 0x01],
                unlock_script: vec![],
            },
            receiver: Address::random(),
        };
        assert_eq!(tx_invalid_asset_type.verify(), Err(SyntaxError::InvalidAssetType(invalid_asset_type)));
    }

    #[test]
    fn verify_wrap_ccc_transaction_should_fail() {
        let tx_zero_quantity = Action::WrapCCC {
            shard_id: 0,
            lock_script_hash: H160::random(),
            parameters: vec![],
            quantity: 0,
            payer: Address::random(),
        };
        assert_eq!(tx_zero_quantity.verify(), Err(SyntaxError::ZeroQuantity));
    }
}
