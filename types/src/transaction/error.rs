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
use unexpected::Mismatch;

#[derive(Debug, PartialEq, Clone)]
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
        }
    }
}
