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
use std::ops::{self, Deref};

use ctypes::{H256, H264, U256};
use heapsize::HeapSizeOf;
use kvdb::PREFIX_LEN as DB_PREFIX_LEN;

use super::super::consensus::epoch::{PendingTransition as PendingEpochTransition, Transition as EpochTransition};
use super::super::db::Key;
use super::super::invoice::Invoice;
use super::super::types::BlockNumber;

/// Represents index of extra data in database
#[derive(Copy, Debug, Hash, Eq, PartialEq, Clone)]
pub enum ExtrasIndex {
    /// Block details index
    BlockDetails = 0,
    /// Block hash index
    BlockHash = 1,
    /// Parcel address index
    ParcelAddress = 2,
    /// Block invoices index
    BlockInvoices = 3,
    /// Epoch transition data index.
    EpochTransitions = 4,
    /// Pending epoch transition data index.
    PendingEpochTransition = 5,
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

/// length of epoch keys.
pub const EPOCH_KEY_LEN: usize = DB_PREFIX_LEN + 16;

/// epoch key prefix.
/// used to iterate over all epoch transitions in order from genesis.
pub const EPOCH_KEY_PREFIX: &'static [u8; DB_PREFIX_LEN] =
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
    /// List of children block hashes
    pub children: Vec<H256>,
}

impl HeapSizeOf for BlockDetails {
    fn heap_size_of_children(&self) -> usize {
        self.children.heap_size_of_children()
    }
}

/// Represents address of certain parcel within block
#[derive(Debug, PartialEq, Clone, RlpEncodable, RlpDecodable)]
pub struct ParcelAddress {
    /// Block hash
    pub block_hash: H256,
    /// Parcel index within the block
    pub index: usize,
}

impl HeapSizeOf for ParcelAddress {
    fn heap_size_of_children(&self) -> usize {
        0
    }
}

#[derive(Clone, RlpEncodableWrapper, RlpDecodableWrapper)]
pub struct BlockInvoices {
    pub invoices: Vec<Invoice>,
}

impl BlockInvoices {
    pub fn new(invoices: Vec<Invoice>) -> Self {
        Self {
            invoices,
        }
    }
}

/// Candidate transitions to an epoch with specific number.
#[derive(Clone, RlpEncodable, RlpDecodable)]
pub struct EpochTransitions {
    pub number: u64,
    pub candidates: Vec<EpochTransition>,
}
