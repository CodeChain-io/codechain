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
use primitives::{H160, H256};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::Timelock;
use crate::util::unexpected::Mismatch;
use crate::ShardId;

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
    AssetSchemeDuplicated(H256),
    InvalidAssetType(H256),
    /// Script hash does not match with provided lock script
    ScriptHashMismatch(Mismatch<H160>),
    /// Failed to decode script
    InvalidScript,
    /// Script execution result is `Fail`
    FailedToUnlock(H256),
    /// Returned when the sum of the transaction's inputs is different from the sum of outputs.
    InconsistentTransactionInOut,
    /// There are burn/inputs that shares same previous output
    DuplicatedPreviousOutput {
        transaction_hash: H256,
        index: usize,
    },
    InsufficientPermission,
    EmptyShardOwners(ShardId),
    NotApproved(Address),
    /// Returned when the amount of either input or output is 0.
    ZeroAmount,
    TooManyOutputs(usize),
    /// AssetCompose requires at least 1 input.
    EmptyInput,
    InvalidDecomposedInput {
        address: H256,
        got: u64,
    },
    InvalidComposedOutput {
        got: u64,
    },
    InvalidDecomposedOutput {
        address: H256,
        expected: u64,
        got: u64,
    },
    EmptyOutput,
    Timelocked {
        timelock: Timelock,
        remaining_time: u64,
    },
}

