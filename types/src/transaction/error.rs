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
use primitives::H256;
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::util::unexpected::Mismatch;
use super::super::{ShardId, WorldId};

#[derive(Debug, PartialEq, Clone, Eq, Serialize)]
#[serde(tag = "type", content = "content")]
pub enum Error {
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
    InsufficientPermission,
    InvalidWorldId(WorldId),
    InvalidWorldNonce(Mismatch<u64>),
    EmptyShardOwners(ShardId),
    NotRegistrar(Mismatch<Address>),
    /// Returned when the amount of either input or output is 0.
    ZeroAmount,
}

const ERROR_ID_INVALID_ASSET_AMOUNT: u8 = 4u8;
const ERROR_ID_ASSET_NOT_FOUND: u8 = 5u8;
const ERROR_ID_ASSET_SCHEME_NOT_FOUND: u8 = 6u8;
const ERROR_ID_INVALID_ASSET_TYPE: u8 = 7u8;
const ERROR_ID_SCRIPT_HASH_MISMATCH: u8 = 8u8;
const ERROR_ID_INVALID_SCRIPT: u8 = 9u8;
const ERROR_ID_FAILED_TO_UNLOCK: u8 = 10u8;
const ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT: u8 = 11u8;
const ERROR_ID_INVALID_SHARD_NONCE: u8 = 12u8;
const ERROR_ID_INSUFFICIENT_PERMISSION: u8 = 13u8;
const ERROR_ID_INVALID_WORLD_ID: u8 = 14u8;
const ERROR_ID_INVALID_WORLD_NONCE: u8 = 15u8;
const ERROR_ID_EMPTY_SHARD_OWNERS: u8 = 16u8;
const ERROR_ID_NOT_REGISTRAR: u8 = 17u8;
const ERROR_ID_ZERO_AMOUNT: u8 = 18u8;

impl Encodable for Error {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
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
            Error::FailedToUnlock(hash) => s.begin_list(2).append(&ERROR_ID_FAILED_TO_UNLOCK).append(hash),
            Error::InconsistentTransactionInOut => s.begin_list(1).append(&ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT),
            Error::InvalidShardNonce(mismatch) => {
                s.begin_list(2).append(&ERROR_ID_INVALID_SHARD_NONCE).append(mismatch)
            }
            Error::InsufficientPermission => s.begin_list(1).append(&ERROR_ID_INSUFFICIENT_PERMISSION),
            Error::InvalidWorldId(world_id) => s.begin_list(2).append(&ERROR_ID_INVALID_WORLD_ID).append(world_id),
            Error::InvalidWorldNonce(mismatch) => {
                s.begin_list(2).append(&ERROR_ID_INVALID_WORLD_NONCE).append(mismatch)
            }
            Error::EmptyShardOwners(shard_id) => s.begin_list(2).append(&ERROR_ID_EMPTY_SHARD_OWNERS).append(shard_id),
            Error::NotRegistrar(mismatch) => s.begin_list(2).append(&ERROR_ID_NOT_REGISTRAR).append(mismatch),
            Error::ZeroAmount => s.begin_list(1).append(&ERROR_ID_ZERO_AMOUNT),
        };
    }
}

impl Decodable for Error {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let tag = rlp.val_at::<u8>(0)?;
        Ok(match tag {
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
            ERROR_ID_INSUFFICIENT_PERMISSION => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InsufficientPermission
            }
            ERROR_ID_INVALID_WORLD_ID => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidWorldId(rlp.val_at(1)?)
            }
            ERROR_ID_INVALID_WORLD_NONCE => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidWorldNonce(rlp.val_at(1)?)
            }
            ERROR_ID_EMPTY_SHARD_OWNERS => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::EmptyShardOwners(rlp.val_at(1)?)
            }
            ERROR_ID_NOT_REGISTRAR => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::NotRegistrar(rlp.val_at(1)?)
            }
            ERROR_ID_ZERO_AMOUNT => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::ZeroAmount
            }
            _ => return Err(DecoderError::Custom("Invalid transaction error")),
        })
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FormatResult {
        match self {
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
            Error::InsufficientPermission => write!(f, "The current sender doesn't have the permission"),
            Error::InvalidWorldId(_) => write!(f, "The world id is invalid"),
            Error::InvalidWorldNonce(mismatch) => write!(f, "The world nonce {}", mismatch),
            Error::EmptyShardOwners(shard_id) => write!(f, "Shard({}) must have at least one owner", shard_id),
            Error::NotRegistrar(mismatch) => write!(
                f,
                "The signer of the parcel({}) does not match the asset's registrar({})",
                mismatch.found, mismatch.expected
            ),
            Error::ZeroAmount => write!(f, "An amount cannot be 0"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_decode_insufficient_permission() {
        rlp_encode_and_decode_test!(Error::InsufficientPermission);
    }

    #[test]
    fn encode_and_decode_invalid_world_id() {
        rlp_encode_and_decode_test!(Error::InvalidWorldId(3));
    }

    #[test]
    fn encode_and_decode_invalid_world_nonce() {
        rlp_encode_and_decode_test!(Error::InvalidWorldNonce(Mismatch {
            expected: 1,
            found: 2,
        }));
    }
}
