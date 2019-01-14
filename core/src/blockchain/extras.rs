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

use std::io::Write;
use std::ops::{self, Add, AddAssign, Deref, Sub, SubAssign};

use ctypes::invoice::BlockInvoices;
use ctypes::BlockNumber;
use kvdb::PREFIX_LEN as DB_PREFIX_LEN;
use primitives::{H256, H264, U256};

use crate::consensus::epoch::{PendingTransition as PendingEpochTransition, Transition as EpochTransition};
use crate::db::Key;
use crate::types::TransactionId;

/// Represents index of extra data in database
#[derive(Copy, Debug, Hash, Eq, PartialEq, Clone)]
enum ExtrasIndex {
    /// Block details index
    BlockDetails = 0,
    /// Block hash index
    BlockHash = 1,
    /// Parcel address index
    ParcelAddress = 2,
    /// Transaction address index
    TransactionAddress = 3,
    /// Block invoices index
    BlockInvoices = 4,
    /// Epoch transition data index.
    EpochTransitions = 5,
    /// Pending epoch transition data index.
    PendingEpochTransition = 6,
}

fn with_index(hash: &H256, i: ExtrasIndex) -> H264 {
    let mut result = H264::default();
    result[0] = i as u8;
    (*result)[1..].clone_from_slice(hash);
    result
}

pub struct BlockNumberKey([u8; 5]);

impl Deref for BlockNumberKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


impl Key<H256> for BlockNumber {
    type Target = BlockNumberKey;

    fn key(&self) -> Self::Target {
        let mut result = [0u8; 5];
        result[0] = ExtrasIndex::BlockHash as u8;
        result[1] = (self >> 24) as u8;
        result[2] = (self >> 16) as u8;
        result[3] = (self >> 8) as u8;
        result[4] = *self as u8;
        BlockNumberKey(result)
    }
}

impl Key<BlockDetails> for H256 {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ExtrasIndex::BlockDetails)
    }
}

impl Key<ParcelAddress> for H256 {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ExtrasIndex::ParcelAddress)
    }
}

impl Key<BlockInvoices> for H256 {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ExtrasIndex::BlockInvoices)
    }
}

impl Key<TransactionAddress> for H256 {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ExtrasIndex::TransactionAddress)
    }
}

/// length of epoch keys.
const EPOCH_KEY_LEN: usize = DB_PREFIX_LEN + 16;

/// epoch key prefix.
/// used to iterate over all epoch transitions in order from genesis.
pub const EPOCH_KEY_PREFIX: &[u8; DB_PREFIX_LEN] =
    &[ExtrasIndex::EpochTransitions as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub struct EpochTransitionsKey([u8; EPOCH_KEY_LEN]);

impl ops::Deref for EpochTransitionsKey {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl Key<EpochTransitions> for u64 {
    type Target = EpochTransitionsKey;

    fn key(&self) -> Self::Target {
        let mut arr = [0u8; EPOCH_KEY_LEN];
        arr[..DB_PREFIX_LEN].copy_from_slice(&EPOCH_KEY_PREFIX[..]);

        write!(&mut arr[DB_PREFIX_LEN..], "{:016x}", self)
            .expect("format arg is valid; no more than 16 chars will be written; qed");

        EpochTransitionsKey(arr)
    }
}

impl Key<PendingEpochTransition> for H256 {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ExtrasIndex::PendingEpochTransition)
    }
}

/// Familial details concerning a block
#[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
pub struct BlockDetails {
    /// Block number
    pub number: BlockNumber,
    /// Total score of the block and all its parents
    pub total_score: U256,
    /// Parent block hash
    pub parent: H256,
}

/// Represents address of certain parcel within block
#[derive(Debug, PartialEq, Clone, Copy, RlpEncodable, RlpDecodable)]
pub struct ParcelAddress {
    /// Block hash
    pub block_hash: H256,
    /// Parcel index within the block
    pub index: usize,
}

impl From<ParcelAddress> for TransactionId {
    fn from(addr: ParcelAddress) -> Self {
        TransactionId::Location(addr.block_hash.into(), addr.index)
    }
}

/// Represents address of certain transaction within parcel
#[derive(Debug, Default, PartialEq, Clone, RlpEncodableWrapper, RlpDecodableWrapper)]
pub struct TransactionAddress {
    parcel_addresses: Vec<ParcelAddress>,
}

/// Candidate transitions to an epoch with specific number.
#[derive(Clone, RlpEncodable, RlpDecodable)]
pub struct EpochTransitions {
    pub number: u64,
    pub candidates: Vec<EpochTransition>,
}