const ERROR_ID_INVALID_ASSET_AMOUNT: u8 = 4u8;
const ERROR_ID_ASSET_NOT_FOUND: u8 = 5u8;
const ERROR_ID_ASSET_SCHEME_NOT_FOUND: u8 = 6u8;
const ERROR_ID_INVALID_ASSET_TYPE: u8 = 7u8;
const ERROR_ID_SCRIPT_HASH_MISMATCH: u8 = 8u8;
const ERROR_ID_INVALID_SCRIPT: u8 = 9u8;
const ERROR_ID_FAILED_TO_UNLOCK: u8 = 10u8;
const ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT: u8 = 11u8;
const ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT: u8 = 12u8;
const ERROR_ID_INSUFFICIENT_PERMISSION: u8 = 13u8;
const ERROR_ID_EMPTY_SHARD_OWNERS: u8 = 16u8;
const ERROR_ID_NOT_APPROVED: u8 = 17u8;
const ERROR_ID_ZERO_AMOUNT: u8 = 18u8;
const ERROR_ID_TOO_MANY_OUTPUTS: u8 = 19u8;
const ERROR_ID_ASSET_SCHEME_DUPLICATED: u8 = 20u8;
const ERROR_ID_EMPTY_INPUT: u8 = 21u8;
const ERROR_ID_INVALID_DECOMPOSED_INPUT: u8 = 22u8;
const ERROR_ID_INVALID_COMPOSED_OUTPUT: u8 = 23u8;
const ERROR_ID_INVALID_DECOMPOSED_OUTPUT: u8 = 24u8;
const ERROR_ID_EMPTY_OUTPUT: u8 = 25u8;
const ERROR_ID_TIMELOCKED: u8 = 26u8;

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
            Error::AssetSchemeDuplicated(addr) => {
                s.begin_list(2).append(&ERROR_ID_ASSET_SCHEME_DUPLICATED).append(addr)
            }
            Error::InvalidAssetType(addr) => s.begin_list(2).append(&ERROR_ID_INVALID_ASSET_TYPE).append(addr),
            Error::ScriptHashMismatch(mismatch) => {
                s.begin_list(2).append(&ERROR_ID_SCRIPT_HASH_MISMATCH).append(mismatch)
            }
            Error::InvalidScript => s.begin_list(1).append(&ERROR_ID_INVALID_SCRIPT),
            Error::FailedToUnlock(hash) => s.begin_list(2).append(&ERROR_ID_FAILED_TO_UNLOCK).append(hash),
            Error::InconsistentTransactionInOut => s.begin_list(1).append(&ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT),
            Error::DuplicatedPreviousOutput {
                transaction_hash,
                index,
            } => s.begin_list(3).append(&ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT).append(transaction_hash).append(index),
            Error::InsufficientPermission => s.begin_list(1).append(&ERROR_ID_INSUFFICIENT_PERMISSION),
            Error::EmptyShardOwners(shard_id) => s.begin_list(2).append(&ERROR_ID_EMPTY_SHARD_OWNERS).append(shard_id),
            Error::NotApproved(address) => s.begin_list(2).append(&ERROR_ID_NOT_APPROVED).append(address),
            Error::ZeroAmount => s.begin_list(1).append(&ERROR_ID_ZERO_AMOUNT),
            Error::TooManyOutputs(num) => s.begin_list(2).append(&ERROR_ID_TOO_MANY_OUTPUTS).append(num),
            Error::EmptyInput => s.begin_list(1).append(&ERROR_ID_EMPTY_INPUT),
            Error::InvalidDecomposedInput {
                address,
                got,
            } => s.begin_list(3).append(&ERROR_ID_INVALID_DECOMPOSED_INPUT).append(address).append(got),
            Error::InvalidComposedOutput {
                got,
            } => s.begin_list(2).append(&ERROR_ID_INVALID_COMPOSED_OUTPUT).append(got),
            Error::InvalidDecomposedOutput {
                address,
                expected,
                got,
            } => {
                s.begin_list(4).append(&ERROR_ID_INVALID_DECOMPOSED_OUTPUT).append(address).append(expected).append(got)
            }
            Error::EmptyOutput => s.begin_list(1).append(&ERROR_ID_EMPTY_OUTPUT),
            Error::Timelocked {
                timelock,
                remaining_time,
            } => s.begin_list(3).append(&ERROR_ID_TIMELOCKED).append(timelock).append(remaining_time),
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
            ERROR_ID_ASSET_SCHEME_DUPLICATED => Error::AssetSchemeDuplicated(rlp.val_at(1)?),
            ERROR_ID_INVALID_ASSET_TYPE => Error::InvalidAssetType(rlp.val_at(1)?),
            ERROR_ID_SCRIPT_HASH_MISMATCH => Error::ScriptHashMismatch(rlp.val_at(1)?),
            ERROR_ID_INVALID_SCRIPT => Error::InvalidScript,
            ERROR_ID_FAILED_TO_UNLOCK => Error::FailedToUnlock(rlp.val_at(1)?),
            ERROR_ID_INCONSISTENT_TRANSACTION_IN_OUT => Error::InconsistentTransactionInOut,
            ERORR_ID_DUPLICATED_PREVIOUS_OUTPUT => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::DuplicatedPreviousOutput {
                    transaction_hash: rlp.val_at(1)?,
                    index: rlp.val_at(2)?,
                }
            }
            ERROR_ID_INSUFFICIENT_PERMISSION => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InsufficientPermission
            }
            ERROR_ID_EMPTY_SHARD_OWNERS => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::EmptyShardOwners(rlp.val_at(1)?)
            }
            ERROR_ID_NOT_APPROVED => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::NotApproved(rlp.val_at(1)?)
            }
            ERROR_ID_ZERO_AMOUNT => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::ZeroAmount
            }
            ERROR_ID_TOO_MANY_OUTPUTS => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::TooManyOutputs(rlp.val_at(1)?)
            }
            ERROR_ID_EMPTY_INPUT => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::EmptyInput
            }
            ERROR_ID_INVALID_DECOMPOSED_INPUT => {
                if rlp.item_count()? != 3 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidDecomposedInput {
                    address: rlp.val_at(1)?,
                    got: rlp.val_at(2)?,
                }
            }
            ERROR_ID_INVALID_COMPOSED_OUTPUT => {
                if rlp.item_count()? != 2 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidComposedOutput {
                    got: rlp.val_at(1)?,
                }
            }
            ERROR_ID_INVALID_DECOMPOSED_OUTPUT => {
                if rlp.item_count()? != 4 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::InvalidDecomposedOutput {
                    address: rlp.val_at(1)?,
                    expected: rlp.val_at(2)?,
                    got: rlp.val_at(3)?,
                }
            }
            ERROR_ID_EMPTY_OUTPUT => {
                if rlp.item_count()? != 1 {
                    return Err(DecoderError::RlpInvalidLength)
                }
                Error::EmptyOutput
            }
            ERROR_ID_TIMELOCKED => Error::Timelocked {
                timelock: rlp.val_at(1)?,
                remaining_time: rlp.val_at(2)?,
            },
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
            Error::AssetSchemeDuplicated(addr) => write!(f, "Asset scheme already exists: {}", addr),
            Error::InvalidAssetType(addr) => write!(f, "Asset type is invalid: {}", addr),
            Error::ScriptHashMismatch(mismatch) => {
                write!(f, "Expected script with hash {}, but got {}", mismatch.expected, mismatch.found)
            }
            Error::InvalidScript => write!(f, "Failed to decode script"),
            Error::FailedToUnlock(hash) => write!(f, "Failed to unlock asset {}", hash),
            Error::InconsistentTransactionInOut => {
                write!(f, "The sum of the transaction's inputs is different from the sum of the transaction's outputs")
            }
            Error::DuplicatedPreviousOutput {
                transaction_hash,
                index,
            } => write!(f, "The previous output of inputs/burns are duplicated: ({}, {})", transaction_hash, index),
            Error::InsufficientPermission => write!(f, "The current sender doesn't have the permission"),
            Error::EmptyShardOwners(shard_id) => write!(f, "Shard({}) must have at least one owner", shard_id),
            Error::NotApproved(address) => write!(f, "{} should approve it.", address),
            Error::ZeroAmount => write!(f, "An amount cannot be 0"),
            Error::TooManyOutputs(num) => write!(f, "The number of outputs is {}. It should be 126 or less.", num),
            Error::EmptyInput => write!(f, "The input is empty"),
            Error::InvalidDecomposedInput {
                address,
                got,
            } => write!(f, "The inputs are not valid. The amount of asset({}) input must be 1, but {}.", address, got),
            Error::InvalidComposedOutput {
                got,
            } => write!(f, "The composed output is note valid. The amount must be 1, but {}.", got),
            Error::InvalidDecomposedOutput {
                address,
                expected,
                got,
            } => write!(
                f,
                "The decomposed output is not balid. The amount of asset({}) must be {}, but {}.",
                address, expected, got
            ),
            Error::EmptyOutput => writeln!(f, "The output is empty"),
            Error::Timelocked {
                timelock,
                remaining_time,
            } => write!(
                f,
                "The transaction cannot be executed because of the timelock({:?}). The remaining time is {}",
                timelock, remaining_time
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_insufficient_permission() {
        rlp_encode_and_decode_test!(Error::InsufficientPermission);
    }

    #[test]
    fn encode_and_decode_too_many_outpus() {
        rlp_encode_and_decode_test!(Error::TooManyOutputs(127));
    }
}
