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

use std::fmt::{Display, Formatter, Result as FormatResult};

use ckey::Address;
use primitives::{H256, U256};

use super::super::util::unexpected::Mismatch;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
pub enum Error {
    InvalidPaymentSender(Mismatch<Address>),
    InvalidAddressToSetKey(Mismatch<Address>),
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
    /// Returned when the sum of the transaction's inputs is different from the sum of outputs.
    InconsistentTransactionInOut,
    InvalidShardNonce(Mismatch<u64>),
}

const ERROR_ID_INVALID_PAYMENT_SENDER: u8 = 1u8;
const ERROR_ID_INVALID_ADDRESS_TO_SET_KEY: u8 = 2u8;
const ERROR_ID_INSUFFICIENT_BALANCE: u8 = 3u8;
const ERROR_ID_INVALID_ASSET_AMOUNT: u8 = 4u8;
const ERROR_ID_ASSET_NOT_FOUND: u8 = 5u8;
const ERROR_ID_ASSET_SCHEME_NOT_FOUND: u8 = 6u8;
const ERROR_ID_INVALID_ASSET_TYPE: u8 = 7u8;
const ERROR_ID_SCRIPT_HASH_MISMATCH: u8 = 8u8;
const ERROR_ID_INVALID_SCRIPT: u8 = 9u8;
const ERROR_ID_FAILED_TO_UNLOCK: u8 = 10u8;
const ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT: u8 = 11u8;
const ERROR_ID_INVALID_SHARD_NONCE: u8 = 12u8;

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Error::InvalidPaymentSender(mismatch) => {
                s.begin_list(2).append(&ERROR_ID_INVALID_PAYMENT_SENDER).append(mismatch)
            }
            Error::InvalidAddressToSetKey(mismatch) => {
                s.begin_list(2).append(&ERROR_ID_INVALID_ADDRESS_TO_SET_KEY).append(mismatch)
            }
            Error::InsufficientBalance {
                address,
                required,
                got,
            } => s.begin_list(4).append(&ERROR_ID_INSUFFICIENT_BALANCE).append(address).append(required).append(got),
            Error::InvalidAssetAmount {
                address,
                expected,
                got,
            } => s.begin_list(4).append(&ERROR_ID_INVALID_ASSET_AMOUNT).append(address).append(expected).append(got),
            Error::AssetNotFound(addr) => s.begin_list(2).append(&ERROR_ID_ASSET_NOT_FOUND).append(addr),
            Error::AssetSchemeNotFound(addr) => s.begin_list(2).append(&ERROR_ID_ASSET_SCHEME_NOT_FOUND).append(addr),
            Error::InvalidAssetType(addr) => s.begin_list(2).append(&ERROR_ID_INVALID_ASSET_TYPE).append(addr),
            Error::ScriptHashMismatch(mismatch) => {
                s.begin_list(2).append(&ERROR_ID_SCRIPT_HASH_MISMATCH).append(mismatch)
            }
            Error::InvalidScript => s.begin_list(1).append(&ERROR_ID_INVALID_SCRIPT),
            Error::FailedToUnlock(hash) => s.begin_list(1).append(&ERROR_ID_FAILED_TO_UNLOCK).append(hash),
            Error::InconsistentTransactionInOut => s.begin_list(1).append(&ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT),
            Error::InvalidShardNonce(mismatch) => {
                s.begin_list(2).append(&ERROR_ID_INVALID_SHARD_NONCE).append(mismatch)
            }
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        Ok(match tag {
            ERROR_ID_INVALID_PAYMENT_SENDER => Error::InvalidPaymentSender(rlp.val_at(1)?),
            ERROR_ID_INVALID_ADDRESS_TO_SET_KEY => Error::InvalidAddressToSetKey(rlp.val_at(1)?),
            ERROR_ID_INSUFFICIENT_BALANCE => Error::InsufficientBalance {
                address: rlp.val_at(1)?,
                required: rlp.val_at(2)?,
                got: rlp.val_at(3)?,
            },
            ERROR_ID_INVALID_ASSET_AMOUNT => Error::InvalidAssetAmount {
                address: rlp.val_at(1)?,
                expected: rlp.val_at(2)?,
                got: rlp.val_at(3)?,
            },
            ERROR_ID_ASSET_NOT_FOUND => Error::AssetNotFound(rlp.val_at(1)?),
            ERROR_ID_ASSET_SCHEME_NOT_FOUND => Error::AssetSchemeNotFound(rlp.val_at(1)?),
            ERROR_ID_INVALID_ASSET_TYPE => Error::InvalidAssetType(rlp.val_at(1)?),
            ERROR_ID_SCRIPT_HASH_MISMATCH => Error::ScriptHashMismatch(rlp.val_at(1)?),
            ERROR_ID_INVALID_SCRIPT => Error::InvalidScript,
            ERROR_ID_FAILED_TO_UNLOCK => Error::FailedToUnlock(rlp.val_at(1)?),
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT => Error::InconsistentTransactionInOut,
            ERROR_ID_INVALID_SHARD_NONCE => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidShardNonce(rlp.val_at(1)?)
            }
            _ => return Err(DecoderError::Custom("Invalid transaction error")),
        })
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
            Error::InvalidPaymentSender(mismatch) => write!(f, "Invalid payment sender {}", mismatch),
            Error::InvalidAddressToSetKey(mismatch) => write!(f, "Invalid address to set key {}", mismatch),
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
            Error::InconsistentTransactionInOut => {
                write!(f, "The sum of the transaction's inputs is different from the sum of the transaction's outputs")
            }
            Error::InvalidShardNonce(mismatch) => write!(f, "The shard nonce {}", mismatch),
        }
    }
}
