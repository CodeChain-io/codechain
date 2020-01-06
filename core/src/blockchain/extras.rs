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

use crate::db::Key;
use crate::types::TransactionId;
use ctypes::{BlockHash, BlockNumber, Tracker, TxHash};
use primitives::{H256, H264, U256};
use std::ops::{Add, AddAssign, Deref, Sub, SubAssign};

/// Represents index of extra data in database
#[derive(Copy, Debug, Hash, Eq, PartialEq, Clone)]
enum ExtrasIndex {
    /// Block details index
    BlockDetails = 0,
    /// Block hash index
    BlockHash = 1,
    /// Transaction address index
    TransactionAddress = 2,
    /// Transaction addresses index
    TransactionAddresses = 3,
    // (Reserved) = 4,
    // (Reserved) = 5,
}

fn with_index(hash: &H256, i: ExtrasIndex) -> H264 {
    let mut result = H264::default();
    result[0] = i as u8;
    (*result)[1..].copy_from_slice(hash);
    result
}

pub struct BlockNumberKey([u8; 5]);

impl Deref for BlockNumberKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


impl Key<BlockHash> for BlockNumber {
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

impl Key<BlockDetails> for BlockHash {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ExtrasIndex::BlockDetails)
    }
}

impl Key<TransactionAddress> for TxHash {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ExtrasIndex::TransactionAddress)
    }
}

impl Key<TransactionAddresses> for Tracker {
    type Target = H264;

    fn key(&self) -> H264 {
        with_index(self, ExtrasIndex::TransactionAddresses)
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
    pub parent: BlockHash,
}

/// Represents address of certain transaction within block
#[derive(Debug, PartialEq, Clone, Copy, RlpEncodable, RlpDecodable)]
pub struct TransactionAddress {
    /// Block hash
    pub block_hash: BlockHash,
    /// Transaction index within the block
    pub index: usize,
}

impl From<TransactionAddress> for TransactionId {
    fn from(addr: TransactionAddress) -> Self {
        TransactionId::Location(addr.block_hash.into(), addr.index)
    }
}

/// Represents address of certain transaction that has the same tracker
#[derive(Debug, Default, PartialEq, Clone, RlpEncodableWrapper, RlpDecodableWrapper)]
pub struct TransactionAddresses {
    addresses: Vec<TransactionAddress>,
}

impl TransactionAddresses {
    pub fn new(address: TransactionAddress) -> Self {
        Self {
            addresses: vec![address],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.addresses.is_empty()
    }
}

impl IntoIterator for TransactionAddresses {
    type Item = TransactionAddress;
    type IntoIter = ::std::vec::IntoIter<<Self as IntoIterator>::Item>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.addresses.into_iter()
    }
}

impl Add for TransactionAddresses {
    type Output = Self;

    fn add(self, rhs: Self) -> <Self as Add>::Output {
        let mut s = self;
        s += rhs;
        s
    }
}

impl AddAssign for TransactionAddresses {
    fn add_assign(&mut self, rhs: Self) {
        // FIXME: Please fix this O(n*m) algorithm
        let new_addresses: Vec<_> = rhs.into_iter().filter(|addr| !self.addresses.contains(addr)).collect();
        self.addresses.extend(new_addresses);
    }
}

impl Sub for TransactionAddresses {
    type Output = Self;

    fn sub(self, rhs: Self) -> <Self as Add>::Output {
        let mut s = self;
        s -= rhs;
        s
    }
}

impl SubAssign for TransactionAddresses {
    fn sub_assign(&mut self, rhs: Self) {
        // FIXME: Please fix this O(n*m) algorithm
        self.addresses.retain(|addr| !rhs.addresses.contains(addr));
        self.addresses.shrink_to_fit();
    }
}

#[cfg(test)]
mod tests {
    use rlp::rlp_encode_and_decode_test;

    use super::*;

    #[test]
    fn encode_and_decode_transaction_address_with_single_address() {
        rlp_encode_and_decode_test!(TransactionAddresses::new(TransactionAddress {
            block_hash: H256::random().into(),
            index: 0,
        }));
    }

    #[test]
    fn encode_and_decode_transaction_address_without_address() {
        rlp_encode_and_decode_test!(TransactionAddresses::default());
    }

    #[test]
    fn encode_and_decode_transaction_address_with_multiple_addresses() {
        rlp_encode_and_decode_test!(TransactionAddresses {
            addresses: vec![
                TransactionAddress {
                    block_hash: H256::random().into(),
                    index: 0,
                },
                TransactionAddress {
                    block_hash: H256::random().into(),
                    index: 3,
                },
                TransactionAddress {
                    block_hash: H256::random().into(),
                    index: 1,
                },
            ],
        });
    }

    #[test]
    fn add() {
        let t1 = TransactionAddresses {
            addresses: vec![TransactionAddress {
                block_hash: H256::zero().into(),
                index: 0,
            }],
        };
        let t2 = TransactionAddresses {
            addresses: vec![TransactionAddress {
                block_hash: H256::from(1).into(),
                index: 0,
            }],
        };
        assert_eq!(
            vec![
                TransactionAddress {
                    block_hash: H256::zero().into(),
                    index: 0,
                },
                TransactionAddress {
                    block_hash: H256::from(1).into(),
                    index: 0,
                }
            ],
            (t1 + t2).addresses
        );
    }

    #[test]
    fn do_not_add_duplicated_item() {
        let t1 = TransactionAddresses {
            addresses: vec![TransactionAddress {
                block_hash: H256::zero().into(),
                index: 0,
            }],
        };
        let t2 = TransactionAddresses {
            addresses: vec![TransactionAddress {
                block_hash: H256::zero().into(),
                index: 0,
            }],
        };
        assert_eq!(
            vec![TransactionAddress {
                block_hash: H256::zero().into(),
                index: 0,
            },],
            (t1 + t2).addresses
        );
    }

    #[test]
    fn remove() {
        let t1 = TransactionAddresses {
            addresses: vec![
                TransactionAddress {
                    block_hash: H256::zero().into(),
                    index: 0,
                },
                TransactionAddress {
                    block_hash: H256::from(1).into(),
                    index: 0,
                },
                TransactionAddress {
                    block_hash: H256::from(2).into(),
                    index: 0,
                },
            ],
        };
        let t2 = TransactionAddresses {
            addresses: vec![TransactionAddress {
                block_hash: H256::from(1).into(),
                index: 0,
            }],
        };
        assert_eq!(
            vec![
                TransactionAddress {
                    block_hash: H256::zero().into(),
                    index: 0,
                },
                TransactionAddress {
                    block_hash: H256::from(2).into(),
                    index: 0,
                }
            ],
            (t1 - t2).addresses
        );
    }

    #[test]
    fn remove_dont_touch_unmatched_item() {
        let t1 = TransactionAddresses {
            addresses: vec![TransactionAddress {
                block_hash: H256::zero().into(),
                index: 0,
            }],
        };
        let t2 = TransactionAddresses {
            addresses: vec![TransactionAddress {
                block_hash: H256::from(1).into(),
                index: 0,
            }],
        };
        assert_eq!(
            vec![TransactionAddress {
                block_hash: H256::zero().into(),
                index: 0,
            },],
            (t1 - t2).addresses
        );
    }
}
