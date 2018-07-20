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

use ccrypto::blake256;
use ckey::Address;
use primitives::{Bytes, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetOutPoint {
    pub transaction_hash: H256,
    pub index: usize,
    pub asset_type: H256,
    pub amount: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferInput {
    pub prev_out: AssetOutPoint,
    pub lock_script: Bytes,
    pub unlock_script: Bytes,
}

#[derive(Debug, Clone, Eq, PartialEq, RlpDecodable, RlpEncodable, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransferOutput {
    pub lock_script_hash: H256,
    pub parameters: Vec<Bytes>,
    pub asset_type: H256,
    pub amount: u64,
}

/// Parcel transaction type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum Transaction {
    #[serde(rename_all = "camelCase")]
    AssetMint {
        network_id: u64,
        shard_id: u32,
        metadata: String,
        registrar: Option<Address>,
        nonce: u64,

        output: AssetMintOutput,
    },
    #[serde(rename_all = "camelCase")]
    AssetTransfer {
        network_id: u64,
        burns: Vec<AssetTransferInput>,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
        nonce: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

    pub fn network_id(&self) -> u64 {
        match self {
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
}

type TransactionId = u8;
const ASSET_MINT_ID: TransactionId = 0x03;
const ASSET_TRANSFER_ID: TransactionId = 0x04;

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
                    registrar: d.val_at(7)?,
                    nonce: d.val_at(8)?,
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
                registrar,
                nonce,
            } => s
                .begin_list(9)
                .append(&ASSET_MINT_ID)
                .append(network_id)
                .append(shard_id)
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
