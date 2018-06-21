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
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

use super::super::consensus::epoch::{PendingTransition as PendingEpochTransition, Transition as EpochTransition};
use super::super::db::Key;
use super::super::invoice::Invoice;
use super::super::types::{BlockNumber, ParcelId};

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

impl Into<ParcelId> for ParcelAddress {
    fn into(self) -> ParcelId {
        ParcelId::Location(self.block_hash.into(), self.index)
    }
}


/// Represents address of certain transaction within parcel
#[derive(Debug, PartialEq, Clone, RlpEncodable, RlpDecodable)]
pub struct TransactionAddress {
    pub parcel_address: ParcelAddress,
    /// Transaction index within the parcel
    pub index: usize,
}


#[derive(Clone, Debug, PartialEq, RlpEncodableWrapper, RlpDecodableWrapper)]
pub struct ParcelInvoices {
    pub invoices: Vec<Invoice>,
}

impl ParcelInvoices {
    pub fn new(invoices: Vec<Invoice>) -> Self {
        Self {
            invoices,
        }
    }

    pub fn iter(&self) -> ::std::slice::Iter<Invoice> {
        self.invoices.iter()
    }
}

impl Into<Vec<Invoice>> for ParcelInvoices {
    fn into(self) -> Vec<Invoice> {
        self.invoices
    }
}

impl<'a> Into<&'a Vec<Invoice>> for &'a ParcelInvoices {
    fn into(self) -> &'a Vec<Invoice> {
        &self.invoices
    }
}

impl From<Vec<Invoice>> for ParcelInvoices {
    fn from(invoices: Vec<Invoice>) -> Self {
        Self {
            invoices,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockInvoices {
    pub invoices: Vec<ParcelInvoices>,
}

impl BlockInvoices {
    pub fn new(invoices: Vec<ParcelInvoices>) -> Self {
        Self {
            invoices,
        }
    }
}

impl Decodable for BlockInvoices {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        let invoices = rlp.as_list::<Vec<u8>>()?
            .iter()
            .map(|parcel_invoices| UntrustedRlp::new(&parcel_invoices).as_val::<ParcelInvoices>())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            invoices,
        })
    }
}

impl Encodable for BlockInvoices {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(self.invoices.len());
        for i in self.invoices.iter() {
            let encoded = i.rlp_bytes();
            s.append(&encoded.into_vec());
        }
    }
}

/// Candidate transitions to an epoch with specific number.
#[derive(Clone, RlpEncodable, RlpDecodable)]
pub struct EpochTransitions {
    pub number: u64,
    pub candidates: Vec<EpochTransition>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlp::{Encodable, UntrustedRlp};

    #[test]
    fn rlp_encode_and_decode_parcel_invoices() {
        let invoices = vec![
            Invoice::Success,
            Invoice::Success,
            Invoice::Failed,
            Invoice::Success,
            Invoice::Success,
            Invoice::Success,
        ];
        let parcel_invoices = ParcelInvoices {
            invoices,
        };
        let rlp_encoded = parcel_invoices.rlp_bytes();
        let stream = UntrustedRlp::new(&rlp_encoded);
        let rlp_decoded = stream.as_val();
        assert_eq!(Ok(parcel_invoices), rlp_decoded);
    }

    #[test]
    fn rlp_encode_and_decode_block_invoices() {
        let invoices = vec![Invoice::Success, Invoice::Failed];
        let parcel_invoices = ParcelInvoices {
            invoices,
        };
        let block_invoices = BlockInvoices {
            invoices: vec![
                parcel_invoices.clone(),
                parcel_invoices.clone(),
                parcel_invoices.clone(),
                parcel_invoices.clone(),
            ],
        };
        let rlp_encoded = block_invoices.rlp_bytes();
        println!("..:{:?}:", rlp_encoded);
        let rlp = UntrustedRlp::new(&rlp_encoded);
        let rlp_decoded = rlp.as_val();
        assert_eq!(Ok(block_invoices), rlp_decoded);
    }
}