impl TransactionAddress {
    pub fn new(parcel_address: ParcelAddress) -> Self {
        Self {
            parcel_addresses: vec![parcel_address],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.parcel_addresses.is_empty()
    }
}

impl IntoIterator for TransactionAddress {
    type Item = ParcelAddress;
    type IntoIter = ::std::vec::IntoIter<<Self as IntoIterator>::Item>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.parcel_addresses.into_iter()
    }
}

impl Add for TransactionAddress {
    type Output = Self;

    fn add(self, rhs: Self) -> <Self as Add>::Output {
        let mut s = self.clone();
        s += rhs;
        s
    }
}

impl AddAssign for TransactionAddress {
    fn add_assign(&mut self, rhs: Self) {
        // FIXME: Please fix this O(n*m) algorithm
        let new_addresses: Vec<_> = rhs.into_iter().filter(|addr| !self.parcel_addresses.contains(addr)).collect();
        self.parcel_addresses.extend(new_addresses);
    }
}

impl Sub for TransactionAddress {
    type Output = Self;

    fn sub(self, rhs: Self) -> <Self as Add>::Output {
        let mut s = self.clone();
        s -= rhs;
        s
    }
}

impl SubAssign for TransactionAddress {
    fn sub_assign(&mut self, rhs: Self) {
        // FIXME: Please fix this O(n*m) algorithm
        self.parcel_addresses.retain(|addr| !rhs.parcel_addresses.contains(addr));
        self.parcel_addresses.shrink_to_fit();
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_transaction_address_with_single_address() {
        rlp_encode_and_decode_test!(TransactionAddress::new(ParcelAddress {
            block_hash: H256::random(),
            index: 0,
        }));
    }

    #[test]
    fn encode_and_decode_transaction_address_without_address() {
        rlp_encode_and_decode_test!(TransactionAddress::default());
    }

    #[test]
    fn encode_and_decode_transaction_address_with_multiple_addresses() {
        rlp_encode_and_decode_test!(TransactionAddress {
            parcel_addresses: vec![
                ParcelAddress {
                    block_hash: H256::random(),
                    index: 0,
                },
                ParcelAddress {
                    block_hash: H256::random(),
                    index: 3,
                },
                ParcelAddress {
                    block_hash: H256::random(),
                    index: 1,
                },
            ],
        });
    }

    #[test]
    fn add() {
        let t1 = TransactionAddress {
            parcel_addresses: vec![ParcelAddress {
                block_hash: 0.into(),
                index: 0,
            }],
        };
        let t2 = TransactionAddress {
            parcel_addresses: vec![ParcelAddress {
                block_hash: 1.into(),
                index: 0,
            }],
        };
        assert_eq!(
            vec![
                ParcelAddress {
                    block_hash: 0.into(),
                    index: 0,
                },
                ParcelAddress {
                    block_hash: 1.into(),
                    index: 0,
                }
            ],
            (t1 + t2).parcel_addresses
        );
    }

    #[test]
    fn do_not_add_duplicated_item() {
        let t1 = TransactionAddress {
            parcel_addresses: vec![ParcelAddress {
                block_hash: 0.into(),
                index: 0,
            }],
        };
        let t2 = TransactionAddress {
            parcel_addresses: vec![ParcelAddress {
                block_hash: 0.into(),
                index: 0,
            }],
        };
        assert_eq!(
            vec![ParcelAddress {
                block_hash: 0.into(),
                index: 0,
            },],
            (t1 + t2).parcel_addresses
        );
    }

    #[test]
    fn remove() {
        let t1 = TransactionAddress {
            parcel_addresses: vec![
                ParcelAddress {
                    block_hash: 0.into(),
                    index: 0,
                },
                ParcelAddress {
                    block_hash: 1.into(),
                    index: 0,
                },
                ParcelAddress {
                    block_hash: 2.into(),
                    index: 0,
                },
            ],
        };
        let t2 = TransactionAddress {
            parcel_addresses: vec![ParcelAddress {
                block_hash: 1.into(),
                index: 0,
            }],
        };
        assert_eq!(
            vec![
                ParcelAddress {
                    block_hash: 0.into(),
                    index: 0,
                },
                ParcelAddress {
                    block_hash: 2.into(),
                    index: 0,
                }
            ],
            (t1 - t2).parcel_addresses
        );
    }

    #[test]
    fn remove_dont_touch_unmatched_item() {
        let t1 = TransactionAddress {
            parcel_addresses: vec![ParcelAddress {
                block_hash: 0.into(),
                index: 0,
            }],
        };
        let t2 = TransactionAddress {
            parcel_addresses: vec![ParcelAddress {
                block_hash: 1.into(),
                index: 0,
            }],
        };
        assert_eq!(
            vec![ParcelAddress {
                block_hash: 0.into(),
                index: 0,
            },],
            (t1 - t2).parcel_addresses
        );
    }
}
