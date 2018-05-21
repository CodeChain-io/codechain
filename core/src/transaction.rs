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

use std::fmt;

use cbytes::Bytes;
use ccrypto::blake256;
use ctypes::{Address, H256, Public, U256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};
use unexpected::Mismatch;

use super::parcel::{AssetTransferInput, AssetTransferOutput};

/// Parcel transaction type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Transaction {
    Noop,
    Payment {
        nonce: U256,
        /// The receiver's address.
        address: Address,
        /// Transferred value.
        value: U256,
    },
    SetRegularKey {
        nonce: U256,
        key: Public,
    },
    AssetMint {
        metadata: String,
        lock_script_hash: H256,
        parameters: Vec<Bytes>,
        amount: Option<u64>,
        registrar: Option<Address>,
    },
    AssetTransfer {
        network_id: u64,
        inputs: Vec<AssetTransferInput>,
        outputs: Vec<AssetTransferOutput>,
    },
}

impl Default for Transaction {
    fn default() -> Transaction {
        Transaction::Noop
    }
}

impl Transaction {
    pub fn without_script(&self) -> Self {
        match self {
            Transaction::AssetTransfer {
                network_id,
                inputs,
                outputs,
            } => {
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
                    inputs: new_inputs,
                    outputs: outputs.clone(),
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
}

type TransactionId = u8;
const PAYMENT_ID: TransactionId = 0x01;
const SET_REGULAR_KEY_ID: TransactionId = 0x02;
const ASSET_MINT_ID: TransactionId = 0x03;
const ASSET_TRANSFER_ID: TransactionId = 0x04;

impl Decodable for Transaction {
    fn decode(d: &UntrustedRlp) -> Result<Self, DecoderError> {
        if d.is_empty() {
            return Ok(Transaction::Noop)
        }

        match d.val_at(0)? {
            PAYMENT_ID => {
                if d.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::Payment {
                    nonce: d.val_at(1)?,
                    address: d.val_at(2)?,
                    value: d.val_at(3)?,
                })
            }
            SET_REGULAR_KEY_ID => {
                if d.item_count()? != 3 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::SetRegularKey {
                    nonce: d.val_at(1)?,
                    key: d.val_at(2)?,
                })
            }
            ASSET_MINT_ID => {
                if d.item_count()? != 6 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetMint {
                    metadata: d.val_at(1)?,
                    lock_script_hash: d.val_at(2)?,
                    parameters: d.val_at(3)?,
                    amount: d.val_at(4)?,
                    registrar: d.val_at(5)?,
                })
            }
            ASSET_TRANSFER_ID => {
                if d.item_count()? != 4 {
                    return Err(DecoderError::RlpIncorrectListLen)
                }
                Ok(Transaction::AssetTransfer {
                    network_id: d.val_at(1)?,
                    inputs: d.list_at(2)?,
                    outputs: d.list_at(3)?,
                })
            }
            _ => Err(DecoderError::Custom("Unexpected transaction")),
        }
    }
}

impl Encodable for Transaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Transaction::Noop => s.append_internal(&""),
            Transaction::Payment {
                nonce,
                address,
                value,
            } => s.begin_list(4).append(&PAYMENT_ID).append(nonce).append(address).append(value),
            Transaction::SetRegularKey {
                nonce,
                key,
            } => s.begin_list(3).append(&SET_REGULAR_KEY_ID).append(nonce).append(key),
            Transaction::AssetMint {
                metadata,
                lock_script_hash,
                parameters,
                amount,
                registrar,
            } => s.begin_list(6)
                .append(&ASSET_MINT_ID)
                .append(metadata)
                .append(lock_script_hash)
                .append(parameters)
                .append(amount)
                .append(registrar),
            Transaction::AssetTransfer {
                network_id,
                inputs,
                outputs,
            } => s.begin_list(4).append(&ASSET_TRANSFER_ID).append(network_id).append_list(inputs).append_list(outputs),
        };
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    InsufficientBalance {
        address: Address,
        required: U256,
        got: U256,
    },
    InvalidAssetAmount {
        address: H256,
        expected: u64,
        got: u64,
    },
    /// Desired input asset not found
    AssetNotFound(H256),
    /// Desired input asset scheme not found
    AssetSchemeNotFound(H256),
    InvalidAssetType(H256),
    /// Script hash does not match with provided lock script
    ScriptHashMismatch(Mismatch<H256>),
    /// Failed to decode script
    InvalidScript,
    /// Script execution result is `Fail`
    FailedToUnlock(H256),
    InvalidNetworkId(Mismatch<u64>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InsufficientBalance {
                address,
                required,
                got,
            } => write!(f, "{} has only {:?} but it must be larger than {:?}", address, required, got),
            Error::InvalidAssetAmount {
                address,
                expected,
                got,
            } => write!(
                f,
                "AssetTransfer must consume input asset completely. The amount of asset({}) must be {}, but {}.",
                address, expected, got
            ),
            Error::AssetNotFound(addr) => write!(f, "Asset not found: {}", addr),
            Error::AssetSchemeNotFound(addr) => write!(f, "Asset scheme not found: {}", addr),
            Error::InvalidAssetType(addr) => write!(f, "Asset type is invalid: {}", addr),
            Error::ScriptHashMismatch(mismatch) => {
                write!(f, "Expected script with hash {}, but got {}", mismatch.expected, mismatch.found)
            }
            Error::InvalidScript => write!(f, "Failed to decode script"),
            Error::FailedToUnlock(hash) => write!(f, "Failed to unlock asset {}", hash),
            Error::InvalidNetworkId(mismatch) => write!(f, "Invalid network id. {}", mismatch),
        }
    }
}
