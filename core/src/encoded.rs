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

//! Lazily-decoded owning views of RLP-encoded blockchain objects.
//! These views are meant to contain _trusted_ data -- without encoding
//! errors or inconsistencies.
//!
//! In general these views are useful when only a few fields of an object
//! are relevant. In these cases it's more efficient to decode the object piecemeal.
//! When the entirety of the object is needed, it's better to upgrade it to a fully
//! decoded object where parts like the hash can be saved.

use ccrypto::blake256;
use ctypes::{H256, U256, Address};
use heapsize::HeapSizeOf;
use rlp::Rlp;

use super::block::Block as FullBlock;
use super::header::Header as FullHeader;
use super::transaction::UnverifiedTransaction;
use super::types::BlockNumber;
use super::views;

/// Owning header view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header(Vec<u8>);

impl HeapSizeOf for Header {
    fn heap_size_of_children(&self) -> usize { self.0.heap_size_of_children() }
}

impl Header {
    /// Create a new owning header view.
    /// Expects the data to be an RLP-encoded header -- any other case will likely lead to
    /// panics further down the line.
    pub fn new(encoded: Vec<u8>) -> Self { Header(encoded) }

    /// Upgrade this encoded view to a fully owned `Header` object.
    pub fn decode(&self) -> FullHeader { ::rlp::decode(&self.0) }

    /// Get a borrowed header view onto the data.
    #[inline]
    pub fn view(&self) -> views::HeaderView { views::HeaderView::new(&self.0) }

    /// Get the rlp of the header.
    #[inline]
    pub fn rlp(&self) -> Rlp { Rlp::new(&self.0) }

    /// Consume the view and return the raw bytes.
    pub fn into_inner(self) -> Vec<u8> { self.0 }
}

// forwarders to borrowed view.
impl Header {
    /// Returns the header hash.
    pub fn hash(&self) -> H256 { blake256(&self.0) }

    /// Returns the parent hash.
    pub fn parent_hash(&self) -> H256 { self.view().parent_hash() }

    /// Returns the author.
    pub fn author(&self) -> Address { self.view().author() }

    /// Returns the state root.
    pub fn state_root(&self) -> H256 { self.view().state_root() }

    /// Returns the transaction trie root.
    pub fn transactions_root(&self) -> H256 { self.view().transactions_root() }

    /// Score of this block
    pub fn score(&self) -> U256 { self.view().score() }

    /// Number of this block.
    pub fn number(&self) -> BlockNumber { self.view().number() }

    /// Time this block was produced.
    pub fn timestamp(&self) -> u64 { self.view().timestamp() }

    /// Engine-specific seal fields.
    pub fn seal(&self) -> Vec<Vec<u8>> { self.view().seal() }
}

/// Owning block body view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body(Vec<u8>);

impl HeapSizeOf for Body {
    fn heap_size_of_children(&self) -> usize { self.0.heap_size_of_children() }
}

impl Body {
    /// Create a new owning block body view. The raw bytes passed in must be an rlp-encoded block
    /// body.
    pub fn new(raw: Vec<u8>) -> Self { Body(raw) }

    /// Get a borrowed view of the data within.
    #[inline]
    pub fn view(&self) -> views::BodyView { views::BodyView::new(&self.0) }

    /// Fully decode this block body.
    pub fn decode(&self) -> Vec<UnverifiedTransaction> {
        self.view().transactions()
    }

    /// Get the RLP of this block body.
    #[inline]
    pub fn rlp(&self) -> Rlp {
        Rlp::new(&self.0)
    }

    /// Consume the view and return the raw bytes.
    pub fn into_inner(self) -> Vec<u8> { self.0 }
}

// forwarders to borrowed view.
impl Body {
    /// Get a vector of all transactions.
    pub fn transactions(&self) -> Vec<UnverifiedTransaction> { self.view().transactions() }

    /// Number of transactions in the block.
    pub fn transactions_count(&self) -> usize { self.view().transactions_count() }

    /// A view over each transaction in the block.
    pub fn transaction_views(&self) -> Vec<views::TransactionView> { self.view().transaction_views() }

    /// The hash of each transaction in the block.
    pub fn transaction_hashes(&self) -> Vec<H256> { self.view().transaction_hashes() }
}

/// Owning block view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block(Vec<u8>);

impl HeapSizeOf for Block {
    fn heap_size_of_children(&self) -> usize { self.0.heap_size_of_children() }
}

impl Block {
    /// Create a new owning block view. The raw bytes passed in must be an rlp-encoded block.
    pub fn new(raw: Vec<u8>) -> Self { Block(raw) }

    /// Get a borrowed view of the whole block.
    #[inline]
    pub fn view(&self) -> views::BlockView { views::BlockView::new(&self.0) }

    /// Get a borrowed view of the block header.
    #[inline]
    pub fn header_view(&self) -> views::HeaderView { self.view().header_view() }

    /// Decode to a full block.
    pub fn decode(&self) -> FullBlock { ::rlp::decode(&self.0) }

    /// Decode the header.
    pub fn decode_header(&self) -> FullHeader { self.rlp().val_at(0) }

    /// Clone the encoded header.
    pub fn header(&self) -> Header { Header(self.rlp().at(0).as_raw().to_vec()) }

    /// Get the rlp of this block.
    #[inline]
    pub fn rlp(&self) -> Rlp {
        Rlp::new(&self.0)
    }

    /// Consume the view and return the raw bytes.
    pub fn into_inner(self) -> Vec<u8> { self.0 }
}

// forwarders to borrowed header view.
impl Block {
    /// Returns the header hash.
    pub fn hash(&self) -> H256 { self.header_view().hash() }

    /// Returns the parent hash.
    pub fn parent_hash(&self) -> H256 { self.header_view().parent_hash() }

    /// Returns the author.
    pub fn author(&self) -> Address { self.header_view().author() }

    /// Returns the state root.
    pub fn state_root(&self) -> H256 { self.header_view().state_root() }

    /// Returns the transaction trie root.
    pub fn transactions_root(&self) -> H256 { self.header_view().transactions_root() }

    /// Score of this block
    pub fn score(&self) -> U256 { self.header_view().score() }

    /// Number of this block.
    pub fn number(&self) -> BlockNumber { self.header_view().number() }

    /// Time this block was produced.
    pub fn timestamp(&self) -> u64 { self.header_view().timestamp() }

    /// Engine-specific seal fields.
    pub fn seal(&self) -> Vec<Vec<u8>> { self.header_view().seal() }
}

// forwarders to body view.
impl Block {
    /// Get a vector of all transactions.
    pub fn transactions(&self) -> Vec<UnverifiedTransaction> { self.view().transactions() }

    /// Number of transactions in the block.
    pub fn transactions_count(&self) -> usize { self.view().transactions_count() }

    /// A view over each transaction in the block.
    pub fn transaction_views(&self) -> Vec<views::TransactionView> { self.view().transaction_views() }

    /// The hash of each transaction in the block.
    pub fn transaction_hashes(&self) -> Vec<H256> { self.view().transaction_hashes() }
}

